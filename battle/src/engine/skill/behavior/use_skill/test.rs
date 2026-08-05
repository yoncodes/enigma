use super::*;
use crate::engine::skill::behavior::classify::BehaviorSpec;
use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

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
fn target_buff_follow_up_consumes_the_mark_before_invocation() {
    crate::test_support::init_config();
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
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(229701),
                    layer: Some(2),
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
        BehaviorSpec::new(100007, "EzioReuse"),
        vec![229701, 1, 312301711],
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
                target_uid: -1,
                selector: BuffSelector::ExactId(229701),
                amount: 1,
                ..
            }))),
            RuleOp::Skill(invocation)
        ] if invocation.plan.source_uid == 10
            && invocation.plan.skill_id == 312301711
            && invocation.target == crate::engine::skill::action::SkillTarget::Explicit(-1)
            && invocation.extra_skill_kind == Some(ExtraSkillKind::FollowUp)
            && invocation.mode == crate::engine::skill::action::SkillExecutionMode::Active
    ));
    let references = references(&behavior);
    assert_eq!(references.buffs, vec![229701]);
    assert_eq!(references.skills, vec![312301711]);
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
fn direct_use_skill_publishes_an_action_but_no_act_does_not() {
    let managers = crate::engine::manager::BattleManagers::default();
    let pool = TargetPool::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let mut emit = |behavior: &ParsedBehavior| {
        Handler::emit_ops(
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
            behavior,
        )
        .unwrap()
    };

    let direct = emit(&ParsedBehavior::new(50008, "DirectUseSkill", vec![20]));
    let no_act = emit(&ParsedBehavior::new(50012, "DirectUseSkillNoAct", vec![20]));
    let card = emit(&ParsedBehavior::new(
        50039,
        "DirectUseSkillCard",
        vec![21, 0],
    ));

    assert!(matches!(
        direct.as_slice(),
        [RuleOp::Skill(invocation)]
            if invocation.mode
                == crate::engine::skill::action::SkillExecutionMode::Active
    ));
    assert!(matches!(
        no_act.as_slice(),
        [RuleOp::Skill(invocation)]
            if invocation.mode
                == crate::engine::skill::action::SkillExecutionMode::Nested
    ));
    assert!(matches!(
        card.as_slice(),
        [RuleOp::Skill(invocation)]
            if invocation.plan.skill_id == 21
                && invocation.target
                    == crate::engine::skill::action::SkillTarget::Configured
                && invocation.mode
                    == crate::engine::skill::action::SkillExecutionMode::Active
    ));
}

#[test]
fn crystal_reuse_scales_the_configured_chance_by_selected_crystals() {
    crate::test_support::init_config();
    let behavior = ParsedBehavior::new(60242, "CrystalReuse", vec![334, 31340152, 1]);
    let mut managers = crate::engine::manager::BattleManagers::default();
    assert!(managers.emanation.select(10, 300));
    let pool = TargetPool::default();
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_permille_rolls([999]);
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 31340151,
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
            if invocation.plan.skill_id == 31340152
                && invocation.target
                    == crate::engine::skill::action::SkillTarget::Configured
                && invocation.extra_skill_kind == Some(ExtraSkillKind::FollowUp)
                && invocation.start
                    == crate::engine::skill::action::SkillStart::AfterCurrentAction
                && invocation.mode
                    == crate::engine::skill::action::SkillExecutionMode::Active
    ));
}

#[test]
fn crystal_reuse_requires_the_configured_crystal_type() {
    let behavior = ParsedBehavior::new(60242, "CrystalReuse", vec![1000, 31340152, 1]);
    let managers = crate::engine::manager::BattleManagers::default();
    let pool = TargetPool::default();
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_random_skills([31340152]);
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 31340151,
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

    assert!(ops.is_empty());
    assert_eq!(determinism.take_random_skill(&[31340152]), Some(31340152));
}

#[test]
fn repeat_previous_skill_uses_the_event_actor_skill_and_target() {
    let behavior = ParsedBehavior::new(60014, "DirectUseSkillPrev", vec![]);
    let managers = crate::engine::manager::BattleManagers::default();
    let pool = TargetPool::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext {
        active_skill_source_uid: 20,
        active_skill_id: 370001002,
        runtime_target_uid: -3,
        ..Default::default()
    };

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -3,
            active_skill_id: 370002100,
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
            if invocation.plan.source_uid == 20
                && invocation.plan.skill_id == 370001002
                && invocation.target
                    == crate::engine::skill::action::SkillTarget::Explicit(-3)
                && invocation.start
                    == crate::engine::skill::action::SkillStart::AfterCurrentAction
                && invocation.mode
                    == crate::engine::skill::action::SkillExecutionMode::Nested
    ));
}

