use super::*;

#[test]
fn selected_attack_without_client_input_uses_the_first_enemy() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity_at(10, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity_at(-1, 1), entity_at(-2, 2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();

    assert_eq!(
        resolve_code_with_context(
            1,
            10,
            &pool,
            &mut determinism,
            TargetContext {
                active_skill_is_attack: true,
                ..Default::default()
            },
        ),
        vec![-1]
    );
}

#[test]
fn ally_single_target_opcodes_keep_their_exact_selection_rules() {
    init_config();
    let mut first = entity_stats(10, 1, 40, 100, 0);
    first.attr.as_mut().unwrap().attack = Some(100);
    let mut second = entity_stats(11, 2, 30, 50, 0);
    second.attr.as_mut().unwrap().attack = Some(300);
    let mut third = entity_stats(12, 3, 80, 100, 0);
    third.attr.as_mut().unwrap().attack = Some(200);
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![first, second, third],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity_at(-1, 1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_skill_target_choices([SkillTargetChoice {
        skill_id: 1001,
        source_uid: 10,
        target_code: 106,
        targets: vec![12],
        additional_targets: Vec::new(),
        crit_targets: Vec::new(),
        additional_crit_targets: Vec::new(),
    }]);

    assert_eq!(resolve_code(106, 10, &pool, &mut determinism), vec![12]);
    assert_eq!(
        resolve_code(107, 10, &pool, &mut RoundDeterminism::default()),
        vec![10]
    );
    assert_eq!(
        resolve_code(108, 10, &pool, &mut RoundDeterminism::default()),
        vec![11]
    );
    assert_eq!(
        resolve_code(109, 10, &pool, &mut RoundDeterminism::default()),
        vec![11]
    );
}

#[test]
fn resolves_lowest_highest_and_position_targets() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity_stats(10, 1, 80, 100, 1),
                entity_stats(11, 2, 30, 100, 5),
                entity_stats(12, 3, 50, 100, 2),
                entity_stats(13, 4, 60, 100, 3),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                entity_stats(-10, 1, 70, 100, 0),
                entity_stats(-11, 2, 90, 100, 0),
                entity_stats(-12, 3, 20, 100, 0),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();

    assert_eq!(resolve_code(109, 10, &pool, &mut determinism), vec![11]);
    assert_eq!(resolve_code(112, 10, &pool, &mut determinism), vec![11]);
    assert_eq!(resolve_code(113, 10, &pool, &mut determinism), vec![10]);
    assert_eq!(resolve_code(208, 10, &pool, &mut determinism), vec![-11]);
    assert_eq!(resolve_code(231, 10, &pool, &mut determinism), vec![-11]);
    assert_eq!(resolve_code(228, 10, &pool, &mut determinism), vec![-12]);
    assert_eq!(resolve_code(117, 11, &pool, &mut determinism), vec![10, 12]);
    assert_eq!(resolve_code(118, 11, &pool, &mut determinism), vec![12]);
    assert_eq!(resolve_code(128, 11, &pool, &mut determinism), vec![10]);
    assert_eq!(resolve_code(120, 11, &pool, &mut determinism), vec![11, 10]);
    assert_eq!(resolve_code(123, 11, &pool, &mut determinism), vec![12, 13]);
    assert_eq!(resolve_code(127, 11, &pool, &mut determinism), vec![13]);
}

#[test]
fn highest_ex_point_uses_current_main_ally_moxie() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity_stats(10, 1, 100, 100, 0),
                entity_stats(11, 2, 100, 100, 0),
            ],
            sub_entitys: vec![entity_stats(12, -1, 100, 100, 10)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.ex_point.set(11, 11, 3, 0);

    let targets = TargetResolver::resolve_with_managers_and_context(
        &TargetRequest {
            code: 112,
            raw: Vec::new(),
        },
        1001,
        10,
        &pool,
        &mut RoundDeterminism::default(),
        Some(&managers),
        TargetContext::default(),
    );

    assert_eq!(targets, vec![11]);
}

