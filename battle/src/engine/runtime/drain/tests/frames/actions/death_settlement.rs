use super::*;

#[test]
fn scheduled_defender_cleanup_policy_keeps_player_cleanup_in_the_death_frame() {
    let entity = |uid, team_type| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(1),
        attr: Some(HeroAttribute {
            hp: Some(1),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let card = |skill_id| CardInfo {
        uid: Some(10),
        skill_id: Some(skill_id),
        ..Default::default()
    };
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![card(100)],
            draw_pile: vec![card(200)],
            deck_num: 2,
        }))
        .unwrap();
    managers.card.add_temp_card_for(10, 300, None, 0, 1);
    managers
        .execute_card(CardCommand::SetTeamCards(
            crate::engine::manager::card::CardSetTeamCards {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "TestTeamCards"),
                },
                cards: vec![card(400)],
            },
        ))
        .unwrap();
    managers
        .execute_card(CardCommand::SetAiQueue(
            crate::engine::manager::card::CardSetAiQueue {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "TestAiQueue"),
                },
                cards: vec![CardInfo {
                    uid: Some(-1),
                    skill_id: Some(500),
                    ..Default::default()
                }],
            },
        ))
        .unwrap();

    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![
            SkillEffectSlot::new(
                ParsedBehavior::from_spec(
                    BehaviorSpec::new(30005, "LostLife"),
                    vec![1, AttrId::CurrentHp as i32, 1000],
                    Vec::new(),
                ),
                TargetRequest::self_only(),
            ),
            SkillEffectSlot::new(
                ParsedBehavior::from_spec(
                    BehaviorSpec::new(30014, "OriginDamage"),
                    vec![0, AttrId::CurrentHp as i32, 0],
                    Vec::new(),
                ),
                TargetRequest::self_only(),
            ),
        ],
    });
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: -1,
        skill_id: 100,
    }
    .into();
    invocation.target = SkillTarget::Explicit(10);
    let mut queue = VecDeque::from([QueuedOp {
        op: RuleOp::Skill(invocation),
        trigger: SkillOpTrigger::Active,
        skill_execution: None,
        frame_path: None,
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber: None,
        caster_frame: None,
    }]);
    let result = drain_queue_with_frames_and_cleanup_owner(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &mut queue,
        Vec::new(),
        DefenderCardCleanupOwner::ScheduledRoundPhase,
    )
    .unwrap();

    assert!(managers.card.hand().is_empty());
    assert!(managers.card.draw_pile().is_empty());
    assert!(managers.card.generated().is_empty());
    assert!(managers.card.team_cards().is_empty());
    assert_eq!(managers.card.ai_queue()[0].uid, Some(-1));

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let effects = &steps[0].act_effect;
    let death = effects
        .iter()
        .position(|effect| {
            effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Dead as i32)
                && effect.target_id == Some(10)
        })
        .unwrap();
    let removal = effects[death + 1].fight_step.as_ref().unwrap();
    assert_eq!(
        effects[death + 1].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Fightstep as i32)
    );
    assert_eq!(
        removal
            .act_effect
            .iter()
            .map(|effect| (effect.effect_type, effect.target_id, effect.team_type))
            .collect::<Vec<_>>(),
        vec![(
            Some(sonettobuf::effect_type_enum::EffectType::Removeentitycards as i32),
            Some(10),
            Some(1),
        )]
    );
}

