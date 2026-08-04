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
}
