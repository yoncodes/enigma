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
            .before_after_hit_observers
            .iter()
            .filter(|queued| !matches!(queued.op, RuleOp::SubscriberGroupComplete))
            .count(),
        1
    );
    assert!(matches!(
        reactions.before_after_hit_observers[0].op,
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(_)))
    ));
    assert!(matches!(
        reactions
            .before_after_hit_observers
            .last()
            .map(|queued| &queued.op),
        Some(RuleOp::SubscriberGroupComplete)
    ));
}
