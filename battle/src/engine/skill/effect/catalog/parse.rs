use super::*;

pub(super) fn numeric_ids(raw: &str) -> impl Iterator<Item = i32> + '_ {
    raw.split(|character: char| !character.is_ascii_digit() && character != '-')
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse().ok())
        .filter(|id| *id > 0)
}

pub fn configured_effect_id(skill_id: i32) -> i32 {
    config::try_get()
        .and_then(|db| db.skill.get(skill_id))
        .map(|skill| skill.skill_effect)
        .filter(|id| *id != 0)
        .unwrap_or(skill_id)
}

fn configured_effect(skill_id: i32) -> Option<&'static config::skill_effect::SkillEffect> {
    let db = config::try_get()?;
    let effect_id = configured_effect_id(skill_id);
    db.skill_effect.get(effect_id)
}

pub fn configured_extra_kind(skill_id: i32) -> i32 {
    configured_effect(skill_id)
        .map(|effect| effect.is_extra)
        .unwrap_or_default()
}

pub fn configured_big_skill_point(skill_id: i32) -> i32 {
    configured_effect(skill_id)
        .map(|effect| effect.big_skill_point)
        .unwrap_or_default()
}

pub fn configured_is_big_skill(skill_id: i32) -> bool {
    configured_effect(skill_id).is_some_and(|effect| effect.is_big_skill != 0)
}

pub fn configured_skill_type(skill_id: i32) -> i32 {
    configured_effect(skill_id)
        .map(|effect| effect.r#type)
        .unwrap_or_default()
}

pub fn configured_effect_tag(skill_id: i32) -> i32 {
    configured_effect(skill_id)
        .map(|effect| effect.effect_tag)
        .unwrap_or_default()
}

pub fn configured_is_attack(skill_id: i32) -> bool {
    configured_effect(skill_id).is_some_and(|effect| {
        effect.damage_rate > 0
            || matches!(
                effect.effect_tag,
                tag if tag == SkillEffectTag::RealityDamage as i32
                    || tag == SkillEffectTag::MentalDamage as i32
            )
    })
}

pub(super) fn rule_issue(db: &GameDB, effect_id: i32, slot: u8, raw: &str) -> RuleIssue {
    let opcode = parse_parts(raw)
        .first()
        .and_then(|value| value.parse::<i32>().ok());
    let behavior = opcode.and_then(|opcode| db.skill_behavior.get(opcode));
    let type_name = behavior.map(|row| row.r#type.clone());
    let reason = match (opcode, behavior) {
        (None, _) => RuleIssueReason::MalformedBehavior,
        (Some(_), None) => RuleIssueReason::MissingBehavior,
        (Some(opcode), Some(row))
            if BehaviorSpec::new(opcode, row.r#type.clone()).kind == BehaviorKind::Unknown =>
        {
            RuleIssueReason::UnsupportedBehavior
        }
        _ => RuleIssueReason::MalformedBehavior,
    };

    RuleIssue {
        effect_id,
        slot,
        opcode,
        type_name,
        raw: raw.to_owned(),
        reason,
    }
}

/// Returns the process-wide catalog used by config-backed tests and tools.
pub fn global() -> &'static SkillEffectCatalog {
    use std::sync::OnceLock;
    static CATALOG: OnceLock<SkillEffectCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| SkillEffectCatalog::from_game_db(config::configs::get()))
}

pub(super) struct RawSlot<'a> {
    pub(super) behavior: &'a str,
    pub(super) target: &'a str,
    pub(super) condition: &'a str,
    pub(super) condition_target: &'a str,
    pub(super) logic_target: &'a str,
    pub(super) limit: i32,
    pub(super) round_limit: i32,
}

