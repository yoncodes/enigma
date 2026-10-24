use super::*;

#[test]
fn death_outside_the_subscriber_group_supersedes_its_pending_transition() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::default();
    let mut determinism = RoundDeterminism::default();
    let mut result = DrainResult::default();
    let mut state = DrainState::new(TargetContext::default());
    let trigger = SkillOpTrigger::Event(BattleEvent::Kind(
        crate::engine::event::kind::EventKind::RoundEndEntitySettlement,
    ));
    let queued = |op, subscriber: Option<SubscriberContext>| QueuedOp {
        op,
        trigger: trigger.clone(),
        skill_execution: None,
        frame_path: None,
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber,
        caster_frame: None,
    };
    let kill = |source_uid, opcode, name| {
        RuleOp::Command(BattleCommand::Hp(
            crate::engine::manager::hp::HpCommand::Kill(crate::engine::manager::hp::HpKill {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(opcode, name),
                },
                source_uid,
                target_uid: -1,
                config_effect: 1,
            }),
        ))
    };
    let outer_group = Rc::new(RefCell::new(SubscriberDeathGroup::default()));
    let outer = SubscriberContext {
        owner_uid: -1,
        skill_id: None,
        group: Some(outer_group),
        intermediate_death_group: false,
    };
    drain_nested_queue(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        [queued(kill(10, 701, "OuterDamage"), Some(outer.clone()))].into(),
        &mut result,
        &mut state,
    )
    .unwrap();

    drain_nested_queue(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        [
            queued(
                RuleOp::Command(BattleCommand::Hp(
                    crate::engine::manager::hp::HpCommand::Heal(
                        crate::engine::manager::hp::HpHeal {
                            origin: CommandOrigin {
                                domain: RuleDomain::BuffAct,
                                key: DefinitionKey::new(702, "NestedRevive"),
                            },
                            source_uid: 11,
                            target_uid: -1,
                            amount: 1,
                            config_effect: 1,
                            kind: crate::engine::manager::hp::HpHealKind::Revive,
                        },
                    ),
                )),
                None,
            ),
            queued(kill(11, 703, "NestedDamage"), None),
        ]
        .into(),
        &mut result,
        &mut state,
    )
    .unwrap();
    drain_nested_queue(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        [queued(RuleOp::SubscriberGroupComplete, Some(outer))].into(),
        &mut result,
        &mut state,
    )
    .unwrap();

    assert_eq!(
        result
            .outcomes
            .iter()
            .filter_map(|outcome| match outcome {
                RuleOutcome::DeathSettlement(death) => Some(*death),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [crate::engine::manager::hp::DeathTransition {
            source_uid: 11,
            target_uid: -1,
        }]
    );
}

#[test]
fn subscriber_death_batch_cleans_each_defeated_halo_emitter() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: [10, 11, 30, 12]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    for target_uid in [10, 11, 30, 12] {
        managers
            .execute_buff(BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 30,
                target_uid,
                buff_id: 31450141,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }))
            .unwrap();
    }

    let trigger = SkillOpTrigger::Event(BattleEvent::Kind(
        crate::engine::event::kind::EventKind::RoundEndEntitySettlement,
    ));
    let group = Rc::new(RefCell::new(SubscriberDeathGroup::default()));
    let subscriber = SubscriberContext {
        owner_uid: 10,
        skill_id: None,
        group: Some(group),
        intermediate_death_group: false,
    };
    let queued = |op| QueuedOp {
        op,
        trigger: trigger.clone(),
        skill_execution: None,
        frame_path: None,
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber: Some(subscriber.clone()),
        caster_frame: None,
    };
    let kill = |target_uid| {
        RuleOp::Command(BattleCommand::Hp(
            crate::engine::manager::hp::HpCommand::Kill(crate::engine::manager::hp::HpKill {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "TestBatchDeath"),
                },
                source_uid: -1,
                target_uid,
                config_effect: 1,
            }),
        ))
    };
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let mut determinism = RoundDeterminism::default();
    let mut result = DrainResult::default();
    let mut state = DrainState::new(TargetContext::default());
    drain_nested_queue(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        [queued(kill(11)), queued(kill(30))].into(),
        &mut result,
        &mut state,
    )
    .unwrap();
    drain_nested_queue(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        [queued(RuleOp::SubscriberGroupComplete)].into(),
        &mut result,
        &mut state,
    )
    .unwrap();

    for owner_uid in [10, 12] {
        assert_eq!(
            managers
                .buff
                .active_for(owner_uid)
                .filter(|buff| buff.buff_id == Some(31450143))
                .count(),
            2
        );
        assert_eq!(managers.buff.buff_id_amount(owner_uid, 31450143), 1);
    }
}

#[test]
fn active_halo_cleanup_precedes_emitter_death() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        uid: Some(2),
                        buff_id: Some(31050146),
                        from_uid: Some(30),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(30),
                    team_type: Some(1),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        uid: Some(1),
                        buff_id: Some(31050143),
                        from_uid: Some(30),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let result = run(
        &mut managers,
        &pool,
        &SkillEffectCatalog::from_fight(config::configs::get(), &fight),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Command(BattleCommand::Hp(
            crate::engine::manager::hp::HpCommand::Kill(crate::engine::manager::hp::HpKill {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "TestHaloEmitterDeath"),
                },
                source_uid: -1,
                target_uid: 30,
                config_effect: 1,
            }),
        ))],
    )
    .unwrap();

    fn collect(step: &sonettobuf::FightStep, order: &mut Vec<&'static str>) {
        for effect in &step.act_effect {
            if effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Buffdel as i32)
                && effect
                    .buff
                    .as_ref()
                    .is_some_and(|buff| buff.buff_id == Some(31050146))
            {
                order.push("cleanup");
            }
            if effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Dead as i32)
                && effect.target_id == Some(30)
            {
                order.push("death");
            }
            if let Some(child) = effect.fight_step.as_ref() {
                collect(child, order);
            }
        }
    }

    let mut order = Vec::new();
    for step in crate::engine::packet::timeline::project(&result.frames).unwrap() {
        collect(&step, &mut order);
    }
    assert_eq!(order, ["cleanup", "death"]);
}

#[test]
fn deferred_same_owner_batches_share_one_final_completion() {
    let trigger = SkillOpTrigger::Event(BattleEvent::Kind(
        crate::engine::event::kind::EventKind::SkillAction,
    ));
    let queued = |buff_act_id| QueuedOp {
        op: RuleOp::BuffFeatureMarker {
            target_uid: 10,
            effect_type: 357,
            effect_num: 1,
            buff_act_id,
        },
        trigger: trigger.clone(),
        skill_execution: None,
        frame_path: None,
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber: Some(SubscriberContext::new(10)),
        caster_frame: None,
    };
    let action_path = vec![0];
    let batch = |buff_act_id| {
        let mut reactions = ReactionBatch {
            after_hit_skills: vec![queued(buff_act_id)],
            ..Default::default()
        };
        reactions.assign_subscriber_death_groups(Some(&action_path));
        reactions.after_hit_skills
    };
    let mut state = DrainState::new(TargetContext::default());
    state.defer_after_hit_skills(Some(&action_path), batch(701));
    state.defer_after_hit_skills(Some(&action_path), batch(702));
    let queued = state.take_after_hit_skills(Some(&action_path)).unwrap();

    assert_eq!(
        queued
            .iter()
            .filter(|queued| matches!(queued.op, RuleOp::SubscriberGroupComplete))
            .count(),
        1
    );
    assert!(matches!(
        queued.last().map(|queued| &queued.op),
        Some(RuleOp::SubscriberGroupComplete)
    ));
}
