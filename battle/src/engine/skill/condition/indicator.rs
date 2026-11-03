use super::parse::{ParsedConditionKind, parse_fixed};

pub fn at_least(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [indicator_id, threshold] = parse_fixed(args)?;
    (indicator_id > 0 && threshold > 0).then_some(ParsedConditionKind::IndicatorAtLeast {
        indicator_id,
        threshold,
    })
}
