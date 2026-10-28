use super::*;

#[test]
fn card_level_change_is_a_valid_noop_without_an_eligible_hand_card() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                skill_group1: vec![100, 101, 102],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);

    for args in [vec![3, 1, 1], vec![1, 2, 1]] {
        let behavior = ParsedBehavior::new(50011, "CardLevelChange", args);
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
        let mut target = TargetContext::default();

        assert!(matches!(
            crate::engine::skill::behavior::rule_ops(
                crate::engine::skill::behavior::BehaviorOpContext {
                    source_uid: 10,
                    source_team: 1,
                    target_uid: 10,
                    active_skill_id: 0,
                    transfer_count: 1,
                    event: None,
                    managers: &managers,
                    pool: &pool,
                    determinism: &mut determinism,
                    modifiers: &mut modifiers,
                    target: &mut target,
                },
                &behavior,
            ),
            Some(ops) if ops.is_empty()
        ));
    }
}
