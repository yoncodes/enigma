use super::ParsedConditionKind;

pub(super) fn created(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    args.is_empty()
        .then_some(ParsedConditionKind::SovereignCardCreated)
}
