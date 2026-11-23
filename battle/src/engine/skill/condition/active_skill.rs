use crate::engine::skill::condition::parse::{
    ConditionCompare, ParsedConditionKind, first_i32, parse_i32_list,
};

pub fn active_use(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::ActiveUseSkill {
        slot: first_i32(args)?,
    })
}

pub fn active_ally_use(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::ActiveAllyUseSkill {
        slot: first_i32(args)?,
    })
}

pub fn active_source_use(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::ActiveUseSkillBySource {
        slot: first_i32(args)?,
    })
}

pub fn use_skill(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::UseSkillRank(parse_i32_list(
        args.first()?,
    )?))
}

pub fn incantation_rank(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [raw_rank] = args else {
        return None;
    };
    let rank = raw_rank.parse().ok()?;
    matches!(rank, 2 | 3).then_some(ParsedConditionKind::UseSkillRank(vec![rank]))
}

pub fn hurt_skill(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    args.is_empty().then_some(ParsedConditionKind::UseHurtSkill)
}

pub fn device_hurt_skill(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    args.is_empty()
        .then_some(ParsedConditionKind::UseDeviceSkill)
}

pub fn skill_id(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::ActiveSkillId(parse_i32_list(
        args.first()?,
    )?))
}

pub fn can_use_skill(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::CanUseSkill(parse_i32_list(
        args.first()?,
    )?))
}

pub fn specific_skill(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let (group, rank) = match args {
        [group] if group.parse::<i32>().ok() == Some(4) => (4, 0),
        [group, rank] => (group.parse().ok()?, rank.parse().ok()?),
        _ => return None,
    };
    matches!(
        (group, rank),
        (0, 2 | 3) | (1, 0 | 2 | 3) | (2, 0 | 2 | 3) | (3, 0 | 1 | 3) | (4, 0..=3) | (5, 0..=3)
    )
    .then_some(ParsedConditionKind::SpecificSkill { group, rank })
}

pub fn received_specific_skill(
    opcode: i32,
    type_name: &str,
    args: &[String],
) -> Option<ParsedConditionKind> {
    let ParsedConditionKind::SpecificSkill { group, rank } =
        specific_skill(opcode, type_name, args)?
    else {
        return None;
    };
    Some(ParsedConditionKind::ReceivedSpecificSkill { group, rank })
}

pub fn skill_type(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::ActiveSkillType(first_i32(args)?))
}

pub fn effect_tag(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::ActiveSkillEffectTag(parse_i32_list(
        args.first()?,
    )?))
}

pub fn effect_tags(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let mut tags = Vec::new();
    for arg in args {
        tags.extend(parse_i32_list(arg)?);
    }
    (!tags.is_empty()).then_some(ParsedConditionKind::ActiveSkillEffectTag(tags))
}

pub fn single_effect_tag(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [raw] = args else {
        return None;
    };
    let tag = raw.parse().ok()?;
    (1..=6)
        .contains(&tag)
        .then_some(ParsedConditionKind::ActiveSkillEffectTag(vec![tag]))
}

pub fn rank(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::ActiveSkillRank {
        compare: ConditionCompare::Equal,
        ranks: parse_i32_list(args.first()?)?,
    })
}

#[cfg(test)]
mod tests;