#[test]
fn direct_use_skill_no_act_preserves_the_trigger_target() {
    let behavior = ParsedBehavior::new(50012, "DirectUseSkillNoAct", vec![434725, 1]);
    let managers = crate::engine::manager::BattleManagers::default();
    let pool = TargetPool::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext {
        runtime_target_uid: 10,
        ..Default::default()
    };

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 20,
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
            if invocation.plan.skill_id == 434725
                && invocation.target
                    == crate::engine::skill::action::SkillTarget::Explicit(10)
    ));
}

#[test]
fn direct_use_skill_no_act_accepts_only_observed_arities() {
    assert!(supports_direct_no_action_skill(&ParsedBehavior::new(
        50012,
        "DirectUseSkillNoAct",
        vec![23390241],
    )));
    assert!(supports_direct_no_action_skill(&ParsedBehavior::new(
        50012,
        "DirectUseSkillNoAct",
        vec![23390241, 1],
    )));
    assert!(!supports_direct_no_action_skill(&ParsedBehavior::new(
        50012,
        "DirectUseSkillNoAct",
        vec![23390241, 1, 0],
    )));
}

#[test]
fn consume_power_skill_repeats_cost_and_extra_action_for_each_affordable_cast() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                power_infos: vec![sonettobuf::PowerInfo {
                    power_id: Some(EUREKA_RESOURCE_ID),
                    num: Some(4),
                    max: Some(5),
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
    let behavior = ParsedBehavior::new(60188, "ConsumePowerUseSkill", vec![2, 31170145]);

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
        [
            RuleOp::Command(BattleCommand::Eureka(EurekaCommand::Change(
                EurekaChange { delta: -2, .. }
            ))),
            RuleOp::Skill(first),
            RuleOp::Command(BattleCommand::Eureka(EurekaCommand::Change(
                EurekaChange { delta: -2, .. }
            ))),
            RuleOp::Skill(second),
        ] if [first, second].into_iter().all(|invocation|
            invocation.plan.skill_id == 31170145
            && invocation.extra_skill_kind == Some(ExtraSkillKind::ExtraAction)
            && invocation.mode
                == crate::engine::skill::action::SkillExecutionMode::Active
        )
    ));
}

#[test]
fn consume_power_direct_skill_spends_once_without_extra_action_or_team_energy() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                power_infos: vec![sonettobuf::PowerInfo {
                    power_id: Some(EUREKA_RESOURCE_ID),
                    num: Some(3),
                    max: Some(5),
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
    let behavior = ParsedBehavior::new(50036, "ConsumePowerDirectUseSkill", vec![3, 530000731]);

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 530000742,
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
            RuleOp::Command(BattleCommand::Eureka(EurekaCommand::Change(EurekaChange {
                delta: -3,
                ..
            }))),
            RuleOp::Skill(invocation),
        ] if invocation.plan.skill_id == 530000731
            && invocation.target
                == crate::engine::skill::action::SkillTarget::Explicit(-1)
            && invocation.extra_skill_kind.is_none()
            && invocation.mode
                == crate::engine::skill::action::SkillExecutionMode::Nested
    ));
}

#[test]
fn direct_use_skill_owns_its_random_ally_target_code() {
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(1),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&Fight {
        defender: Some(FightTeam {
            entitys: vec![entity(-1), entity(-2), entity(-3)],
            ..Default::default()
        }),
        ..Default::default()
    });
    let behavior = ParsedBehavior::new(50008, "DirectUseSkill", vec![10]);

    assert_eq!(
        resolve_targets(
            20,
            -1,
            201,
            &pool,
            &mut RoundDeterminism::default(),
            &behavior,
        ),
        Some(vec![-3])
    );

    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_skill_target_choices([
        crate::engine::runtime::determinism::SkillTargetChoice {
            skill_id: 20,
            source_uid: -1,
            target_code: 201,
            targets: vec![-2],
            additional_targets: Vec::new(),
            crit_targets: Vec::new(),
            additional_crit_targets: Vec::new(),
        },
    ]);
    assert_eq!(
        resolve_targets(20, -1, 201, &pool, &mut determinism, &behavior),
        Some(vec![-2])
    );
}

#[test]
fn direct_use_skill_two_preserves_the_configured_reinforced_subtype() {
    let managers = crate::engine::manager::BattleManagers::default();
    let pool = TargetPool::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::new(60053, "DirectUseSkill2", vec![30860143, 0, 0, 5]);

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 30860176,
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
            if invocation.plan.skill_id == 30860143
                && invocation.target
                    == crate::engine::skill::action::SkillTarget::Explicit(-1)
                && invocation.extra_skill_kind == Some(ExtraSkillKind::Reinforced)
    ));
}
