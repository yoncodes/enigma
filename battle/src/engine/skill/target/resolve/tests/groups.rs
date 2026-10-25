use super::*;

#[test]
fn captured_target_comparison_preserves_order_and_cardinality() {
    assert!(captured_targets_match(&[10, 11, 12], &[10, 11, 12]));
    assert!(!captured_targets_match(&[10, 11, 12], &[12, 10, 11]));
    assert!(!captured_targets_match(&[10, 11, 12], &[12, 10]));
    assert!(!captured_targets_match(&[10, 10, 11], &[10, 11, 12]));
}

#[test]
fn other_allies_anchor_their_shared_skill_frame_to_the_caster() {
    assert_eq!(frame_anchor_for_rule(102, 10), Some(10));
    assert_eq!(frame_anchor_for_rule(103, 10), None);
}

#[test]
fn mass_targets_preserve_the_selected_skill_frame_target() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity_at(10, 1), entity_at(11, 2)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity_at(-1, 1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let attack = TargetContext {
        active_skill_is_attack: true,
        ..Default::default()
    };

    assert!(preserves_selected_frame_target(130, 10, -1, &pool, attack));
    assert!(preserves_selected_frame_target(101, 10, -1, &pool, attack));
    assert!(preserves_selected_frame_target(202, 10, -1, &pool, attack));
    assert!(preserves_selected_frame_target(1, 10, 11, &pool, attack));
    assert!(!preserves_selected_frame_target(1, 10, -1, &pool, attack));
}

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
fn enemy_side_attacks_put_the_selected_primary_before_the_roster_order() {
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
        vec![-11, -10, -12]
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

    assert_eq!(
        TargetResolver::resolve_primary_candidates(
            &TargetRequest {
                code: 202,
                raw: Vec::new(),
            },
            1001,
            10,
            &pool,
            &RoundDeterminism::default(),
            None,
            TargetContext {
                active_skill_is_attack: true,
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
fn side_state_target_resolves_to_the_attacker_side() {
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
        vec![crate::engine::fight::rules::ATTACKER_SIDE_UID]
    );
}
