use std::collections::VecDeque;

use anyhow::{Result, bail};
use battle::engine::{
    entity::{destiny::Destiny, skill::split_ids},
    skill::{
        behavior::{self, classify::BehaviorSpec},
        buff_act::{effect_time, registry as buff_act_registry, wire},
        condition::{
            ParsedCondition, ParsedConditionKind, parse_conditions, registry as condition_registry,
        },
        effect::{ParsedBehavior, SkillEffectCatalog},
        rule::route::ConditionRoute,
    },
};

use crate::scan::{Pending, Report, collect_episode_roots, collect_hero_build_roots, scan_closure};

use super::capture::Evidence;
use super::model::{Buff, BuffAct, Node, Scan, Skill, Slot, Subject, Variant};

pub(crate) fn heroes(
    db: &config::GameDB,
    battle_catalog: battle::catalog::BattleCatalog,
    selected: &[i32],
    evidence: &Evidence,
    wire_evidence: &crate::wire_evidence::Evidence,
) -> Result<Vec<Subject>> {
    let mut rows = db
        .character
        .iter()
        .filter(|hero| hero.is_online == "1")
        .filter(|hero| selected.is_empty() || selected.contains(&hero.id))
        .collect::<Vec<_>>();
    rows.sort_by_key(|hero| hero.id);
    let missing = selected
        .iter()
        .copied()
        .filter(|id| !rows.iter().any(|hero| hero.id == *id))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        bail!("requested heroes are not reportable: {missing:?}");
    }

    rows.into_iter()
        .map(|hero| {
            let mut variants = vec![hero_variant(
                db,
                battle_catalog,
                hero.id,
                None,
                evidence,
                wire_evidence,
            )?];
            for stone in Destiny::stones(db, hero.id) {
                variants.push(hero_variant(
                    db,
                    battle_catalog,
                    hero.id,
                    Some((stone, Destiny::rank_limit(db, stone))),
                    evidence,
                    wire_evidence,
                )?);
            }
            let base_skills = variants[0]
                .scan
                .skills
                .iter()
                .map(|skill| skill.id)
                .collect::<std::collections::BTreeSet<_>>();
            variants[0].observation = if variants[0].scan.skills.iter().any(|skill| skill.observed)
            {
                "Capture referenced"
            } else {
                "Unexercised"
            };
            for variant in variants.iter_mut().skip(1) {
                variant.observation = if variant
                    .scan
                    .skills
                    .iter()
                    .any(|skill| !base_skills.contains(&skill.id) && skill.observed)
                {
                    "Euphoria-specific skill referenced"
                } else {
                    "Unexercised"
                };
            }
            let name = localized(db, &hero.name);
            Ok(Subject {
                id: hero.id,
                slug: slug(&name),
                name,
                variants,
                chapter_id: None,
            })
        })
        .collect()
}

fn hero_variant(
    db: &config::GameDB,
    battle_catalog: battle::catalog::BattleCatalog,
    hero_id: i32,
    destiny: Option<(i32, i32)>,
    evidence: &Evidence,
    wire_evidence: &crate::wire_evidence::Evidence,
) -> Result<Variant> {
    let mut roots = VecDeque::new();
    let mut report = Report::default();
    collect_hero_build_roots(hero_id, None, destiny, db, &mut roots, &mut report)?;
    let label = destiny.map_or_else(
        || "Base".to_owned(),
        |(stone, _)| {
            db.character_destiny_stone_cost(stone)
                .map(|row| {
                    if row.title_name.trim().is_empty() {
                        localized(db, &row.name)
                    } else {
                        localized(db, &row.title_name)
                    }
                })
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| format!("Euphoria {stone}"))
        },
    );
    let scan = scan(db, battle_catalog, roots, report, evidence, wire_evidence);
    Ok(Variant {
        label,
        source_id: destiny.map(|value| value.0),
        source_rank: destiny.map(|value| value.1),
        observation: "Unexercised",
        scan,
    })
}

