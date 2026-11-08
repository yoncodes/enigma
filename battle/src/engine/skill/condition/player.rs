use super::parse::{ConditionCompare, ParsedConditionKind, parse_fixed};

pub fn level(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [mode, threshold] = parse_fixed(args)?;
    let compare = match mode {
        1 => ConditionCompare::GreaterThan,
        2 => ConditionCompare::LessThan,
        _ => return None,
    };
    (threshold > 0).then_some(ParsedConditionKind::PlayerLevel { compare, threshold })
}
