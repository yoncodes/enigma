use super::*;

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
fn hurt_skill_reactions_are_once_per_attacked_owner_per_action() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(1_000),
        passive_skill: vec![1_183_004],
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                skill_group2: vec![31_490_121],
                ..entity(10)
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1), entity(-2), entity(-3)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let hit = |target_uid| {
        BattleEvent::Hit(crate::engine::event::payload::HitEvent {
            origin: CommandOrigin {
                domain: RuleDomain::Skill,
                key: DefinitionKey::new(1, "Damage"),
            },
            source_uid: 10,
            target_uid,
            skill_id: 31_490_121,
            amount: 100,
            resolved_damage: 100,
            shield_absorbed: 0,
            career_restraint: false,
            damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
            share_count: 0,
            assassinate: false,
            ignore_riposte: false,
        })
    };
    let events = [hit(-1), hit(-1), hit(-2)];
    let mut frames = Vec::new();
    let root = push_root(&mut frames, FrameOwner::Command, FrameTrigger::Active);

    let reactions = dispatch_event_batch(
        &pool,
        &managers,
        &catalog,
        &mut RoundDeterminism::default(),
        &events,
        &root,
        &root,
        Some(&root),
        Some((10, 31_490_121, Some(-1))),
        false,
        true,
        crate::engine::event::subscription::PublicationPhase::AfterPublish,
        None,
    )
    .unwrap();
    let mut queue = std::collections::VecDeque::from(reactions.attacked_skills);
    drain_queue_with_frames(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            active_skill_id: 31_490_121,
            active_skill_source_uid: 10,
            active_skill_is_attack: true,
            ..Default::default()
        },
        &mut queue,
        frames,
    )
    .unwrap();

    assert_eq!(managers.buff.buff_id_amount(-1, 11_830_041), 1);
    assert_eq!(managers.buff.buff_id_amount(-2, 11_830_041), 1);
    assert_eq!(managers.buff.buff_id_amount(-3, 11_830_041), 0);
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

    let queued = |mut reactions: ReactionBatch| {
        let attacked = std::mem::take(&mut reactions.attacked_skills);
        reactions
            .into_ordered()
            .into_iter()
            .chain(attacked)
            .collect::<Vec<_>>()
    };
    let first = queued(first);
    let second = queued(second);
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