pub(crate) fn psychubes(
    db: &config::GameDB,
    battle_catalog: battle::catalog::BattleCatalog,
    selected: Option<i32>,
    evidence: &Evidence,
    wire_evidence: &crate::wire_evidence::Evidence,
) -> Result<Vec<Subject>> {
    let mut ids = db
        .equip_skill
        .iter()
        .filter(|row| row.skill > 0 || row.skill2 > 0)
        .filter(|row| selected.is_none_or(|id| row.id == id))
        .map(|row| row.id)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    let subjects = ids
        .into_iter()
        .filter_map(|id| {
            let equip = db.equip.get(id)?;
            let row = db
                .equip_skill
                .iter()
                .filter(|row| row.id == id)
                .max_by_key(|row| row.skill_lv)?;
            let roots = [row.skill, row.skill2]
                .into_iter()
                .filter(|skill_id| *skill_id > 0)
                .map(|skill_id| Pending {
                    id: skill_id,
                    path: format!("psychube {id} > rank {}", row.skill_lv),
                })
                .collect();
            let name = localized(db, &equip.name);
            let scan = scan(
                db,
                battle_catalog,
                roots,
                Report::default(),
                evidence,
                wire_evidence,
            );
            let observation = if scan.skills.iter().any(|skill| skill.observed) {
                "Capture referenced"
            } else {
                "Unexercised"
            };
            Some(Subject {
                id,
                slug: slug(&name),
                name,
                variants: vec![Variant {
                    label: format!("Max amplification {}", row.skill_lv),
                    source_id: Some(id),
                    source_rank: Some(row.skill_lv),
                    observation,
                    scan,
                }],
                chapter_id: None,
            })
        })
        .collect::<Vec<_>>();
    if let Some(selected) = selected
        && subjects.is_empty()
    {
        bail!("requested psychube is not reportable: {selected}");
    }
    Ok(subjects)
}

pub(crate) fn stages(
    db: &config::GameDB,
    battle_catalog: battle::catalog::BattleCatalog,
    selected: Option<i32>,
    evidence: &Evidence,
    wire_evidence: &crate::wire_evidence::Evidence,
) -> Result<Vec<Subject>> {
    let mut rows = db
        .episode
        .iter()
        .filter(|episode| episode.battle_id > 0)
        .filter(|episode| selected.is_none_or(|id| episode.id == id))
        .collect::<Vec<_>>();
    rows.sort_by_key(|episode| (episode.chapter_id, episode.id));
    if let Some(selected) = selected
        && rows.is_empty()
    {
        bail!("requested stage is not reportable: {selected}");
    }
    rows.into_iter()
        .map(|episode| {
            let mut roots = VecDeque::new();
            let mut report = Report::default();
            if let Err(error) = collect_episode_roots(episode.id, db, &mut roots, &mut report) {
                report.errors.insert(format!(
                    "UnresolvedStageConfig episode={} battle={} reason={error:#}",
                    episode.id, episode.battle_id
                ));
            }
            let name = stage_name(db, episode);
            let report_slug = slug(&name);
            Ok(Subject {
                id: episode.id,
                slug: if report_slug.is_empty() {
                    format!("episode-{}", episode.id)
                } else {
                    report_slug
                },
                name,
                variants: vec![Variant {
                    label: format!("Battle {}", episode.battle_id),
                    source_id: Some(episode.battle_id),
                    source_rank: None,
                    observation: if evidence.episode(episode.id) {
                        "Episode observed"
                    } else {
                        "Unexercised"
                    },
                    scan: scan(db, battle_catalog, roots, report, evidence, wire_evidence),
                }],
                chapter_id: Some(episode.chapter_id),
            })
        })
        .collect()
}

