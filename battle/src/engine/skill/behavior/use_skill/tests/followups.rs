use super::*;

#[test]
fn consume_buff_use_skill_settles_exact_buff_after_extra_action() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(30070211),
                    layer: Some(4),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = crate::engine::manager::BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60121, "ConsumeBuffUseSkill"),
        vec![30070211, 2, 30073335, 1],
        Vec::new(),
    );

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -1,
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
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Skill(invocation),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                target_uid: 10,
                selector: BuffSelector::ExactId(30070211),
                amount: 2,
                ..
            })))
        ] if invocation.plan.skill_id == 30073335
            && invocation.target == crate::engine::skill::action::SkillTarget::Explicit(-1)
            && invocation.extra_skill_kind == Some(ExtraSkillKind::ExtraAction)
            && invocation.mode == crate::engine::skill::action::SkillExecutionMode::Active
    ));

    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60311, "ConsumeBuffUseSkill3"),
        vec![30070211, 2, 31430181],
        Vec::new(),
    );
    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -1,
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
    )
    .unwrap();
    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                target_uid: 10,
                selector: BuffSelector::ExactId(30070211),
                amount: 2,
                ..
            }))),
            RuleOp::Skill(invocation)
        ] if invocation.plan.skill_id == 31430181
            && invocation.target == crate::engine::skill::action::SkillTarget::Explicit(-1)
            && invocation.extra_skill_kind.is_none()
    ));
}

#[test]
fn consume_buff_use_skill_two_resolves_group_after_consuming_marker() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3047),
                current_hp: Some(100),
                skill_group2: vec![30470151, 30470152, 30470153],
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(30470171),
                    layer: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = crate::engine::manager::BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60128, "ConsumeBuffUseSkill2"),
        vec![30470171, 1, 2, 1],
        Vec::new(),
    );

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 30470171,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                target_uid: 10,
                selector: BuffSelector::ExactId(30470171),
                amount: 1,
                ..
            }))),
            RuleOp::Skill(invocation)
        ] if invocation.plan.skill_id == 30470151
            && invocation.target == crate::engine::skill::action::SkillTarget::Explicit(10)
            && invocation.mode == crate::engine::skill::action::SkillExecutionMode::Nested
    ));
    assert!(supports_consume_buff_use_skill2(&behavior));
}

fn ezio_reuse_ops(
    source_uid: i64,
    card_index: i32,
    is_attack: bool,
    target_uid: i64,
    mode: crate::engine::skill::action::SkillExecutionMode,
    second_marked_enemy: bool,
) -> (Option<Vec<RuleOp>>, ParsedBehavior) {
    crate::test_support::init_config();
    let mut enemies = vec![FightEntityInfo {
        uid: Some(-1),
        position: Some(1),
        team_type: Some(2),
        current_hp: Some(100),
        buffs: vec![BuffInfo {
            uid: Some(20),
            buff_id: Some(229701),
            layer: Some(2),
            ..Default::default()
        }],
        ..Default::default()
    }];
    if second_marked_enemy {
        enemies.push(FightEntityInfo {
            uid: Some(-2),
            position: Some(2),
            team_type: Some(2),
            current_hp: Some(100),
            buffs: vec![BuffInfo {
                uid: Some(21),
                buff_id: Some(229701),
                layer: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        });
    }
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: enemies,
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = crate::engine::manager::BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(100007, "EzioReuse"),
        vec![229701, 1, 312301711],
        Vec::new(),
    );

    let ally_action = crate::engine::event::payload::BattleEvent::AllyAction(
        crate::engine::skill::action::ActionEvent {
            source_uid,
            card_index,
            is_attack,
            mode,
            ..Default::default()
        },
    );
    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid,
            active_skill_id: 0,
            transfer_count: 1,
            event: Some(&ally_action),
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    );
    (ops, behavior)
}

#[test]
fn target_buff_follow_up_consumes_the_mark_after_invocation() {
    let (ops, behavior) = ezio_reuse_ops(
        11,
        1,
        true,
        -1,
        crate::engine::skill::action::SkillExecutionMode::Active,
        false,
    );
    let ops = ops.unwrap();

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Skill(invocation),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                target_uid: -1,
                selector: BuffSelector::ExactId(229701),
                amount: 1,
                depleted: DepletedBuff::RemoveWithZeroLayer,
                ..
            })))
        ] if invocation.plan.source_uid == 10
            && invocation.plan.skill_id == 312301711
            && invocation.target == crate::engine::skill::action::SkillTarget::Explicit(-1)
            && invocation.extra_skill_kind == Some(ExtraSkillKind::FollowUp)
            && invocation.mode == crate::engine::skill::action::SkillExecutionMode::Active
    ));
    assert_eq!(Handler::output_owner(&behavior, &ops[0], 0), None);
    assert_eq!(
        Handler::output_owner(&behavior, &ops[1], 1),
        Some(super::super::super::registry::OutputOwner::Parent)
    );
    let references = references(&behavior);
    assert_eq!(references.buffs, vec![229701]);
    assert_eq!(references.skills, vec![312301711]);
}

