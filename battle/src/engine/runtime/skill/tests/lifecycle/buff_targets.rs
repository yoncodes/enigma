use super::*;

#[test]
fn unsupported_behavior_is_skipped_without_aborting_the_skill() {
    crate::test_support::init_config();
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: Vec::new(),
    });
    catalog.insert_issue(
        100,
        RuleIssue {
            effect_id: 100,
            slot: 1,
            opcode: Some(999),
            type_name: Some("Missing".to_owned()),
            raw: "999".to_owned(),
            reason: RuleIssueReason::UnsupportedBehavior,
        },
    );
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };

    let ops = emit_all_ops(
        SkillRequest {
            source_uid: 10,
            skill_id: 100,
        }
        .into(),
        &BattleManagers::seeded(&fight),
        &TargetPool::from_fight(&fight),
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &SkillOpTrigger::Active,
    )
    .unwrap();

    assert!(ops.is_empty());
}

#[test]
fn configured_after_damage_buff_targets_every_resolved_action_target() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(10_000),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(10_000),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    current_hp: Some(10_000),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_skill_target_choices([
        crate::engine::runtime::determinism::SkillTargetChoice {
            skill_id: 30810481,
            source_uid: 10,
            target_code: 201,
            targets: vec![-1, -2],
            additional_targets: Vec::new(),
            crit_targets: Vec::new(),
            additional_crit_targets: Vec::new(),
        },
    ]);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 30810481,
    }
    .into();
    invocation.mode = crate::engine::skill::action::SkillExecutionMode::Active;
    invocation.target = crate::engine::skill::action::SkillTarget::Explicit(-1);
    let result = crate::engine::runtime::drain::run(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut determinism,
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    assert_eq!(managers.buff.max_id_or_type_layer(-1, 4150001), 2);
    assert_eq!(managers.buff.max_id_or_type_layer(-2, 4150001), 2);
    assert!(result.outcomes.iter().any(|outcome| matches!(
        outcome,
        crate::engine::runtime::executor::RuleOutcome::SkillLifecycle(
            crate::engine::skill::action::SkillLifecycle::ActionCompleted(action)
        ) if action.target_uids == [-1, -2]
    )));
    assert!(result.outcomes.iter().any(|outcome| matches!(
        outcome,
        crate::engine::runtime::executor::RuleOutcome::SkillLifecycle(
            crate::engine::skill::action::SkillLifecycle::PhaseCompleted(action)
        ) if action.phase == crate::engine::skill::action::SkillPhase::Immediate
            && action.target_uids == [-1, -2]
    )));
}

#[test]
fn aggregate_buff_count_with_target_999_applies_to_the_action_targets() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(10_000),
        team_type: Some(if uid > 0 { 1 } else { 2 }),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10), entity(11), entity(12), entity(13)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                entity(-1),
                FightEntityInfo {
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(530000712),
                        from_uid: Some(-2),
                        count: Some(1),
                        ..Default::default()
                    }],
                    ..entity(-2)
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };

    let seeded = BattleManagers::seeded(&fight);
    let effect = crate::engine::skill::effect::catalog::global()
        .get(530000531)
        .unwrap();
    assert!(
        crate::engine::skill::rule::ownership::behavior_is_owned_by_buff_act(
            &effect.slots[0],
            -2,
            &seeded,
        ),
        "hp={} buff={:?} features={:?} slot={:?}",
        seeded.hp.current(-2),
        seeded.buff.snapshot(-2, 20),
        seeded.buff.active_features(&seeded.hp),
        effect.slots[0]
    );
    let ops = emit_all_ops(
        SkillRequest {
            source_uid: -2,
            skill_id: 530000531,
        }
        .into(),
        &seeded,
        &TargetPool::from_fight(&fight),
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &SkillOpTrigger::Active,
    )
    .unwrap();
    let targets = ops
        .iter()
        .filter_map(|op| match op {
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant)))
                if grant.buff_id == 530000412 =>
            {
                Some(grant.target_uid)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(targets.is_empty());

    let mut managers = BattleManagers::seeded(&fight);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: -2,
        skill_id: 530000531,
    }
    .into();
    invocation.mode = crate::engine::skill::action::SkillExecutionMode::Active;
    let result = crate::engine::runtime::drain::run(
        &mut managers,
        &TargetPool::from_fight(&fight),
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    let present = [10, 11, 12, 13].map(|uid| (uid, managers.buff.has_buff_id(uid, 530000412)));
    assert!(
        present.iter().all(|(_, present)| *present),
        "present={present:?} events={:?}",
        result.events
    );
}
