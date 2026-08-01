use crate::engine::skill::condition::parse::ParsedConditionKind;

pub fn parse_buff_feature(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    args.is_empty()
        .then_some(ParsedConditionKind::BuffFeatureTriggered { act_id: 827 })
}

pub fn parse_no_action_round(_: i32, _: &str, _: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::NoActionRound)
}

pub fn parse_target_attacked(_: i32, _: &str, _: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::TargetAttacked)
}

pub fn parse_ally_attacked(_: i32, _: &str, _: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::AllyAttacked)
}

pub fn parse_share_damage(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    args.is_empty().then_some(ParsedConditionKind::ShareDamage)
}

pub fn parse_assassinate(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    args.is_empty().then_some(ParsedConditionKind::Assassinate)
}

pub fn parse_target_guard_broken(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    args.is_empty()
        .then_some(ParsedConditionKind::TargetGuardBroken)
}

pub fn parse_guard_broken(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    args.is_empty().then_some(ParsedConditionKind::GuardBroken)
}

pub fn parse_use_ex_skill(_: i32, _: &str, _: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::UseExSkill)
}

pub fn parse_target_use_ex_skill(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    args.is_empty()
        .then_some(ParsedConditionKind::TargetUseExSkill)
}

pub fn parse_teammate_use_ex_skill(
    _: i32,
    _: &str,
    args: &[String],
) -> Option<ParsedConditionKind> {
    args.is_empty()
        .then_some(ParsedConditionKind::TeammateUseExSkill)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_parsers_only_construct_their_owned_kind() {
        assert_eq!(
            parse_target_attacked(22209, "BeAttacked", &[]),
            Some(ParsedConditionKind::TargetAttacked)
        );
        assert_eq!(
            parse_use_ex_skill(25210, "UseExSkill", &[]),
            Some(ParsedConditionKind::UseExSkill)
        );
        assert_eq!(
            parse_target_use_ex_skill(25212, "UseExSkill", &[]),
            Some(ParsedConditionKind::TargetUseExSkill)
        );
        assert_eq!(
            parse_teammate_use_ex_skill(720212, "TeammateUseExSkill", &[]),
            Some(ParsedConditionKind::TeammateUseExSkill)
        );
        assert_eq!(
            parse_ally_attacked(22213, "BeAttacked", &[]),
            Some(ParsedConditionKind::AllyAttacked)
        );
        assert_eq!(
            parse_target_guard_broken(791210, "ToBrokenEnemy", &[]),
            Some(ParsedConditionKind::TargetGuardBroken)
        );
        assert_eq!(
            parse_guard_broken(2092, "None", &[]),
            Some(ParsedConditionKind::GuardBroken)
        );
    }
}
