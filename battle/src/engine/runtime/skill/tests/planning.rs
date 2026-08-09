use super::*;

fn field_event(kind: crate::engine::manager::field::FieldChangeKind) -> BattleEvent {
    BattleEvent::FieldChanged(crate::engine::event::payload::FieldChangeEvent {
        origin: FIELD_ORIGIN,
        team: 1,
        kind,
        field_id: 30002,
        before_level: 1,
        after_level: 2,
        before_progress: 80,
        after_progress: 100,
        overflow: 0,
    })
}

#[test]
fn field_level_activates_added_field_conditions_but_progress_does_not() {
    let mut level_context = TargetContext::default();
    apply_event_context(
        crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
        &mut level_context,
        &field_event(crate::engine::manager::field::FieldChangeKind::Level),
    );
    assert_eq!(level_context.magic_circle_id, 30002);
    assert_eq!(level_context.added_magic_circle_id, 30002);

    let mut progress_context = TargetContext::default();
    apply_event_context(
        crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
        &mut progress_context,
        &field_event(crate::engine::manager::field::FieldChangeKind::Progress),
    );
    assert_eq!(progress_context.magic_circle_id, 30002);
    assert_eq!(progress_context.added_magic_circle_id, 0);
}

#[test]
fn active_skill_rate_modifier_reads_the_authoritative_field_state() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: [-1, -2]
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .to_vec(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .field
        .execute_command(crate::engine::manager::field::FieldCommand {
            origin: FIELD_ORIGIN,
            team: 1,
            operation: crate::engine::manager::field::FieldOperation::DeployIfAbsent {
                definition: crate::engine::manager::field::FieldDefinition {
                    field_id: 30001,
                    duration: 3,
                },
                create_uid: 10,
                initial_level: 1,
                thresholds: Vec::new(),
            },
        })
        .unwrap();
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 30865117,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    invocation.target = SkillTarget::Explicit(-1);
    let mut execution = SkillExecution::new(TargetContext::default());
    execution.configured_targets = Some(vec![-1, -2]);
    execution.context.runtime_target_uid = -1;
    execution.context.logic_target = 201;

    emit_ops(
        invocation,
        &managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        &mut execution,
        &SkillOpTrigger::Active,
    )
    .unwrap();

    assert!(execution.modifiers.rates.iter().any(|modifier| {
        modifier.opcode == 10001
            && modifier.target_uid == 0
            && modifier.fixed_value() == Some(1_000)
    }));
    assert!(execution.modifiers.rates.iter().any(|modifier| {
        modifier.opcode == 10001
            && modifier.target_uid == -2
            && modifier.fixed_value() == Some(-1_100)
    }));
}

#[test]
fn buff_feature_event_preserves_the_exact_act_identity() {
    let mut context = TargetContext::default();
    apply_event_context(
        crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
        &mut context,
        &BattleEvent::BuffFeatureTriggered(
            crate::engine::event::payload::BuffFeatureTriggeredEvent {
                owner_uid: 10,
                source_uid: 11,
                target_uid: 14,
                buff_uid: 12,
                buff_id: 13,
                act_id: 827,
            },
        ),
    );

    assert_eq!(context.runtime_target_uid, 14);
    assert_eq!(context.triggered_buff_uid, 12);
    assert_eq!(context.triggered_buff_act_id, 827);
}

#[test]
fn reinforced_cast_uses_the_configured_upgrade_effect_without_changing_skill_identity() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(700),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [30860143], []);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 30860143,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    invocation.extra_skill_kind =
        Some(crate::engine::skill::condition::extra::ExtraSkillKind::Reinforced);

    let ops = emit_all_ops(
        invocation,
        &managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &SkillOpTrigger::Active,
    )
    .unwrap();

    let damage = ops.iter().find_map(|op| match op {
        RuleOp::Command(BattleCommand::Hp(HpCommand::Damage(damage))) => Some(damage),
        RuleOp::Command(BattleCommand::HpBatch(commands)) => commands.iter().find_map(|command| {
            let HpCommand::Damage(damage) = command else {
                return None;
            };
            Some(damage)
        }),
        _ => None,
    });
    assert_eq!(
        damage.map(|damage| (damage.origin.key, damage.amount)),
        Some((DefinitionKey::new(30860143, "SkillDamage"), 3_000))
    );
}
