use super::{ConditionCompare, ParsedConditionKind};

pub fn parse(_opcode: i32, _type_name: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    let tag_id = raw_args.first()?.parse().ok()?;
    let threshold = raw_args.get(1)?.parse().ok()?;
    let compare = match raw_args.get(2).map(String::as_str).unwrap_or("1") {
        "1" => ConditionCompare::GreaterThanOrEqual,
        "3" => ConditionCompare::Equal,
        "5" => ConditionCompare::LessThan,
        _ => return None,
    };
    Some(ParsedConditionKind::BattleTagCount {
        tag_id,
        compare,
        threshold,
    })
}

pub fn present(_opcode: i32, _type_name: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    let [tag] = raw_args else { return None };
    Some(ParsedConditionKind::TargetBattleTag(tag.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_and_comparison_follow_the_config_argument_order() {
        assert_eq!(
            parse(
                762021,
                "BattleTagNum",
                &["114".into(), "3".into(), "1".into()]
            ),
            Some(ParsedConditionKind::BattleTagCount {
                tag_id: 114,
                compare: ConditionCompare::GreaterThanOrEqual,
                threshold: 3,
            })
        );
    }

    #[test]
    fn presence_keeps_the_configured_target_tag() {
        assert_eq!(
            present(1021002, "BattleTagCheck", &["102".into()]),
            Some(ParsedConditionKind::TargetBattleTag(102))
        );
        assert_eq!(
            present(1021002, "BattleTagCheck", &["102".into(), "999".into()]),
            None
        );
    }
}
