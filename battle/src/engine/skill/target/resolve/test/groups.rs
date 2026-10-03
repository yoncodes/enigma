use super::*;

#[test]
fn resolves_relative_ally_and_enemy_groups_from_fight() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity_at(10, 1), entity_at(11, 2)],
            sub_entitys: vec![entity_at(12, 3)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity_at(-10, 1), entity_at(-11, 2)],
            sub_entitys: vec![entity_at(-12, 3)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();

    assert_eq!(
        TargetResolver::resolve(
            &TargetRequest {
                code: 101,
                raw: Vec::new(),
            },
            1001,
            10,
            &pool,
            &mut determinism,
        ),
        vec![10, 11]
    );
    assert_eq!(
        TargetResolver::resolve(
            &TargetRequest {
                code: 102,
                raw: Vec::new(),
            },
            1001,
            10,
            &pool,
            &mut determinism,
        ),
        vec![11]
    );
    assert_eq!(
        TargetResolver::resolve_with_context(
            &TargetRequest {
                code: 102,
                raw: Vec::new(),
            },
            1001,
            10,
            &pool,
            &mut determinism,
            TargetContext {
                runtime_target_uid: 12,
                ..Default::default()
            },
        ),
        vec![11]
    );
    assert_eq!(
        TargetResolver::resolve(
            &TargetRequest {
                code: 202,
                raw: Vec::new(),
            },
            1001,
            10,
            &pool,
            &mut determinism,
        ),
        vec![-10, -11]
    );
    assert_eq!(
        TargetResolver::resolve(
            &TargetRequest {
                code: 301,
                raw: Vec::new(),
            },
            1001,
            -10,
            &pool,
            &mut determinism,
        ),
        vec![10, 11, 12]
    );
}

#[test]
fn group_targets_keep_roster_order_for_attacks_and_reactions() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity_at(10, 1), entity_at(11, 2), entity_at(12, 3)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let allies = |skill_id| {
        TargetResolver::resolve_with_managers_and_context(
            &TargetRequest {
                code: 101,
                raw: Vec::new(),
            },
            skill_id,
            10,
            &pool,
            &mut RoundDeterminism::default(),
            Some(&managers),
            TargetContext {
                runtime_target_uid: 11,
                active_skill_is_attack: true,
                ..Default::default()
            },
        )
    };

    // 434111 grants a buff after its carrier is attacked; 31090114 is an attack.
    assert_eq!(allies(434111), vec![10, 11, 12]);
    assert_eq!(allies(31090114), vec![10, 11, 12]);
}

#[test]
fn multi_target_actions_keep_roster_order_independent_of_the_selected_target() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity_at(10, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity_at(-10, 1), entity_at(-11, 2), entity_at(-12, 3)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);

    assert_eq!(
        TargetResolver::resolve_with_context(
            &TargetRequest {
                code: 202,
                raw: Vec::new(),
            },
            1001,
            10,
            &pool,
            &mut RoundDeterminism::default(),
            TargetContext {
                runtime_target_uid: -11,
                active_skill_is_attack: true,
                ..Default::default()
            },
        ),
        vec![-10, -11, -12]
    );

    assert_eq!(
        TargetResolver::resolve_with_context(
            &TargetRequest {
                code: 202,
                raw: Vec::new(),
            },
            1001,
            10,
            &pool,
            &mut RoundDeterminism::default(),
            TargetContext {
                runtime_target_uid: -11,
                ..Default::default()
            },
        ),
        vec![-10, -11, -12]
    );
}

#[test]
fn other_allies_preserves_the_committed_dead_event_target() {
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![entity_at(-1, 1), entity_at(-2, 2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let base_pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.hp.lose(-2, 100, 0);
    let pool = base_pool.runtime_view(&managers);

    assert_eq!(
        TargetResolver::resolve_with_managers_and_context(
            &TargetRequest {
                code: 102,
                raw: Vec::new(),
            },
            1001,
            -1,
            &pool,
            &mut RoundDeterminism::default(),
            Some(&managers),
            TargetContext {
                runtime_target_uid: -2,
                ..Default::default()
            },
        ),
        vec![-2]
    );
}

#[test]
fn event_subject_preserves_a_committed_target_missing_from_the_live_pool() {
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![entity_at(-1, 1), entity_at(-2, 2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let base_pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.hp.lose(-2, 100, 0);
    let pool = base_pool.runtime_view(&managers);

    assert_eq!(
        TargetResolver::resolve_with_context(
            &TargetRequest {
                code: 8,
                raw: Vec::new(),
            },
            1001,
            -1,
            &pool,
            &mut RoundDeterminism::default(),
            TargetContext {
                runtime_target_uid: -2,
                ..Default::default()
            },
        ),
        vec![-2]
    );
}