#[test]
fn target_buff_follow_up_uses_the_marked_enemy_for_an_ally_attack() {
    let (ops, _) = ezio_reuse_ops(
        11,
        1,
        true,
        11,
        crate::engine::skill::action::SkillExecutionMode::Active,
        false,
    );
    let ops = ops.unwrap();

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Skill(invocation),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                target_uid: -1,
                ..
            })))
        ] if invocation.target == crate::engine::skill::action::SkillTarget::Explicit(-1)
    ));
}

#[test]
fn target_buff_follow_up_ignores_ineligible_ally_actions() {
    use crate::engine::skill::action::SkillExecutionMode;

    assert!(
        ezio_reuse_ops(11, 0, true, -1, SkillExecutionMode::Nested, false)
            .0
            .unwrap()
            .is_empty()
    );
    assert!(
        ezio_reuse_ops(11, 1, false, 11, SkillExecutionMode::Active, false)
            .0
            .unwrap()
            .is_empty()
    );
    assert!(
        ezio_reuse_ops(10, 1, true, -1, SkillExecutionMode::Active, false)
            .0
            .unwrap()
            .is_empty()
    );
    assert!(
        ezio_reuse_ops(11, 1, true, -1, SkillExecutionMode::DeviceCard, false)
            .0
            .unwrap()
            .is_empty()
    );
}

#[test]
fn target_buff_follow_up_rejects_an_ambiguous_marked_enemy() {
    assert!(
        ezio_reuse_ops(
            11,
            1,
            true,
            11,
            crate::engine::skill::action::SkillExecutionMode::Active,
            true,
        )
        .0
        .is_none()
    );
}

#[test]
fn target_buff_follow_up_rejects_unproven_multi_stack_variants() {
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(100007, "EzioReuse"),
        vec![90071, 10, 30030111],
        Vec::new(),
    );

    assert!(!supports_consume_target_buff_use_skill(&behavior));
}

#[test]
fn remove_buff_use_skill_replays_captured_weighted_follow_up() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(30830111),
                    layer: Some(3),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = crate::engine::manager::BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_random_skills([30830172]);
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(50018, "RemoveBuffUseSkill"),
        vec![30830111, 1, 3],
        vec![
            "30830111".to_owned(),
            "1".to_owned(),
            "3".to_owned(),
            "30830171,30830172,30830173,30830174,30830175".to_owned(),
            "20,40,40".to_owned(),
        ],
    );

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 308301412,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                target_uid: 10,
                selector: BuffSelector::ExactId(30830111),
                amount: 2,
                ..
            }))),
            RuleOp::Skill(invocation),
        ] if invocation.plan.skill_id == 30830172
            && invocation.target
                == crate::engine::skill::action::SkillTarget::Configured
            && invocation.extra_skill_kind == Some(ExtraSkillKind::FollowUp)
            && invocation.mode
                == crate::engine::skill::action::SkillExecutionMode::Active
    ));
}

fn range_modifier_ops(
    liquor_layer: i32,
    cheers_count: i32,
    scripted_skill: i32,
) -> (Vec<RuleOp>, ParsedBehavior) {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![
                    BuffInfo {
                        uid: Some(20),
                        buff_id: Some(30830111),
                        layer: Some(liquor_layer),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(21),
                        buff_id: Some(30830121),
                        count: Some(cheers_count),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = crate::engine::manager::BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_random_skills([scripted_skill]);
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(50018, "RemoveBuffUseSkill"),
        vec![30830111, 1, 3],
        vec![
            "30830111".to_owned(),
            "1".to_owned(),
            "3".to_owned(),
            "30830171,30830172,30830173,30830174,30830175".to_owned(),
            "20,40,40".to_owned(),
        ],
    );

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 308301412,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    (ops, behavior)
}

#[test]
fn range_modifier_shifts_weighted_choice_then_consumes_one_stack() {
    let (ops, behavior) = range_modifier_ops(5, 2, 30830174);

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                target_uid: 10,
                selector: BuffSelector::ExactId(30830111),
                amount: 4,
                ..
            }))),
            RuleOp::Skill(invocation),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                target_uid: 10,
                selector: BuffSelector::Uid(21),
                amount: 1,
                ..
            }))),
        ] if invocation.plan.skill_id == 30830174
    ));
    assert_eq!(
        references(&behavior).skills,
        vec![30830171, 30830172, 30830173, 30830174, 30830175]
    );
}