#[test]
fn bound_ally_target_requires_the_sources_completed_contract() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity_at(10, 1), entity_at(11, 2), entity_at(12, 3)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let request = TargetRequest {
        code: 309,
        raw: Vec::new(),
    };
    let resolve = |source_uid, managers: &BattleManagers| {
        TargetResolver::resolve_with_managers_and_context(
            &request,
            433611,
            source_uid,
            &pool,
            &mut RoundDeterminism::default(),
            Some(managers),
            TargetContext::default(),
        )
    };

    assert!(resolve(10, &managers).is_empty());
    let origin = crate::engine::skill::rule::CommandOrigin {
        domain: crate::engine::skill::rule::RuleDomain::Behavior,
        key: crate::engine::skill::rule::DefinitionKey::new(60092, "NotifyHeroContract"),
    };
    managers
        .contract
        .execute(crate::engine::manager::contract::ContractCommand::Offer {
            origin,
            owner_uid: 10,
            candidates: vec![11, 12],
        })
        .unwrap();
    managers
        .contract
        .execute(
            crate::engine::manager::contract::ContractCommand::SelectOwner {
                owner_uid: 10,
                bound_uid: 11,
            },
        )
        .unwrap();
    managers
        .contract
        .execute(
            crate::engine::manager::contract::ContractCommand::SelectBound {
                owner_uid: 10,
                bound_uid: 11,
            },
        )
        .unwrap();

    assert_eq!(resolve(10, &managers), vec![11]);
    assert!(resolve(12, &managers).is_empty());
}

#[test]
fn random_target_codes_validate_captured_choice() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity_at(10, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity_at(-10, 1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    assert_eq!(resolve_code(206, 10, &pool, &mut determinism), vec![-10]);

    let mut determinism = RoundDeterminism::default();
    determinism.skill_targets.push(SkillTargetChoice {
        skill_id: 1001,
        source_uid: 10,
        target_code: 236,
        targets: vec![-10],
        additional_targets: Vec::new(),
        crit_targets: Vec::new(),
        additional_crit_targets: Vec::new(),
    });
    assert_eq!(resolve_code(236, 10, &pool, &mut determinism), vec![-10]);

    let mut determinism = RoundDeterminism::default();
    determinism.skill_targets.push(SkillTargetChoice {
        skill_id: 1001,
        source_uid: 10,
        target_code: 236,
        targets: vec![-99],
        additional_targets: Vec::new(),
        crit_targets: Vec::new(),
        additional_crit_targets: Vec::new(),
    });
    assert_eq!(resolve_code(236, 10, &pool, &mut determinism), vec![-10]);
}

#[test]
fn resolves_runtime_secondary_and_deterministic_random_targets() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity_at(10, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                entity_stats(-10, 1, 70, 100, 0),
                entity_stats(-11, 2, 90, 100, 0),
                entity_stats(-12, 3, 20, 100, 0),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    let context = TargetContext {
        runtime_target_uid: -11,
        logic_target: 202,
        extra_skill_kind: 0,
        ..Default::default()
    };

    assert_eq!(
        resolve_code_with_context(1, 10, &pool, &mut determinism, context),
        vec![-11]
    );
    assert_eq!(
        resolve_code_with_context(7, 10, &pool, &mut determinism, context),
        vec![-11]
    );
    assert_eq!(
        resolve_code_with_context(216, 10, &pool, &mut determinism, context),
        vec![-10, -12]
    );
    assert_eq!(
        resolve_code_with_context(
            216,
            10,
            &pool,
            &mut determinism,
            TargetContext {
                logic_target: 201,
                ..context
            },
        ),
        vec![-10, -12]
    );

    let target = resolve_code_with_context(
        206,
        -10,
        &pool,
        &mut determinism,
        TargetContext {
            runtime_target_uid: -11,
            ..Default::default()
        },
    );
    assert_eq!(target.len(), 1);
    assert!(
        pool.attacker_all
            .iter()
            .any(|entity| entity.uid == target[0])
    );
    assert_eq!(
        resolve_code_with_context(236, 10, &pool, &mut determinism, context),
        vec![-11]
    );
    assert_eq!(
        resolve_code_with_context(201, 10, &pool, &mut determinism, TargetContext::default()),
        vec![-10, -11]
    );
}
