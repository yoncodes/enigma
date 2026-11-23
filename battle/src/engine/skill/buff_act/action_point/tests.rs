use super::*;

fn feature(act_id: i32, kind: &str) -> ActiveBuffFeature {
    ActiveBuffFeature {
        owner_uid: 1,
        source_uid: 1,
        buff_uid: 2,
        buff_id: 3,
        amount: 1,
        team_type: 1,
        owner_alive: true,
        act_type: kind.to_owned(),
        effect_time: 0,
        effect_condition: 0,
        raw: String::new(),
        values: vec![act_id],
    }
}

#[test]
fn ultimate_only_rule_waives_only_the_owner_ultimate() {
    let feature = feature(946, "BigSkillNoUseActPoint");
    assert!(!skill_uses_action_point(
        std::slice::from_ref(&feature),
        1,
        true
    ));
    assert!(skill_uses_action_point(
        std::slice::from_ref(&feature),
        1,
        false
    ));
    assert!(skill_uses_action_point(&[feature], 2, true));
}

#[test]
fn owner_wide_rule_waives_basic_and_ultimate_costs_only_for_its_owner() {
    let feature = feature(1140, "SkillNoUseActPoint");
    assert!(!skill_uses_action_point(
        std::slice::from_ref(&feature),
        1,
        false
    ));
    assert!(!skill_uses_action_point(
        std::slice::from_ref(&feature),
        1,
        true
    ));
    assert!(skill_uses_action_point(
        std::slice::from_ref(&feature),
        2,
        false
    ));
    assert!(skill_uses_action_point(&[feature], 2, true));
}
