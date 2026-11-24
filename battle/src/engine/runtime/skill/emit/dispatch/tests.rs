use super::*;
use crate::engine::skill::{
    behavior::classify::BehaviorSpec, condition::parse::parse_conditions, effect::ParsedBehavior,
    rule::route::ConditionRoute,
};

#[test]
fn phase_lookahead_propagates_a_conflicting_route() {
    let conditions = parse_conditions(crate::test_support::game_data(), "208&210");
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(1, "AddBuff"), vec![1, 1], Vec::new()),
        TargetRequest::self_only(),
    );
    slot.compiled_route = ConditionRoute::compile(&conditions);

    assert!(matches!(
        slot_runs_in_phase(&slot, SkillPhase::Immediate, 123),
        Err(SkillOpError::UncompiledRoute { skill_id: 123, .. })
    ));
}