#[test]
fn non_action_death_settles_after_its_reaction_and_card_cleanup() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3002),
                team_type: Some(1),
                current_hp: Some(10_000),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(100101),
                team_type: Some(2),
                current_hp: Some(1),
                passive_skill: vec![2345],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::SetAiQueue(
            crate::engine::manager::card::CardSetAiQueue {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "TestAiQueue"),
                },
                cards: vec![CardInfo {
                    uid: Some(-1),
                    skill_id: Some(500),
                    ..Default::default()
                }],
            },
        ))
        .unwrap();
    let origin = CommandOrigin {
        domain: RuleDomain::Lifecycle,
        key: DefinitionKey::new(0, "TestDeathSettlement"),
    };
    let result = run(
        &mut managers,
        &pool,
        &SkillEffectCatalog::from_fight(config::configs::get(), &fight),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Command(BattleCommand::Hp(
            crate::engine::manager::hp::HpCommand::Kill(crate::engine::manager::hp::HpKill {
                origin,
                source_uid: 10,
                target_uid: -1,
                config_effect: 1,
            }),
        ))],
    )
    .unwrap();

    fn collect(step: &sonettobuf::FightStep, order: &mut Vec<&'static str>) {
        if step.act_id == Some(2345) {
            order.push("reaction");
        }
        for effect in &step.act_effect {
            match effect.effect_type {
                Some(value)
                    if value
                        == sonettobuf::effect_type_enum::EffectType::Removeentitycards as i32 =>
                {
                    order.push("cleanup");
                }
                Some(value) if value == sonettobuf::effect_type_enum::EffectType::Dead as i32 => {
                    order.push("death");
                }
                _ => {}
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
    assert_eq!(order, ["reaction", "cleanup", "death"]);
    assert!(managers.card.ai_queue().is_empty());
}

#[test]
fn subscriber_group_finishes_before_death_settlement() {
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
    managers
        .execute_card(CardCommand::SetAiQueue(
            crate::engine::manager::card::CardSetAiQueue {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "TestAiQueue"),
                },
                cards: vec![CardInfo {
                    uid: Some(-1),
                    skill_id: Some(500),
                    ..Default::default()
                }],
            },
        ))
        .unwrap();
    let trigger = SkillOpTrigger::Event(BattleEvent::Kind(
        crate::engine::event::kind::EventKind::RoundEndEntitySettlement,
    ));
    let mut frames = Vec::new();
    let root = push_root(&mut frames, FrameOwner::EventRule, frame_trigger(&trigger));
    let queued = |op| QueuedOp {
        op,
        trigger: trigger.clone(),
        skill_execution: None,
        frame_path: Some(root.clone()),
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber: Some(SubscriberContext::new(-1)),
        caster_frame: None,
    };
    let mut reactions = ReactionBatch {
        after_publish: vec![
            queued(RuleOp::BuffFeatureMarker {
                target_uid: -1,
                effect_type: 137,
                effect_num: 1,
                buff_act_id: 701,
            }),
            queued(RuleOp::Command(BattleCommand::Hp(
                crate::engine::manager::hp::HpCommand::Kill(crate::engine::manager::hp::HpKill {
                    origin: CommandOrigin {
                        domain: RuleDomain::BuffAct,
                        key: DefinitionKey::new(701, "TestDamage"),
                    },
                    source_uid: 10,
                    target_uid: -1,
                    config_effect: 1,
                }),
            ))),
            queued(RuleOp::Command(BattleCommand::Hp(
                crate::engine::manager::hp::HpCommand::Heal(crate::engine::manager::hp::HpHeal {
                    origin: CommandOrigin {
                        domain: RuleDomain::BuffAct,
                        key: DefinitionKey::new(702, "TestRevive"),
                    },
                    source_uid: 10,
                    target_uid: -1,
                    amount: 1,
                    config_effect: 1,
                    kind: crate::engine::manager::hp::HpHealKind::Revive,
                }),
            ))),
            queued(RuleOp::Command(BattleCommand::Hp(
                crate::engine::manager::hp::HpCommand::Kill(crate::engine::manager::hp::HpKill {
                    origin: CommandOrigin {
                        domain: RuleDomain::BuffAct,
                        key: DefinitionKey::new(703, "TestDamageAgain"),
                    },
                    source_uid: 11,
                    target_uid: -1,
                    config_effect: 1,
                }),
            ))),
        ],
        after_settlement: vec![queued(RuleOp::BuffFeatureMarker {
            target_uid: -1,
            effect_type: 357,
            effect_num: 2,
            buff_act_id: 702,
        })],
        ..Default::default()
    };
    reactions.assign_subscriber_death_groups(None);
    let mut queue = reactions.into_ordered().into();
    let result = drain_queue_with_frames(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &mut queue,
        frames,
    )
    .unwrap();

    fn collect(step: &sonettobuf::FightStep, order: &mut Vec<&'static str>) {
        for effect in &step.act_effect {
            match (effect.buff_act_id, effect.effect_type) {
                (Some(701), _) => order.push("first"),
                (Some(702), _) => order.push("second"),
                (_, Some(value))
                    if value
                        == sonettobuf::effect_type_enum::EffectType::Removeentitycards as i32 =>
                {
                    order.push("cleanup");
                }
                (_, Some(value))
                    if value == sonettobuf::effect_type_enum::EffectType::Dead as i32 =>
                {
                    order.push("death");
                }
                _ => {}
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
    assert_eq!(order, ["first", "cleanup", "second", "death"]);
    assert_eq!(
        result
            .outcomes
            .iter()
            .filter_map(|outcome| match outcome {
                crate::engine::runtime::executor::RuleOutcome::DeathSettlement(death) => {
                    Some(*death)
                }
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
fn subscriber_group_does_not_settle_a_revived_target() {
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
    let trigger = SkillOpTrigger::Event(BattleEvent::Kind(
        crate::engine::event::kind::EventKind::RoundEndEntitySettlement,
    ));
    let queued = |op| QueuedOp {
        op,
        trigger: trigger.clone(),
        skill_execution: None,
        frame_path: None,
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber: Some(SubscriberContext::new(-1)),
        caster_frame: None,
    };
    let origin = |opcode, name| CommandOrigin {
        domain: RuleDomain::BuffAct,
        key: DefinitionKey::new(opcode, name),
    };
    let mut reactions = ReactionBatch {
        after_publish: vec![
            queued(RuleOp::Command(BattleCommand::Hp(
                crate::engine::manager::hp::HpCommand::Kill(crate::engine::manager::hp::HpKill {
                    origin: origin(701, "TestDamage"),
                    source_uid: 10,
                    target_uid: -1,
                    config_effect: 1,
                }),
            ))),
            queued(RuleOp::Command(BattleCommand::Hp(
                crate::engine::manager::hp::HpCommand::Heal(crate::engine::manager::hp::HpHeal {
                    origin: origin(702, "TestRevive"),
                    source_uid: 10,
                    target_uid: -1,
                    amount: 1,
                    config_effect: 1,
                    kind: crate::engine::manager::hp::HpHealKind::Revive,
                }),
            ))),
        ],
        ..Default::default()
    };
    reactions.assign_subscriber_death_groups(None);
    let result = drain_queue_with_frames(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &mut reactions.into_ordered().into(),
        Vec::new(),
    )
    .unwrap();

    assert_eq!(managers.hp.current(-1), 1);
    assert!(!result.outcomes.iter().any(|outcome| matches!(
        outcome,
        crate::engine::runtime::executor::RuleOutcome::DeathSettlement(_)
    )));
}
