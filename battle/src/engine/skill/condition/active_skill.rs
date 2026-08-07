use crate::engine::skill::condition::parse::{
    ConditionCompare, ParsedConditionKind, first_i32, parse_i32_list,
};

pub fn active_use(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::ActiveUseSkill {
        slot: first_i32(args)?,
    })
}

pub fn use_skill(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::UseSkillRank(parse_i32_list(
        args.first()?,
    )?))
}

pub fn hurt_skill(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    args.is_empty().then_some(ParsedConditionKind::UseHurtSkill)
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

pub fn rank(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::ActiveSkillRank {
        compare: ConditionCompare::Equal,
        ranks: parse_i32_list(args.first()?)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specific_skill_preserves_group_and_rank_selectors() {
        assert_eq!(
            specific_skill(66203, "UseSpecificSkill", &["1".into(), "0".into()]),
            Some(ParsedConditionKind::SpecificSkill { group: 1, rank: 0 })
        );
        assert_eq!(
            specific_skill(66210, "UseSpecificSkill", &["4".into()]),
            Some(ParsedConditionKind::SpecificSkill { group: 4, rank: 0 })
        );
        assert_eq!(
            specific_skill(66203, "UseSpecificSkill", &["5".into(), "0".into()]),
            Some(ParsedConditionKind::SpecificSkill { group: 5, rank: 0 })
        );
        assert!(specific_skill(66210, "UseSpecificSkill", &["0".into()]).is_none());
        assert!(
            specific_skill(
                66210,
                "UseSpecificSkill",
                &["4".into(), "0".into(), "999".into()]
            )
            .is_none()
        );
        assert!(specific_skill(66210, "UseSpecificSkill", &["6".into(), "1".into()]).is_none());
    }

    #[test]
    fn can_use_skill_keeps_its_exact_semantic_kind() {
        assert_eq!(
            can_use_skill(615201, "CanUseSkill", &["308801711".into()]),
            Some(ParsedConditionKind::CanUseSkill(vec![308801711]))
        );
    }

    #[test]
    fn current_skill_level_preserves_all_configured_ranks() {
        assert_eq!(
            rank(620402, "CurrSkillLevel", &["2,3".into()]),
            Some(ParsedConditionKind::ActiveSkillRank {
                compare: ConditionCompare::Equal,
                ranks: vec![2, 3],
            })
        );
    }

    #[test]
    fn aleph_active_attack_conditions_use_the_ally_action_lane() {
        assert_eq!(
            active_use(502212, "ActiveUseSkill", &["0".into()]),
            Some(ParsedConditionKind::ActiveUseSkill { slot: 0 })
        );
        assert_eq!(
            hurt_skill(501212, "UseHurtSkill", &[]),
            Some(ParsedConditionKind::UseHurtSkill)
        );
    }
}
