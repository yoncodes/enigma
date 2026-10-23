use super::*;

#[test]
fn rapport_debuff_projects_add_remove_and_recast_in_capture_order() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3146),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(31460003),
                    layer: Some(3),
                    from_uid: Some(10),
                    ..Default::default()
                }],
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
                    uid: Some(-1),
                    buff_id: Some(31460003),
                    layer: Some(8),
                    from_uid: Some(-1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "AddBuff"),
    };
    let grant = || {
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: 10,
            target_uid: -1,
            buff_id: 31460212,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        })))
    };
    let remove = || {
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
            origin,
            target_uid: -1,
            selector: BuffRemoveSelector::ExactId(31460212),
        })))
    };
    let projected_types = |result: &crate::engine::runtime::drain::DrainResult| {
        fn collect(effect: &sonettobuf::ActEffect, types: &mut Vec<i32>) {
            let relevant = [
                sonettobuf::effect_type_enum::EffectType::Buffadd as i32,
                sonettobuf::effect_type_enum::EffectType::Buffdel as i32,
                sonettobuf::effect_type_enum::EffectType::Attr as i32,
            ];
            if effect
                .effect_type
                .is_some_and(|kind| relevant.contains(&kind))
            {
                types.push(effect.effect_type.unwrap());
            }
            if let Some(step) = &effect.fight_step {
                for nested in &step.act_effect {
                    collect(nested, types);
                }
            }
        }

        let mut types = Vec::new();
        for effect in crate::engine::packet::timeline::project(&result.frames)
            .unwrap()
            .iter()
            .flat_map(|step| &step.act_effect)
        {
            collect(effect, &mut types);
        }
        types
    };
    let added = sonettobuf::effect_type_enum::EffectType::Buffadd as i32;
    let deleted = sonettobuf::effect_type_enum::EffectType::Buffdel as i32;
    let attr = sonettobuf::effect_type_enum::EffectType::Attr as i32;

    let add = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [grant()],
    )
    .unwrap();
    assert_eq!(projected_types(&add), vec![added, attr]);
    assert_eq!(
        managers.persistent_attribute_delta(-1, AttrId::CriticalDef),
        -240
    );

    let recast = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [remove(), grant()],
    )
    .unwrap();
    assert_eq!(projected_types(&recast), vec![deleted, added, attr]);
    assert_eq!(
        managers.persistent_attribute_delta(-1, AttrId::CriticalDef),
        -240
    );
}

#[test]
fn moxie_gain_waits_for_the_normal_card_refill() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ex_point: Some(4),
                ex_skill: Some(900),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![CardInfo {
                uid: Some(10),
                skill_id: Some(101),
                ..Default::default()
            }],
            draw_pile: Vec::new(),
            deck_num: 1,
        }))
        .unwrap();

    run(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Command(BattleCommand::ExPoint(
            ExPointCommand::Change(ExPointChange {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "UltimateRefillTest"),
                },
                source_uid: 10,
                target_uid: 10,
                delta: 1,
                config_effect: 0,
                effect_type: 0,
            }),
        ))],
    )
    .unwrap();

    assert!(
        managers
            .card
            .hand()
            .iter()
            .all(|card| card.skill_id != Some(900))
    );
}

#[test]
fn assist_boss_attack_passive_resolves_from_the_derived_skill_cast_event() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            assist_boss: Some(FightEntityInfo {
                uid: Some(-1),
                team_type: Some(1),
                current_hp: Some(999_999),
                attr: Some(HeroAttribute {
                    hp: Some(999_999),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                passive_skill: vec![12_720_012],
                ..Default::default()
            }),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: [-2, -3]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    team_type: Some(2),
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let event = BattleEvent::SkillAction(crate::engine::skill::action::SkillActionEvent {
        source_uid: -1,
        skill_id: 370_001_002,
        target_uid: -2,
        target_uids: vec![-2, -3],
        attacked_target_uids: vec![-2, -3],
        phase: crate::engine::skill::action::SkillPhase::HitPassives,
        skill_slot: -1,
        is_attack: true,
        rank: 1,
        skill_type: 0,
        effect_tag: 2,
        assassinate: false,
        ignore_riposte: false,
        damage_amount: 1,
        kill_count: 0,
        crit_count: 0,
        guard_break_count: 0,
        additional_moxie: 0,
        extra_skill_kind: 0,
        mode: crate::engine::skill::action::SkillExecutionMode::Active,
        teammate_injury_count: 0,
        teammate_injury_count_not_reset: 0,
        team_injury_count_round: 0,
        card_enchants: Vec::new(),
        buff_additions: Vec::new(),
    });

    run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        event,
    )
    .unwrap();

    assert!(managers.hp.current(-2) < 10_000);
    assert!(managers.hp.current(-3) < 10_000);
}

