use std::{collections::HashMap, fs, path::Path};

use battle::engine::entity::stats::{StatInputs, Stats, rank_from_level};
use sonettobuf::{
    Fight, FightEntityInfo, HeroExAttribute, HeroInfo, HeroInfoListReply, HeroSpAttribute,
    HeroUpdatePush,
};

use crate::normalize_live_json;

type PreviewAttributes = (Vec<(i64, HeroExAttribute)>, Vec<(i64, HeroSpAttribute)>);

enum HeroMetadata {
    Roster(HeroInfo),
    Update(HeroInfo),
}

#[derive(Debug, PartialEq, Eq)]
struct MissingAttackerMetadata {
    uid: i64,
    model_id: i32,
}

pub fn preview_attributes(fight: &Fight, battle_path: &Path) -> anyhow::Result<PreviewAttributes> {
    let local = battle_hero_metadata(battle_path)?;
    let (attributes, missing) = hydrate_preview_attributes(fight, &local);
    for missing in &missing {
        eprintln!(
            "attribute preview missing attacker metadata uid={} hero={}; using battle defaults",
            missing.uid, missing.model_id,
        );
    }
    Ok(attributes)
}

fn hydrate_preview_attributes(
    fight: &Fight,
    local: &HashMap<i64, HeroMetadata>,
) -> (PreviewAttributes, Vec<MissingAttackerMetadata>) {
    let mut ex_attributes = Vec::new();
    let mut sp_attributes = Vec::new();
    let mut missing = Vec::new();
    for entity in fight
        .attacker
        .iter()
        .flat_map(|team| team.entitys.iter().chain(team.sub_entitys.iter()))
    {
        let Some(uid) = entity.uid else {
            continue;
        };
        match local.get(&uid) {
            Some(HeroMetadata::Roster(hero)) => {
                if let Some(attributes) = hero.ex_attr {
                    ex_attributes.push((uid, attributes));
                }
                if let Some(attributes) = hero.sp_attr {
                    sp_attributes.push((uid, attributes));
                }
                if hero.ex_attr.is_none() || hero.sp_attr.is_none() {
                    missing.push(MissingAttackerMetadata {
                        uid,
                        model_id: entity.model_id.unwrap_or_default(),
                    });
                } else if battle::engine::diagnostics::enabled(
                    battle::engine::diagnostics::TraceArea::Damage,
                ) {
                    eprintln!(
                        "attribute preview uid={uid} hero={} source=hero-roster",
                        entity.model_id.unwrap_or_default(),
                    );
                }
            }
            Some(HeroMetadata::Update(hero)) => {
                let inputs = preview_stat_inputs(entity, hero);
                let stats = Stats::build(&inputs);
                if battle::engine::diagnostics::enabled(
                    battle::engine::diagnostics::TraceArea::Damage,
                ) {
                    eprintln!(
                        "attribute preview uid={uid} hero={} source=hero-update inputs={inputs:?} stats={stats:?}",
                        entity.model_id.unwrap_or_default(),
                    );
                }
                ex_attributes.push((uid, stats.ex()));
                sp_attributes.push((uid, stats.sp()));
            }
            None => missing.push(MissingAttackerMetadata {
                uid,
                model_id: entity.model_id.unwrap_or_default(),
            }),
        }
    }
    ((ex_attributes, sp_attributes), missing)
}

fn battle_hero_metadata(path: &Path) -> anyhow::Result<HashMap<i64, HeroMetadata>> {
    let Some(parent) = path.parent() else {
        return Ok(HashMap::new());
    };
    let mut roster_files = Vec::new();
    let mut update_files = Vec::new();
    for path in fs::read_dir(parent)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
    {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.starts_with("HeroInfoListReply") && name.ends_with(".json") {
            roster_files.push(path);
        } else if name.starts_with("HeroUpdatePush") && name.ends_with(".json") {
            update_files.push(path);
        }
    }
    roster_files.sort();
    update_files.sort();

    let mut heroes = HashMap::new();
    for file in roster_files {
        let mut value: serde_json::Value = serde_json::from_str(&fs::read_to_string(file)?)?;
        normalize_live_json(&mut value);
        let roster: HeroInfoListReply = serde_json::from_value(value)?;
        for hero in roster.heros {
            heroes.insert(hero.uid, HeroMetadata::Roster(hero));
        }
    }
    for file in update_files {
        let mut value: serde_json::Value = serde_json::from_str(&fs::read_to_string(file)?)?;
        normalize_live_json(&mut value);
        let update: HeroUpdatePush = serde_json::from_value(value)?;
        for hero in update.hero_updates {
            heroes.insert(hero.uid, HeroMetadata::Update(hero));
        }
    }
    Ok(heroes)
}