fn scan(
    db: &config::GameDB,
    battle_catalog: battle::catalog::BattleCatalog,
    mut roots: VecDeque<Pending>,
    mut report: Report,
    evidence: &Evidence,
    wire_evidence: &crate::wire_evidence::Evidence,
) -> Scan {
    report.wire_evidence = wire_evidence.clone();
    let mut buffs = VecDeque::new();
    let mut catalog = SkillEffectCatalog::from_roots(db, roots.iter().map(|root| root.id), []);
    scan_closure(
        db,
        battle_catalog,
        &mut catalog,
        &mut roots,
        &mut buffs,
        &mut report,
    );
    let mut skill_ids = report.checked_skills.iter().copied().collect::<Vec<_>>();
    skill_ids.sort_unstable();
    let mut buff_ids = report.checked_buffs.iter().copied().collect::<Vec<_>>();
    buff_ids.sort_unstable();
    Scan {
        skills: skill_ids
            .into_iter()
            .filter_map(|id| skill(db, &catalog, id, evidence))
            .collect(),
        buffs: buff_ids
            .into_iter()
            .filter_map(|id| buff(db, id, evidence, wire_evidence))
            .collect(),
        errors: report.errors,
        warnings: report.warnings,
        gaps: report.gaps.len(),
    }
}

fn skill(
    db: &config::GameDB,
    catalog: &SkillEffectCatalog,
    id: i32,
    evidence: &Evidence,
) -> Option<Skill> {
    let row = db.skill.get(id)?;
    let observed = evidence.skill(id);
    let effect = db.skill_effect.get(row.skill_effect);
    let slots = effect
        .map(|effect| {
            raw_slots(effect)
                .into_iter()
                .filter_map(|slot| slot_node(db, slot, observed))
                .collect()
        })
        .unwrap_or_default();
    let issues = catalog
        .issues(id)
        .iter()
        .map(|issue| {
            format!(
                "slot {}: {:?}; opcode={:?}; type={:?}; raw={:?}",
                issue.slot, issue.reason, issue.opcode, issue.type_name, issue.raw
            )
        })
        .collect();
    Some(Skill {
        id,
        effect_id: row.skill_effect,
        name: localized(db, &row.name),
        description: row.eff_desc.clone(),
        mechanic_template: effect.map_or_else(String::new, |effect| localized(db, &effect.desc)),
        effect_metadata: effect.map_or_else(String::new, |effect| {
            format!(
                "damageRate={}; effectTag={}; type={}; skillEffectType={}; isBigSkill={}; isExtra={}; logicTarget={}; targetLimit={}",
                effect.damage_rate,
                effect.effect_tag,
                effect.r#type,
                effect.skill_effect_type,
                effect.is_big_skill,
                effect.is_extra,
                effect.logic_target,
                effect.target_limit,
            )
        }),
        art_description: localized(db, &row.desc_art),
        observed,
        slots,
        issues,
    })
}

struct RawSlot {
    number: usize,
    behavior: String,
    target: String,
    condition: String,
    condition_target: String,
    limit: i32,
    round_limit: i32,
}

fn raw_slots(row: &config::skill_effect::SkillEffect) -> Vec<RawSlot> {
    let value = serde_json::to_value(row).expect("generated skill-effect row serializes");
    (1..=20)
        .map(|number| RawSlot {
            number,
            behavior: field_string(&value, &format!("behavior{number}")),
            target: field_string(&value, &format!("behaviorTarget{number}")),
            condition: field_string(&value, &format!("condition{number}")),
            condition_target: field_string(&value, &format!("conditionTarget{number}")),
            limit: field_i32(&value, &format!("limit{number}")),
            round_limit: field_i32(&value, &format!("roundLimit{number}")),
        })
        .collect()
}