#[test]
fn random_additional_target_passive_expands_the_configured_attack() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            assist_boss: Some(FightEntityInfo {
                uid: Some(-1),
                team_type: Some(1),
                current_hp: Some(999_999),
                attr: Some(HeroAttribute {
                    hp: Some(999_999),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                passive_skill: vec![370_002_190],
                ..Default::default()
            }),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: [-2, -3]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    team_type: Some(2),
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_condition_random_choices(vec![
        crate::engine::runtime::determinism::ConditionRandomChoice {
            skill_id: 370_001_002,
            opcode: 552_203,
            roll: 499,
        },
    ]);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: -1,
        skill_id: 370_001_002,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-2);

    let result = run(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut determinism,
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    assert!(managers.hp.current(-2) < 10_000);
    assert!(
        managers.hp.current(-3) < 10_000,
        "marker={} frames={:#?}",
        managers.buff.has_buff_id(-1, 370_002_190),
        result.frames
    );
    assert!(!managers.buff.has_buff_id(-1, 370_002_190));
}

#[test]
fn active_skill_publishes_exact_phase_to_its_psychube_passive() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3107),
                    team_type: Some(1),
                    career: Some(6),
                    current_hp: Some(100),
                    skill_group1: vec![31070111],
                    passive_skill: vec![435011],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    model_id: Some(3074),
                    team_type: Some(1),
                    current_hp: Some(100),
                    passive_skill: vec![2270001],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                career: Some(6),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 31070111,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    invocation.card_index = 1;

    let result = run(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    assert!(managers.buff.has_buff_id(10, 31070111));
    assert!(managers.buff.has_buff_id(10, 435011));
    assert!(!managers.buff.has_buff_id(11, 90071));
    assert_eq!(
        managers
            .buff
            .active_for(10)
            .find(|buff| buff.buff_id == Some(31070111))
            .and_then(|buff| buff.act_common_params.as_deref()),
        Some("1003#0")
    );

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let root = steps
        .iter()
        .find(|step| step.act_id == Some(31070111))
        .expect("the active skill owns its packet frame");
    assert_eq!(root.to_id, Some(-1));
    assert_eq!(
        root.act_effect
            .iter()
            .take(3)
            .map(|effect| effect.effect_type)
            .collect::<Vec<_>>(),
        vec![
            Some(sonettobuf::effect_type_enum::EffectType::Buffadd as i32),
            Some(sonettobuf::effect_type_enum::EffectType::None as i32),
            Some(sonettobuf::effect_type_enum::EffectType::Fightstep as i32),
        ]
    );
    let passive = root.act_effect[2]
        .fight_step
        .as_ref()
        .expect("the psychube reaction stays nested under the active skill");
    assert_eq!(passive.act_id, Some(435011));
    assert_eq!(passive.from_id, Some(10));
    assert_eq!(passive.to_id, Some(-1));
}

#[test]
fn reactive_skill_frame_targets_the_other_team_of_a_hit() {
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
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-2),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-3),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let event = BattleEvent::Hit(crate::engine::event::payload::HitEvent {
        origin: CommandOrigin {
            domain: RuleDomain::Skill,
            key: DefinitionKey::new(100, "SkillDamage"),
        },
        source_uid: 10,
        target_uid: -2,
        skill_id: 100,
        amount: 50,
        resolved_damage: 50,
        shield_absorbed: 0,
        career_restraint: false,
        damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
        share_count: 0,
        assassinate: false,
        ignore_riposte: false,
    });

    assert_eq!(reaction_counterparty(&pool, &event, -2), Some(10));
    assert_eq!(reaction_counterparty(&pool, &event, -3), Some(10));
    assert_eq!(reaction_counterparty(&pool, &event, 10), Some(-2));
    assert_eq!(
        reaction_skill_target(
            &pool,
            &event,
            -3,
            crate::engine::skill::condition::registry::ReactionFrameTarget::Counterparty,
            None,
        ),
        Some(10)
    );
    assert_eq!(
        reaction_skill_target(
            &pool,
            &event,
            -3,
            crate::engine::skill::condition::registry::ReactionFrameTarget::CausingFrame,
            Some(-2),
        ),
        Some(-2)
    );
    assert_eq!(
        reaction_skill_target(
            &pool,
            &event,
            -3,
            crate::engine::skill::condition::registry::ReactionFrameTarget::CausingFrame,
            None,
        ),
        Some(-2)
    );
}

