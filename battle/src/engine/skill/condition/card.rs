use super::{ParsedConditionKind, parse::parse_i32_args};

pub fn current_enchant(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::CurrentCardEnchant {
        enchant_id: args.first()?.parse().ok()?,
    })
}

pub fn hand_skill_presence(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::HandSkillPresence(parse_i32_args(
        args,
    )?))
}

pub fn round_used_minimum_rank(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let values = parse_i32_args(args)?;
    let [minimum_rank, threshold] = values.as_slice() else {
        return None;
    };
    ((2..=3).contains(minimum_rank) && *threshold > 0).then_some(
        ParsedConditionKind::RoundUsedMinimumRank {
            minimum_rank: *minimum_rank,
            threshold: *threshold,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exact_enchant_or_any_rewrite_marker() {
        assert_eq!(
            current_enchant(760212, "CurUseCardEnchant", &["0".into()]),
            Some(ParsedConditionKind::CurrentCardEnchant { enchant_id: 0 })
        );
        assert_eq!(
            current_enchant(760402, "CurUseCardEnchant", &["10011".into()]),
            Some(ParsedConditionKind::CurrentCardEnchant { enchant_id: 10011 })
        );
    }

    #[test]
    fn parses_round_incantation_rank_and_count() {
        assert_eq!(
            round_used_minimum_rank(622304, "RoundUseSkillLevel", &["2".into(), "2".into()]),
            Some(ParsedConditionKind::RoundUsedMinimumRank {
                minimum_rank: 2,
                threshold: 2,
            })
        );
        assert!(
            round_used_minimum_rank(622304, "RoundUseSkillLevel", &["1".into(), "2".into()])
                .is_none()
        );
        assert!(
            round_used_minimum_rank(622304, "RoundUseSkillLevel", &["2".into(), "0".into()])
                .is_none()
        );
    }
}
