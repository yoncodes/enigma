use super::parse::{ParsedConditionKind, TargetIdentityMode, first_i32, parse_i32, parse_i32_list};

pub fn active_skill_source(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    args.is_empty()
        .then_some(ParsedConditionKind::ActiveSkillSourceIsTarget)
}

pub fn ex_skill_level(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [level] = args else {
        return None;
    };
    let level = level.parse().ok()?;
    (0..=5)
        .contains(&level)
        .then_some(ParsedConditionKind::ExSkillLevel(level))
}

pub fn ex_skill_levels(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [raw_levels] = args else {
        return None;
    };
    match raw_levels.as_str() {
        "0,1,2,3,4" => Some(ParsedConditionKind::ExSkillLevels(vec![0, 1, 2, 3, 4])),
        "5" => Some(ParsedConditionKind::ExSkillLevels(vec![5])),
        _ => None,
    }
}

pub fn target_is_self(_: i32, _: &str, _: &[String]) -> Option<ParsedConditionKind> {
    identity(TargetIdentityMode::TargetIsSelf, 0)
}

pub fn target_is_ally_not_self(_: i32, _: &str, _: &[String]) -> Option<ParsedConditionKind> {
    identity(TargetIdentityMode::TargetIsAllyNotSelf, 0)
}

pub fn target_model(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    identity(TargetIdentityMode::TargetModelId, first_i32(args)?)
}

pub fn positive_target_model(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    identity(TargetIdentityMode::TargetModelId, positive_model_id(args)?)
}

pub fn condition_target_model(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    identity(
        TargetIdentityMode::ConditionTargetModelId,
        positive_model_id(args)?,
    )
}

pub fn positive_active_skill_source_model(
    _: i32,
    _: &str,
    args: &[String],
) -> Option<ParsedConditionKind> {
    identity(
        TargetIdentityMode::ActiveSkillSourceModelId,
        positive_model_id(args)?,
    )
}

fn positive_model_id(args: &[String]) -> Option<i32> {
    let [model_id] = args else {
        return None;
    };
    let model_id = parse_i32(model_id)?;
    (model_id > 0).then_some(model_id)
}

pub fn team_contains_model(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::TeamContainsModels(parse_i32_list(
        args.first()?,
    )?))
}

pub fn team_model_presence(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [entity_type, model_ids, scope, mode] = args else {
        return None;
    };
    // This exact opcode is a hero-roster gate. Other HasConditionTarget
    // opcodes carry different entity/scope semantics and need their own routes.
    if entity_type != "6" || scope != "0" {
        return None;
    }
    // Master Assassin: "If there are other [Assassination] characters on the team".
    if mode != "1" {
        return None;
    }
    let model_ids = model_ids
        .split(',')
        .map(|value| value.trim().parse().ok())
        .collect::<Option<Vec<_>>>()?;
    (!model_ids.is_empty()).then_some(ParsedConditionKind::TeamModelPresence {
        model_ids,
        present: true,
    })
}

fn identity(mode: TargetIdentityMode, value: i32) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::TargetIdentity { mode, value })
}

#[cfg(test)]
mod tests;