fn slot_node(db: &config::GameDB, raw: RawSlot, skill_observed: bool) -> Option<Slot> {
    if raw.behavior.trim().is_empty() {
        return None;
    }
    let parts = raw.behavior.split('#').map(str::trim).collect::<Vec<_>>();
    let opcode = parts.first().and_then(|value| value.parse::<i32>().ok());
    let type_name = opcode
        .and_then(|id| db.skill_behavior.get(id))
        .map(|row| row.r#type.clone())
        .unwrap_or_default();
    let parsed = opcode
        .zip((!type_name.is_empty()).then_some(type_name.as_str()))
        .map(|(opcode, type_name)| {
            ParsedBehavior::from_spec(
                BehaviorSpec::new(opcode, type_name),
                parts
                    .iter()
                    .skip(1)
                    .filter_map(|value| value.parse().ok())
                    .collect(),
                parts
                    .iter()
                    .skip(1)
                    .map(|value| (*value).to_owned())
                    .collect(),
            )
        });
    let definition = parsed.as_ref().and_then(behavior::registry::find);
    let registry = if definition.is_some() {
        "exact"
    } else {
        "missing"
    };
    let semantic = match (parsed.as_ref(), definition) {
        (_, None) => "route missing",
        (Some(parsed), Some(definition)) if definition.supports.is_none() => {
            "unvalidated arguments"
        }
        (Some(parsed), Some(definition))
            if !definition.supports.is_some_and(|supports| supports(parsed)) =>
        {
            "unsupported arguments"
        }
        (Some(parsed), Some(_)) if !behavior::has_destination(parsed) => "no semantic owner",
        (Some(_), Some(_)) => "supported",
        _ => "malformed",
    };
    let references = definition
        .zip(parsed.as_ref())
        .map(|(definition, parsed)| (definition.references)(parsed))
        .unwrap_or_default();
    let conditions = parse_conditions(db, &raw.condition);
    let route = parsed.as_ref().map_or_else(
        || "unavailable".to_owned(),
        |behavior| {
            format!(
                "{:?}",
                ConditionRoute::compile_for_behavior(&conditions, &behavior.spec)
            )
        },
    );
    let detail = definition.map_or_else(String::new, |definition| {
        format!(
            "kind={:?}; phase={:?}; owner={:?}",
            definition.kind, definition.phase, definition.output_owner
        )
    });
    Some(Slot {
        number: raw.number,
        behavior: Node {
            opcode,
            type_name,
            raw: raw.behavior,
            registry,
            semantic,
            detail,
            observation: if skill_observed {
                "Parent skill referenced"
            } else {
                "Unexercised"
            },
        },
        conditions: flatten_conditions(&conditions)
            .into_iter()
            .map(|condition| condition_node(condition, &raw.condition, skill_observed))
            .collect(),
        behavior_target: raw.target,
        condition_target: raw.condition_target,
        limit: raw.limit,
        round_limit: raw.round_limit,
        route,
        referenced_skills: references.skills,
        referenced_buffs: references.buffs,
    })
}

fn condition_node(condition: &ParsedCondition, raw: &str, skill_observed: bool) -> Node {
    let definition = condition_registry::find_key(condition.opcode, &condition.type_name);
    let semantic = match (&condition.kind, definition) {
        (_, None) => "route missing",
        (ParsedConditionKind::Unsupported(_), Some(_)) => "unsupported arguments",
        _ => "supported",
    };
    Node {
        opcode: Some(condition.opcode),
        type_name: condition.type_name.clone(),
        raw: raw.to_owned(),
        registry: if definition.is_some() {
            "exact"
        } else {
            "missing"
        },
        semantic,
        detail: definition.map_or_else(String::new, |definition| {
            format!(
                "role={:?}; timing={:?}; dependencies={:?}",
                definition.role,
                condition.timing(),
                definition.dependencies
            )
        }),
        observation: if skill_observed {
            "Parent skill referenced"
        } else {
            "Unexercised"
        },
    }
}

fn flatten_conditions(conditions: &[ParsedCondition]) -> Vec<&ParsedCondition> {
    fn push<'a>(condition: &'a ParsedCondition, output: &mut Vec<&'a ParsedCondition>) {
        match &condition.kind {
            ParsedConditionKind::Any(groups) => {
                for nested in groups.iter().flatten() {
                    push(nested, output);
                }
            }
            _ => output.push(condition),
        }
    }
    let mut output = Vec::new();
    for condition in conditions {
        push(condition, &mut output);
    }
    output
}