#[test]
fn side_state_reactions_anchor_their_frame_to_the_applier() {
    let pool = TargetPool::from_fight(&Fight::default());
    let event = BattleEvent::BuffRemoved(crate::engine::event::payload::BuffChangeEvent {
        source_uid: -1,
        target_uid: crate::engine::fight::rules::ATTACKER_SIDE_UID,
        buff_uid: 20,
        buff_id: 109320001,
        before_amount: 1,
        after_amount: 0,
        act_id: 0,
        act_value: 0,
        duration_expired: true,
    });

    assert_eq!(reaction_counterparty(&pool, &event, -1), Some(-1));
}

#[test]
fn attack_consumption_keeps_first_hit_entity_order() {
    let hit = |source_uid, target_uid| {
        BattleEvent::Hit(crate::engine::event::payload::HitEvent {
            origin: CommandOrigin {
                domain: RuleDomain::Skill,
                key: DefinitionKey::new(100, "SkillDamage"),
            },
            source_uid,
            target_uid,
            skill_id: 100,
            amount: 50,
            resolved_damage: 50,
            shield_absorbed: 0,
            career_restraint: false,
            damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
            share_count: 0,
            assassinate: false,
            ignore_riposte: false,
        })
    };
    let events = [hit(10, -2), hit(10, -1), hit(11, -2)];

    assert_eq!(
        ordered_hit_entities(&events, |hit| hit.source_uid),
        [10, 11]
    );
    assert_eq!(
        ordered_hit_entities(&events, |hit| hit.target_uid),
        [-2, -1]
    );
}

#[test]
fn attack_consumption_waits_for_hit_reactions() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![BuffInfo {
                    uid: Some(30),
                    buff_id: Some(430111),
                    count: Some(1),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let hit = BattleEvent::Hit(crate::engine::event::payload::HitEvent {
        origin: CommandOrigin {
            domain: RuleDomain::Skill,
            key: DefinitionKey::new(1, "Damage"),
        },
        source_uid: 10,
        target_uid: -1,
        skill_id: 1,
        amount: 100,
        resolved_damage: 100,
        shield_absorbed: 0,
        career_restraint: false,
        damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
        share_count: 0,
        assassinate: false,
        ignore_riposte: false,
    });
    let mut frames = Vec::new();
    let root = push_root(&mut frames, FrameOwner::Command, FrameTrigger::Active);

    let reactions = dispatch_event_batch(
        &pool,
        &managers,
        &catalog,
        &mut RoundDeterminism::default(),
        &[hit],
        &root,
        &root,
        Some(&root),
        Some((10, 1, Some(-1))),
        true,
        true,
        crate::engine::event::subscription::PublicationPhase::AfterPublish,
        None,
    )
    .unwrap();

    assert!(reactions.after_publish.is_empty());
    assert_eq!(
        reactions
            .after_hit
            .iter()
            .filter(|queued| !matches!(queued.op, RuleOp::SubscriberGroupComplete))
            .count(),
        1
    );
    assert!(matches!(
        reactions.after_hit[0].op,
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(_)))
    ));
    assert!(matches!(
        reactions.after_hit.last().map(|queued| &queued.op),
        Some(RuleOp::SubscriberGroupComplete)
    ));
}

#[test]
fn damage_based_rebound_routes_once_and_removes_the_counter() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(1_069),
                    buff_id: Some(117200101),
                    from_uid: Some(-1),
                    count: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let hit = BattleEvent::Hit(crate::engine::event::payload::HitEvent {
        origin: CommandOrigin {
            domain: RuleDomain::Skill,
            key: DefinitionKey::new(1, "Damage"),
        },
        source_uid: 10,
        target_uid: -1,
        skill_id: 1,
        amount: 1_000,
        resolved_damage: 1_000,
        shield_absorbed: 0,
        career_restraint: false,
        damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
        share_count: 0,
        assassinate: false,
        ignore_riposte: false,
    });
    let events = [hit.clone(), hit];
    let mut frames = Vec::new();
    let root = push_root(&mut frames, FrameOwner::Command, FrameTrigger::Active);
    let dispatched = dispatch_event_batch(
        &pool.runtime_view(&managers),
        &managers,
        &catalog,
        &mut RoundDeterminism::default(),
        &events,
        &root,
        &root,
        Some(&root),
        Some((10, 1, Some(-1))),
        false,
        true,
        crate::engine::event::subscription::PublicationPhase::AfterPublish,
        None,
    )
    .unwrap();
    let mut queue = std::collections::VecDeque::from(dispatched.into_ordered());

    assert_eq!(
        queue
            .iter()
            .filter(|queued| !matches!(queued.op, RuleOp::SubscriberGroupComplete))
            .count(),
        3
    );
    assert!(matches!(
        queue.back().map(|queued| &queued.op),
        Some(RuleOp::SubscriberGroupComplete)
    ));
    let result = drain_queue_with_frames(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &mut queue,
        frames,
    )
    .unwrap();

    assert_eq!(managers.hp.current(10), 700);
    assert!(managers.buff.snapshot(-1, 1_069).is_none());
    assert!(result.events.iter().any(|event| matches!(
        event,
        BattleEvent::BuffRemoved(change)
            if change.target_uid == -1 && change.buff_uid == 1_069
    )));
    fn contains_rebound_marker(step: &sonettobuf::FightStep) -> bool {
        step.act_effect.iter().any(|effect| {
            (effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Rebound as i32)
                && effect.buff_act_id == Some(743))
                || effect
                    .fight_step
                    .as_ref()
                    .is_some_and(contains_rebound_marker)
        })
    }
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    assert!(steps.iter().any(contains_rebound_marker));
}