fn preview_stat_inputs(entity: &FightEntityInfo, hero: &HeroInfo) -> StatInputs {
    let selected_template = hero.use_talent_template_id.unwrap_or_default();
    let template = (selected_template != 0)
        .then(|| {
            hero.talent_templates
                .iter()
                .find(|template| template.id == Some(selected_template))
        })
        .flatten();
    let cubes = template
        .filter(|template| !template.talent_cube_infos.is_empty())
        .map(|template| template.talent_cube_infos.as_slice())
        .unwrap_or(&hero.talent_cube_infos);
    let equip = entity.equips.first();
    StatInputs {
        hero_id: entity.model_id.unwrap_or(hero.hero_id),
        level: entity.level.or(hero.level).unwrap_or_default(),
        rank: hero.rank.unwrap_or_else(|| {
            rank_from_level(
                entity.model_id.unwrap_or(hero.hero_id),
                entity.level.unwrap_or_default(),
            )
        }),
        destiny_rank: entity
            .destiny_rank
            .or(hero.destiny_rank)
            .unwrap_or_default(),
        equip_id: equip.and_then(|equip| equip.equip_id).unwrap_or_default(),
        equip_level: equip.and_then(|equip| equip.equip_lv).unwrap_or_default(),
        equip_break_level: 0,
        talent: hero.talent.unwrap_or(10),
        talent_style: template
            .and_then(|template| template.style)
            .unwrap_or_default(),
        talent_placements: cubes.iter().filter_map(|cube| cube.cube_id).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonettobuf::{EquipRecord, FightTeam, TalentCubeInfo};

    fn test_directory(label: &str) -> std::path::PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "enigma-preview-attributes-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        directory
    }

    fn fight(uid: i64) -> Fight {
        Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(uid),
                    model_id: Some(3149),
                    equips: vec![
                        EquipRecord {
                            equip_id: Some(1571),
                            ..Default::default()
                        },
                        EquipRecord {
                            equip_id: Some(1572),
                            ..Default::default()
                        },
                    ],
                    passive_skill: vec![437111, 437215],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn hero(uid: i64, critical_damage: i32) -> HeroInfo {
        HeroInfo {
            uid,
            user_id: 1,
            hero_id: 3149,
            ex_attr: Some(HeroExAttribute {
                cri: Some(385),
                recri: Some(100),
                cri_dmg: Some(critical_damage),
                cri_def: Some(0),
                add_dmg: Some(85),
                drop_dmg: Some(45),
            }),
            sp_attr: Some(HeroSpAttribute {
                device_skill_rate: Some(70),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn roster_fallback_uses_exact_attributes_without_changing_fight_loadout() {
        let directory = test_directory("roster");
        let uid = 42;
        fs::write(
            directory.join("HeroInfoListReply_1.json"),
            serde_json::to_vec(&HeroInfoListReply {
                heros: vec![hero(uid, 1485)],
                ..Default::default()
            })
            .unwrap(),
        )
        .unwrap();
        let battle_path = directory.join("BeginRoundReply_1.json");
        let fight = fight(uid);

        let (extended, special) = preview_attributes(&fight, &battle_path).unwrap();

        assert_eq!(extended, vec![(uid, hero(uid, 1485).ex_attr.unwrap())]);
        assert_eq!(special, vec![(uid, hero(uid, 1485).sp_attr.unwrap())]);
        let entity = &fight.attacker.as_ref().unwrap().entitys[0];
        assert_eq!(entity.equips.len(), 2);
        assert_eq!(entity.passive_skill, vec![437111, 437215]);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn sorted_hero_updates_override_roster_metadata() {
        crate::init_test_config();
        let directory = test_directory("precedence");
        let uid = 42;
        let mut earlier_update = hero(uid, 1666);
        earlier_update.level = Some(1);
        earlier_update.rank = Some(0);
        earlier_update.talent_cube_infos = vec![TalentCubeInfo {
            cube_id: Some(62),
            ..Default::default()
        }];
        let mut later_update = hero(uid, 1777);
        later_update.level = Some(60);
        later_update.rank = Some(5);
        later_update.talent_cube_infos = vec![TalentCubeInfo {
            cube_id: Some(61),
            ..Default::default()
        }];
        fs::write(
            directory.join("HeroInfoListReply_1.json"),
            serde_json::to_vec(&HeroInfoListReply {
                heros: vec![hero(uid, 1485)],
                ..Default::default()
            })
            .unwrap(),
        )
        .unwrap();
        fs::write(
            directory.join("HeroUpdatePush_2.json"),
            serde_json::to_vec(&HeroUpdatePush {
                hero_updates: vec![later_update.clone()],
            })
            .unwrap(),
        )
        .unwrap();
        fs::write(
            directory.join("HeroUpdatePush_1.json"),
            serde_json::to_vec(&HeroUpdatePush {
                hero_updates: vec![earlier_update.clone()],
            })
            .unwrap(),
        )
        .unwrap();

        let fight = fight(uid);
        let battle_path = directory.join("BeginRoundReply_1.json");
        let (extended, special) = preview_attributes(&fight, &battle_path).unwrap();
        let entity = &fight.attacker.as_ref().unwrap().entitys[0];
        let expected = Stats::build(&preview_stat_inputs(entity, &later_update));
        let earlier = Stats::build(&preview_stat_inputs(entity, &earlier_update));

        assert_eq!(extended, vec![(uid, expected.ex())]);
        assert_eq!(special, vec![(uid, expected.sp())]);
        assert_ne!(extended, vec![(uid, earlier.ex())]);
        assert_ne!(extended, vec![(uid, hero(uid, 1485).ex_attr.unwrap())]);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn missing_attacker_metadata_is_reported_without_failing() {
        let uid = 42;
        let (attributes, missing) = hydrate_preview_attributes(&fight(uid), &HashMap::new());

        assert_eq!(attributes, (Vec::new(), Vec::new()));
        assert_eq!(
            missing,
            vec![MissingAttackerMetadata {
                uid,
                model_id: 3149
            }]
        );
    }

    #[cfg(feature = "private-fixtures")]
    #[test]
    fn battle_local_talent_placements_reconstruct_captured_attributes() {
        crate::init_test_config();
        let battle = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/battles/battle2/begin_round_1.json");
        let heroes = battle_hero_metadata(&battle).unwrap();
        let mut start: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(battle.with_file_name("StartDungeonReply.json")).unwrap(),
        )
        .unwrap();
        normalize_live_json(&mut start);
        let fight: Fight = serde_json::from_value(start["fight"].clone()).unwrap();

        for entity in fight.attacker.iter().flat_map(|team| &team.entitys) {
            let uid = entity.uid.unwrap();
            let Some(HeroMetadata::Update(hero)) = heroes.get(&uid) else {
                continue;
            };
            let stats = Stats::build(&preview_stat_inputs(entity, hero));
            let generated = stats.ex();
            let captured = hero.ex_attr.as_ref().unwrap();
            let generated_sp = stats.sp();
            let captured_sp = hero.sp_attr.as_ref().unwrap();
            let base = entity.attr.as_ref().unwrap();

            assert_eq!(stats.hp, base.hp.unwrap());
            assert_eq!(stats.atk, base.attack.unwrap());
            assert_eq!(stats.def, base.defense.unwrap());
            assert_eq!(stats.mdef, base.mdefense.unwrap());
            assert_eq!(stats.technic, base.technic.unwrap());

            assert_eq!(generated.cri, captured.cri);
            assert_eq!(generated.recri, captured.recri);
            assert_eq!(generated.cri_dmg, captured.cri_dmg);
            assert_eq!(generated.cri_def, captured.cri_def);
            assert_eq!(generated.add_dmg, captured.add_dmg);
            assert_eq!(generated.drop_dmg, captured.drop_dmg);
            // HeroUpdate does not fold destiny-stone attributes into SpAttr.
            if entity.destiny_rank.unwrap_or_default() == 0 {
                assert_eq!(
                    (
                        generated_sp.revive,
                        generated_sp.heal,
                        generated_sp.absorb,
                        generated_sp.defense_ignore,
                        generated_sp.clutch,
                        generated_sp.normal_skill_rate,
                        generated_sp.rebound_dmg,
                        generated_sp.extra_dmg,
                        generated_sp.reuse_dmg,
                    ),
                    (
                        captured_sp.revive,
                        captured_sp.heal,
                        captured_sp.absorb,
                        captured_sp.defense_ignore,
                        captured_sp.clutch,
                        captured_sp.normal_skill_rate,
                        captured_sp.rebound_dmg,
                        captured_sp.extra_dmg,
                        captured_sp.reuse_dmg,
                    ),
                    "special attributes differ for uid={uid}",
                );
            }
        }
    }

    #[cfg(feature = "private-fixtures")]
    #[test]
    fn tutorial_trial_without_a_hero_update_does_not_invent_extended_attributes() {
        crate::init_test_config();
        let battle = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/battles/battle62/BeginRoundReply_1.json");
        let mut start: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(battle.with_file_name("StartDungeonReply.json")).unwrap(),
        )
        .unwrap();
        normalize_live_json(&mut start);
        let fight: Fight = serde_json::from_value(start["fight"].clone()).unwrap();

        let (extended, special) = preview_attributes(&fight, &battle).unwrap();

        assert!(extended.is_empty());
        assert!(special.is_empty());
    }
}