#[test]
fn range_modifier_consumes_all_liquor_when_below_its_shifted_minimum() {
    let (ops, _) = range_modifier_ops(1, 1, 30830171);

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                selector: BuffSelector::ExactId(30830111),
                amount: 1,
                ..
            }))),
            RuleOp::Skill(invocation),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                selector: BuffSelector::Uid(21),
                amount: 1,
                ..
            }))),
        ] if invocation.plan.skill_id == 30830171
    ));
}

#[test]
fn group_skill_keeps_explicit_follow_up_subtype() {
    assert_eq!(
        nested_skill_kind(&[2, 2, 0, 2]),
        ExtraSkillKind::FollowUp.id()
    );
    assert_eq!(nested_skill_kind(&[2, 2]), ExtraSkillKind::ExtraAction.id());
}

#[test]
fn group_three_resolves_the_configured_ultimate() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(251002),
                ex_skill: Some(114100531),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);

    assert_eq!(skill_from_group_and_star(&pool, 10, 3, 1), Some(114100531));
    assert!(supports_group_and_star_skill(&ParsedBehavior::new(
        50010,
        "DirectUseGroupAndStarSkill",
        vec![3, 1],
    )));
    assert!(!supports_group_and_star_skill(&ParsedBehavior::new(
        50010,
        "DirectUseGroupAndStarSkill",
        vec![3],
    )));
}

#[test]
fn parses_weighted_random_skills() {
    assert_eq!(
        weighted_skills("530000751:100&530000752:25"),
        Some(vec![(530000751, 100), (530000752, 25)])
    );
    assert_eq!(weighted_skills("530000751:100&bad"), None);
    assert_eq!(weighted_skills("-530000751:100"), None);
    assert_eq!(weighted_skills("530000751:0"), None);
}

#[test]
fn descriptor_reports_configured_child_skills() {
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60225, "RandomUseSkill"),
        Vec::new(),
        vec!["530000751:100&530000752:25".to_owned()],
    );

    assert_eq!(references(&behavior).skills, vec![530000751, 530000752]);
}

#[test]
fn destination_random_skill_consumes_the_captured_choice() {
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60225, "RandomUseSkill"),
        Vec::new(),
        vec!["530000751:100&530000752:25".to_owned()],
    );
    let managers = crate::engine::manager::BattleManagers::default();
    let pool = TargetPool::default();
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_random_skills([530000752]);
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 99,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Skill(invocation)]
            if invocation.plan.skill_id == 530000752
                && invocation.target
                    == crate::engine::skill::action::SkillTarget::Explicit(-1)
                && invocation.mode
                    == crate::engine::skill::action::SkillExecutionMode::Active
    ));
}

#[test]
fn drive_uses_the_selected_ally_as_the_random_attack_caster() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-5),
                current_hp: Some(100),
                skill_group1: vec![530000111],
                skill_group2: vec![530000121],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = crate::engine::manager::BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_random_skills([530000111]);
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::new(60172, "Drive", vec![1, 1, 0]);

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: -3,
            source_team: 2,
            target_uid: -5,
            active_skill_id: 530000131,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Skill(invocation)]
            if invocation.plan.source_uid == -5
                && invocation.plan.skill_id == 530000111
                && invocation.target == crate::engine::skill::action::SkillTarget::Configured
                && invocation.mode
                    == crate::engine::skill::action::SkillExecutionMode::Active
    ));
    assert!(supports_drive(&behavior));
    assert!(!supports_drive(&ParsedBehavior::new(
        60172,
        "Drive",
        vec![1, 1, 2],
    )));
}

#[test]
fn drive_without_an_attack_candidate_is_a_no_op() {
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-5),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = crate::engine::manager::BattleManagers::default();
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::new(60172, "Drive", vec![1, 1, 0]);

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: -3,
            source_team: 2,
            target_uid: -5,
            active_skill_id: 530000131,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    );

    assert_eq!(ops, Some(Vec::new()));
}

#[test]
fn direct_big_skill_refunds_only_the_configured_ultimate_cost() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ex_point: Some(5),
                ex_skill: Some(30020131),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = crate::engine::manager::BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::new(60175, "DirectUseBigSkill", Vec::new());

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: -1,
            source_team: 2,
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
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Change(ExPointChange {
                delta: -5,
                ..
            }))),
            RuleOp::Skill(invocation),
            RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Change(ExPointChange {
                delta: 5,
                ..
            })))
        ] if invocation.plan.source_uid == 10
            && invocation.plan.skill_id == 30020131
            && invocation.mode == crate::engine::skill::action::SkillExecutionMode::DirectBig
            && invocation.additional_moxie == 5
    ));
}