#[test]
fn shared_damage_subscription_carries_event_batch_ownership() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    passive_skill: vec![434111],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1_000),
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
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let origin = CommandOrigin {
        domain: RuleDomain::Skill,
        key: DefinitionKey::new(1, "Damage"),
    };
    let transaction = |target_uid| {
        vec![
            BattleEvent::Hit(crate::engine::event::payload::HitEvent {
                origin,
                source_uid: -1,
                target_uid,
                skill_id: 1,
                amount: 100,
                resolved_damage: 100,
                shield_absorbed: 0,
                career_restraint: false,
                damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
                share_count: 3,
                assassinate: false,
                ignore_riposte: false,
            }),
            BattleEvent::DamageShared {
                origin,
                source_uid: -1,
                target_uid: 11,
                participant_uids: vec![10, 11, 12],
                amount: 25,
                share_count: 3,
                action_share_count: 3,
                damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
            },
        ]
    };
    let mut frames = Vec::new();
    let root = push_root(&mut frames, FrameOwner::Command, FrameTrigger::Active);
    let mut drain_state = DrainState::new(TargetContext::default());
    let mut determinism = RoundDeterminism::default();
    let first = dispatch_event_groups(
        &pool,
        &managers,
        &catalog,
        &mut determinism,
        &mut drain_state,
        &[transaction(11), transaction(10)],
        &root,
        &root,
        Some(&root),
        &root,
        Some((-1, 1, Some(10))),
        true,
        true,
        crate::engine::event::subscription::PublicationPhase::AfterPublish,
        None,
        None,
        None,
    )
    .unwrap();
    let second = dispatch_event_groups(
        &pool,
        &managers,
        &catalog,
        &mut determinism,
        &mut drain_state,
        &[transaction(11), transaction(10)],
        &root,
        &root,
        Some(&root),
        &root,
        Some((-1, 1, Some(10))),
        true,
        true,
        crate::engine::event::subscription::PublicationPhase::AfterPublish,
        None,
        None,
        None,
    )
    .unwrap();

    let first = first.into_ordered();
    let second = second.into_ordered();
    assert_eq!(
        first
            .iter()
            .filter(|queued| matches!(queued.op, RuleOp::SubscriberGroupComplete))
            .count(),
        1
    );
    assert_eq!(
        second
            .iter()
            .filter(|queued| matches!(queued.op, RuleOp::SubscriberGroupComplete))
            .count(),
        1
    );
    assert!(matches!(
        first.last().map(|queued| &queued.op),
        Some(RuleOp::SubscriberGroupComplete)
    ));
    assert!(matches!(
        second.last().map(|queued| &queued.op),
        Some(RuleOp::SubscriberGroupComplete)
    ));

    let casts = first
        .into_iter()
        .chain(second)
        .filter_map(|queued| match queued.op {
            RuleOp::Skill(
                invocation @ SkillInvocation {
                    plan:
                        SkillRequest {
                            skill_id: 434111, ..
                        },
                    ..
                },
            ) => Some((
                queued.trigger,
                invocation.condition_key,
                invocation.condition_slot,
                invocation.reaction_batch_id,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        casts
            .iter()
            .filter(|(trigger, _, _, _)| matches!(
                trigger,
                SkillOpTrigger::Event(BattleEvent::DamageShared { .. })
            ))
            .count(),
        4,
        "{casts:#?}"
    );
    assert_eq!(
        casts
            .iter()
            .filter(|(trigger, _, _, _)| matches!(
                trigger,
                SkillOpTrigger::Event(BattleEvent::DamageShared { .. })
            ))
            .map(|(_, _, _, batch_id)| *batch_id)
            .collect::<Vec<_>>(),
        vec![Some(0), Some(0), Some(1), Some(1)]
    );
}