pub(super) fn parse_slot(db: &GameDB, raw: RawSlot<'_>) -> Option<SkillEffectSlot> {
    let RawSlot {
        behavior,
        target,
        condition,
        condition_target,
        logic_target,
        limit,
        round_limit,
    } = raw;
    let mut behavior_values = parse_parts(behavior);
    let opcode = behavior_values.first()?.parse().ok()?;
    behavior_values.remove(0);
    let behavior_row = db.skill_behavior.get(opcode)?;
    let spec = BehaviorSpec::new(opcode, behavior_row.r#type.clone());

    if spec.kind == BehaviorKind::Unknown {
        return None;
    }

    let mut behavior_args = Vec::new();
    let mut has_registered_syntax = false;
    for value in &behavior_values {
        if let Ok(value) = value.parse() {
            behavior_args.push(value);
        } else if !value.contains(',')
            || value
                .split(',')
                .any(|part| part.trim().parse::<i32>().is_err())
        {
            has_registered_syntax = true;
        }
    }

    let behavior = ParsedBehavior::from_spec(spec, behavior_args, behavior_values);
    if has_registered_syntax
        && !crate::engine::skill::behavior::registry::find(&behavior)
            .and_then(|definition| definition.supports)
            .is_some_and(|supports| supports(&behavior))
    {
        return None;
    }

    let conditions = parse_conditions(db, condition);
    let round_limit = crate::engine::skill::condition::lifecycle::team_entity_exit_limit(
        &conditions,
    )
    .map_or(round_limit, |condition_limit| {
        if round_limit > 0 {
            round_limit.min(condition_limit)
        } else {
            condition_limit
        }
    });
    let compiled_route = crate::engine::skill::rule::route::ConditionRoute::compile_for_behavior(
        &conditions,
        &behavior.spec,
    );
    let condition_target = parse_target(condition_target);
    let parsed_target = parse_target(target);
    let target_from_condition = parsed_target.code == 999;
    let target = match parsed_target {
        target if target.code == 999 && condition_target.code != 0 => condition_target.clone(),
        target if target.code == 999 => parse_target(logic_target),
        target => target,
    };

    Some(SkillEffectSlot {
        behavior,
        conditions,
        compiled_route,
        condition_target,
        target,
        target_from_condition,
        limit,
        round_limit,
    })
}

pub(super) fn parse_target(raw: &str) -> TargetRequest {
    let mut values = parse_i32_list(raw);
    let code = values.first().copied().unwrap_or_default();
    if !values.is_empty() {
        values.remove(0);
    }

    TargetRequest { code, raw: values }
}

pub(super) fn parse_i32_list(raw: &str) -> Vec<i32> {
    parse_parts(raw)
        .into_iter()
        .filter_map(|part| part.parse().ok())
        .collect()
}

fn parse_parts(raw: &str) -> Vec<String> {
    raw.split('#')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}

pub(super) fn monster_model_skills(db: &GameDB, model_id: i32) -> Vec<i32> {
    let Some(monster) = db.monster.get(model_id) else {
        return Vec::new();
    };
    let Some(template) = db.monster_skill_template.get(monster.skill_template) else {
        return Vec::new();
    };
    let mut skills = crate::engine::entity::skill::parse_skill_group(&template.active_skill, 1);
    skills.extend(crate::engine::entity::skill::parse_skill_group(
        &template.active_skill,
        2,
    ));
    skills.extend(
        crate::engine::entity::skill::split_ids(&template.passive_skill)
            .into_iter()
            .take(monster.passive_skill_count.max(0) as usize),
    );
    skills.extend(crate::engine::entity::skill::split_ids(
        &monster.passive_skills_ex,
    ));
    skills.extend(
        template
            .unique_skill
            .split('#')
            .next()
            .and_then(|value| value.parse::<i32>().ok()),
    );
    skills.retain(|skill_id| *skill_id > 0 && db.skill.get(*skill_id).is_some());
    skills
}

pub(super) fn row_slots(
    row: &config::skill_effect::SkillEffect,
) -> [(&str, &str, &str, &str, i32, i32); 20] {
    [
        (
            &row.behavior1,
            &row.behavior_target1,
            &row.condition1,
            &row.condition_target1,
            row.limit1,
            row.round_limit1,
        ),
        (
            &row.behavior2,
            &row.behavior_target2,
            &row.condition2,
            &row.condition_target2,
            row.limit2,
            row.round_limit2,
        ),
        (
            &row.behavior3,
            &row.behavior_target3,
            &row.condition3,
            &row.condition_target3,
            row.limit3,
            row.round_limit3,
        ),
        (
            &row.behavior4,
            &row.behavior_target4,
            &row.condition4,
            &row.condition_target4,
            row.limit4,
            row.round_limit4,
        ),
        (
            &row.behavior5,
            &row.behavior_target5,
            &row.condition5,
            &row.condition_target5,
            row.limit5,
            row.round_limit5,
        ),
        (
            &row.behavior6,
            &row.behavior_target6,
            &row.condition6,
            &row.condition_target6,
            row.limit6,
            row.round_limit6,
        ),
        (
            &row.behavior7,
            &row.behavior_target7,
            &row.condition7,
            &row.condition_target7,
            row.limit7,
            row.round_limit7,
        ),
        (
            &row.behavior8,
            &row.behavior_target8,
            &row.condition8,
            &row.condition_target8,
            row.limit8,
            row.round_limit8,
        ),
        (
            &row.behavior9,
            &row.behavior_target9,
            &row.condition9,
            &row.condition_target9,
            row.limit9,
            row.round_limit9,
        ),
        (
            &row.behavior10,
            &row.behavior_target10,
            &row.condition10,
            &row.condition_target10,
            row.limit10,
            row.round_limit10,
        ),
        (
            &row.behavior11,
            &row.behavior_target11,
            &row.condition11,
            &row.condition_target11,
            row.limit11,
            row.round_limit11,
        ),
        (
            &row.behavior12,
            &row.behavior_target12,
            &row.condition12,
            &row.condition_target12,
            row.limit12,
            row.round_limit12,
        ),
        (
            &row.behavior13,
            &row.behavior_target13,
            &row.condition13,
            &row.condition_target13,
            row.limit13,
            row.round_limit13,
        ),
        (
            &row.behavior14,
            &row.behavior_target14,
            &row.condition14,
            &row.condition_target14,
            row.limit14,
            row.round_limit14,
        ),
        (
            &row.behavior15,
            &row.behavior_target15,
            &row.condition15,
            &row.condition_target15,
            row.limit15,
            row.round_limit15,
        ),
        (
            &row.behavior16,
            &row.behavior_target16,
            &row.condition16,
            &row.condition_target16,
            row.limit16,
            row.round_limit16,
        ),
        (
            &row.behavior17,
            &row.behavior_target17,
            &row.condition17,
            &row.condition_target17,
            row.limit17,
            row.round_limit17,
        ),
        (
            &row.behavior18,
            &row.behavior_target18,
            &row.condition18,
            &row.condition_target18,
            row.limit18,
            row.round_limit18,
        ),
        (
            &row.behavior19,
            &row.behavior_target19,
            &row.condition19,
            &row.condition_target19,
            row.limit19,
            row.round_limit19,
        ),
        (
            &row.behavior20,
            &row.behavior_target20,
            &row.condition20,
            &row.condition_target20,
            row.limit20,
            row.round_limit20,
        ),
    ]
}
