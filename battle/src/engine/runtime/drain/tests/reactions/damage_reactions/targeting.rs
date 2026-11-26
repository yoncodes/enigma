use super::*;

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
        added_type_total: None,
        act_id: 0,
        act_value: 0,
        replaced_fixed_max_hp: None,
        replaced_buff_id: None,
        expiry: crate::engine::event::payload::BuffExpiry::Forced,
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
                    buff_id: Some(301),
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
            .before_after_hit_observers
            .iter()
            .filter(|queued| !matches!(queued.op, RuleOp::SubscriberGroupComplete))
            .count(),
        1
    );
    assert!(matches!(
        reactions.before_after_hit_observers[0].op,
        RuleOp::Command(BattleCommand::Buff(BuffCommand::ConsumeAttackCharge(_)))
    ));
    assert!(matches!(
        reactions
            .before_after_hit_observers
            .last()
            .map(|queued| &queued.op),
        Some(RuleOp::SubscriberGroupComplete)
    ));
}

#[test]
fn target_bullet_proc_spends_a_charge_after_granting_the_debuff() {
    crate::test_support::init_config();
    let source = FightEntityInfo {
        uid: Some(10),
        team_type: Some(1),
        current_hp: Some(10_000),
        attr: Some(HeroAttribute {
            attack: Some(100),
            hp: Some(10_000),
            ..Default::default()
        }),
        skill_group1: vec![30920131],
        buffs: [(30, 30920156, 2), (31, 31020117, 0)]
            .into_iter()
            .map(|(uid, buff_id, count)| BuffInfo {
                uid: Some(uid),
                buff_id: Some(buff_id),
                count: (count > 0).then_some(count),
                from_uid: Some(10),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![source],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: [-1, -2]
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
                .to_vec(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut determinism = RoundDeterminism::default();
    for spends_charge in [true, false] {
        let result = run_action(
            &mut managers,
            &pool,
            &catalog,
            &mut determinism,
            TargetContext::default(),
            [],
            SkillInvocation {
                mode: SkillExecutionMode::Active,
                target: SkillTarget::Explicit(-1),
                ..SkillRequest {
                    source_uid: 10,
                    skill_id: 30920131,
                }
                .into()
            },
        )
        .unwrap();

        let steps =
            crate::engine::packet::timeline::project_for_version(&result.frames, 7).unwrap();
        let attack = steps
            .iter()
            .find_map(|step| crate::test_support::find_step(step, 30920131))
            .unwrap();
        let frames = &attack.act_effect;
        let grant = frames
            .iter()
            .position(|effect| {
                effect.fight_step.as_ref().is_some_and(|step| {
                    step.act_effect
                        .iter()
                        .any(|effect| effect.buff_act_id == Some(503))
                })
            })
            .unwrap();
        let spend = frames.iter().position(|effect| {
            effect.fight_step.as_ref().is_some_and(|step| {
                step.act_effect.iter().any(|effect| {
                    effect
                        .buff
                        .as_ref()
                        .is_some_and(|buff| buff.uid == Some(30) && buff.count == Some(1))
                })
            })
        });
        if spends_charge {
            assert!(spend.is_some_and(|spend| spend > grant));
        } else {
            assert_eq!(spend, None);
        }
        assert_eq!(
            managers.buff.snapshot(10, 30).and_then(|buff| buff.count),
            Some(1)
        );
        assert!(managers.buff.has_active_buff_id_or_type(-1, 30920158));
        assert!(!managers.buff.has_active_buff_id_or_type(-2, 30920158));
        assert!(result.events.iter().any(|event| matches!(event,
            BattleEvent::Hit(hit) if hit.skill_id == 30920131 && hit.target_uid == -2
        )));
    }
}
