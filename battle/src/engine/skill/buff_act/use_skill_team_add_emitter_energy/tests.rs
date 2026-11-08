use super::*;
use crate::engine::event::{kind::EventKind, subscription::SubscriptionKey};

#[test]
fn only_active_attack_incantations_of_configured_ranks_trigger() {
    let feature = BuffActSubscriber {
        owner_uid: 1,
        source_uid: 1,
        buff_uid: 1,
        buff_id: 2240008,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::SkillCast,
            crate::engine::skill::rule::DefinitionKey::new(881, "UseSkillTeamAddEmitterEnergy"),
        ),
        act_type: "UseSkillTeamAddEmitterEnergy".to_owned(),
        effect_time: 2101,
        effect_condition: 0,
        args: vec![1, 1, 1, 2],
        raw: "881#1#1#1,2".to_owned(),
    };
    assert_eq!(
        spec(&feature, SkillExecutionMode::Active, true, 1),
        Some((1, 1))
    );
    assert_eq!(spec(&feature, SkillExecutionMode::Nested, true, 1), None);
    assert_eq!(spec(&feature, SkillExecutionMode::Active, false, 1), None);
    assert_eq!(spec(&feature, SkillExecutionMode::Active, true, 3), None);
}