fn buff(
    db: &config::GameDB,
    id: i32,
    evidence: &Evidence,
    wire_evidence: &crate::wire_evidence::Evidence,
) -> Option<Buff> {
    let row = db.skill_buff.get(id)?;
    let acts = row
        .features
        .split('|')
        .filter(|raw| !raw.trim().is_empty())
        .filter_map(|raw| buff_act(db, raw, wire_evidence))
        .collect();
    Some(Buff {
        id,
        name: localized(db, &row.name),
        description: localized(db, &row.desc),
        duration: row.during_time,
        type_id: row.type_id,
        observed: evidence.buff(id),
        acts,
    })
}

fn buff_act(
    db: &config::GameDB,
    raw: &str,
    wire_evidence: &crate::wire_evidence::Evidence,
) -> Option<BuffAct> {
    let values = split_ids(raw);
    let opcode = values.first().copied()?;
    let row = db.buff_act.get(opcode)?;
    let definition = buff_act_registry::find(opcode, &row.r#type);
    let args = &values[1..];
    let destination =
        buff_act_registry::destination_with_raw(Some(db), opcode, &row.r#type, args, Some(raw));
    let semantic = match definition {
        None => "route missing",
        Some(definition)
            if definition
                .raw_supports
                .is_some_and(|supports| !supports(Some(db), raw)) =>
        {
            "unsupported arguments"
        }
        Some(definition) if definition.supports.is_some_and(|supports| !supports(args)) => {
            "unsupported arguments"
        }
        Some(_) if destination.is_none() => "no semantic owner",
        Some(_) => "supported",
    };
    Some(BuffAct {
        node: Node {
            opcode: Some(opcode),
            type_name: row.r#type.clone(),
            raw: raw.to_owned(),
            registry: if definition.is_some() {
                "exact"
            } else {
                "missing"
            },
            semantic,
            detail: definition
                .map(|definition| format!("kind={:?}", definition.kind))
                .unwrap_or_default(),
            observation: if wire_evidence.observed_act(opcode, &row.r#type) {
                "Exact wire marker observed"
            } else {
                "Unexercised"
            },
        },
        effect_time: row.effect_time,
        event: buff_act_registry::runtime_event(opcode, &row.r#type, row.effect_time).map_or_else(
            || format!("{:?}", effect_time::classify(row.effect_time)),
            |event| format!("{event:?}"),
        ),
        destination: destination.map_or_else(|| "—".to_owned(), |value| format!("{value:?}")),
        owns_duration: buff_act_registry::owns_duration(opcode, &row.r#type),
        wire: if wire::find(opcode, &row.r#type).is_some() {
            "mapped"
        } else {
            "missing"
        },
    })
}

fn field_string(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn field_i32(value: &serde_json::Value, key: &str) -> i32 {
    value
        .get(key)
        .and_then(serde_json::Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .unwrap_or_default()
}

fn localized(db: &config::GameDB, value: &str) -> String {
    clean_rich_text(db.language_en.get(value).unwrap_or(value))
}

fn clean_rich_text(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find('<') {
        output.push_str(&rest[..start]);
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('>') else {
            output.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let tag = after_start[..end].trim();
        if is_rich_text_tag(tag) {
            if tag
                .trim_start_matches('/')
                .trim_end_matches('/')
                .trim()
                .eq_ignore_ascii_case("br")
            {
                output.push(' ');
            }
            rest = &after_start[end + 1..];
        } else {
            output.push('<');
            rest = after_start;
        }
    }
    output.push_str(rest);
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_rich_text_tag(tag: &str) -> bool {
    let tag = tag.trim_start_matches('/').trim_end_matches('/').trim();
    let name = tag
        .split([':', '='])
        .next()
        .unwrap_or(tag)
        .to_ascii_lowercase();
    matches!(
        name.as_str(),
        "alpha"
            | "b"
            | "br"
            | "color"
            | "font"
            | "i"
            | "id"
            | "link"
            | "nobr"
            | "s"
            | "size"
            | "sprite"
            | "sub"
            | "sup"
            | "u"
    )
}

fn stage_name(db: &config::GameDB, episode: &config::episode::Episode) -> String {
    let english = localized(db, &episode.name_en);
    let title = if !english.is_empty() {
        english.clone()
    } else {
        let name = localized(db, &episode.name);
        if name.is_empty() {
            format!("Episode {}", episode.id)
        } else {
            name
        }
    };
    if (101..=113).contains(&episode.chapter_id) {
        format!(
            "{}-{} — {title}",
            episode.chapter_id % 100,
            episode.id % 100
        )
    } else if (201..=213).contains(&episode.chapter_id) {
        let normal = db.episode.iter().find(|candidate| {
            candidate.chapter_id == episode.chapter_id - 100
                && localized(db, &candidate.name_en) == english
        });
        normal.map_or_else(
            || format!("Hard {} — {title}", episode.id),
            |normal| {
                format!(
                    "Hard {}-{} — {title}",
                    normal.chapter_id % 100,
                    normal.id % 100
                )
            },
        )
    } else {
        title
    }
}

fn slug(value: &str) -> String {
    let mut output = String::new();
    let mut dash = false;
    for character in value.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            output.push(character);
            dash = false;
        } else if !output.is_empty() && !dash {
            output.push('-');
            dash = true;
        }
    }
    output.trim_end_matches('-').to_owned()
}

#[cfg(test)]
mod text_tests {
    use super::*;

    #[test]
    fn removes_client_rich_text_without_joining_words() {
        assert_eq!(
            clean_rich_text("<i>Lady by the Lake</i>"),
            "Lady by the Lake"
        );
        assert_eq!(
            clean_rich_text("In [Silence<id:3136>],<br>gains <nobr>2 stacks</nobr>."),
            "In [Silence], gains 2 stacks."
        );
        assert_eq!(clean_rich_text("HP < 50%"), "HP < 50%");
    }

    #[test]
    fn synthetic_any_is_omitted_and_raw_expression_is_preserved() {
        let child = ParsedCondition {
            opcode: 999_999,
            type_name: "MissingExactCondition".to_owned(),
            kind: ParsedConditionKind::Unsupported("test".to_owned()),
            raw_args: vec!["1".to_owned()],
        };
        let wrapper = ParsedCondition {
            opcode: 0,
            type_name: "Any".to_owned(),
            kind: ParsedConditionKind::Any(vec![vec![child.clone()]]),
            raw_args: Vec::new(),
        };

        let conditions = [wrapper];
        let flattened = flatten_conditions(&conditions);
        assert_eq!(flattened, vec![&child]);
        let node = condition_node(flattened[0], "999999#1!|5", false);
        assert_eq!(node.raw, "999999#1!|5");
    }

    #[test]
    fn narrowed_reports_reject_unknown_subjects() {
        crate::init_config().unwrap();
        let db = config::configs::get();
        let evidence = Evidence::default();
        let wire_evidence = crate::wire_evidence::Evidence::default();

        assert!(
            heroes(
                db,
                battle::catalog::BattleCatalog::new(db),
                &[-1],
                &evidence,
                &wire_evidence,
            )
            .is_err()
        );
        assert!(
            psychubes(
                db,
                battle::catalog::BattleCatalog::new(db),
                Some(-1),
                &evidence,
                &wire_evidence,
            )
            .is_err()
        );
        assert!(
            stages(
                db,
                battle::catalog::BattleCatalog::new(db),
                Some(-1),
                &evidence,
                &wire_evidence,
            )
            .is_err()
        );
    }

    #[test]
    fn grouped_skill_replacement_reports_raw_aware_semantic_support() {
        crate::init_config().unwrap();
        let db = config::get();
        let wire_evidence = crate::wire_evidence::Evidence::default();

        let supported = buff_act(
            db,
            "1138#1:31460211,31460212,31460213#2:31460221,31460222,31460223",
            &wire_evidence,
        )
        .unwrap();
        assert_eq!(supported.node.semantic, "supported");

        for raw in [
            "1138#1:31460211#2:31460221",
            "1138#1:30120111,30120112,30120113#2:30120121,30120122,30120123",
        ] {
            assert_eq!(
                buff_act(db, raw, &wire_evidence).unwrap().node.semantic,
                "unsupported arguments",
                "{raw}"
            );
        }
    }
}
