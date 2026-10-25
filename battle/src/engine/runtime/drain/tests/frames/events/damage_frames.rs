use super::*;

#[test]
fn tracked_damage_records_the_indicator_immediately_after_hp() {
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
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.indicator.track_damage(
        crate::engine::manager::indicator::IndicatorId::BossRushScore,
        -1,
    );
    let result = run_command_group(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Command(BattleCommand::Hp(
            crate::engine::manager::hp::HpCommand::Damage(crate::engine::manager::hp::HpDamage {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "TestDamage"),
                },
                source_uid: 10,
                target_uid: -1,
                amount: 75,
                config_effect: 1,
                effect_kind: crate::engine::manager::hp::DamageEffectKind::Normal,
                assassinate: false,
                ignore_riposte: false,
                hurt: crate::engine::manager::hp::HurtInfoData {
                    from_uid: 10,
                    is_crit: false,
                    career_restraint: false,
                    reduce_hp: 75,
                    effect_id: 1,
                    skill_id: 1,
                    damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
                    buff_act_id: 0,
                    buff_uid: 0,
                    hurt_effect_type: 0,
                    display_amount: None,
                },
            }),
        ))],
    )
    .unwrap();

    let items = &result.frames[0].items;
    assert!(matches!(
        (&items[0], &items[1]),
        (FrameItem::Change(hp), FrameItem::Change(indicator))
            if matches!(hp.as_ref(), BattleChange::Hp(_))
                && matches!(indicator.as_ref(), BattleChange::EffectMarker(marker)
                    if marker.target_uid == 4 && marker.effect_num == 75)
    ));
}

#[test]
fn parent_owned_root_output_stays_in_the_root_frame() {
    use crate::engine::skill::behavior::registry::OutputOwner;

    assert_eq!(output_frame_path(OutputOwner::Parent, &[3]), vec![3]);
    assert_eq!(output_frame_path(OutputOwner::Parent, &[3, 4]), vec![3]);
    assert_eq!(output_frame_path(OutputOwner::Skill, &[3, 4]), vec![3, 4]);
}

#[test]
fn bloodtithe_spend_keeps_atomic_changes_in_their_semantic_frames() {
    use crate::engine::{
        manager::{
            buff::BuffGrant,
            gauge::{GaugeCommand, GaugeKey, GaugeKind, GaugeOperation, GaugeOwner},
        },
        mechanic::bloodtithe::spend::SpendCommand,
        runtime::record::FrameItem,
        skill::rule::SetupStage,
    };

    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60210, "ConsumeBloodAddBuff"),
    };
    let gauge_key = GaugeKey {
        kind: GaugeKind::Bloodtithe,
        owner: GaugeOwner::Team(1),
    };
    managers
        .gauge
        .execute_command(GaugeCommand::new(
            origin,
            gauge_key,
            GaugeOperation::Enable { max: Some(10) },
        ))
        .unwrap();
    managers
        .gauge
        .execute_command(GaugeCommand::new(
            origin,
            gauge_key,
            GaugeOperation::ChangeValue { delta: 1 },
        ))
        .unwrap();

    let trigger = FrameTrigger::Setup {
        stage: SetupStage::RoundStart,
        priority: 3,
    };
    let mut frames = Vec::new();
    let parent_path = push_root(&mut frames, FrameOwner::SetupMechanic, trigger.clone());
    let skill_path = push_child(
        &mut frames,
        &parent_path,
        FrameOwner::Skill {
            source_uid: 10,
            skill_id: 100,
            card_index: -1,
            target_uid: Some(20),
        },
        trigger.clone(),
    );
    let mut queue = VecDeque::from([QueuedOp {
        op: RuleOp::Command(BattleCommand::BloodtitheSpend(SpendCommand {
            gauge: GaugeCommand::new(origin, gauge_key, GaugeOperation::ChangeValue { delta: -1 }),
            buff: BuffCommand::Grant(BuffGrant {
                origin,
                source_uid: 10,
                target_uid: 20,
                buff_id: 31260151,
                amount: Some(1),
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        })),
        trigger: SkillOpTrigger::Setup {
            stage: SetupStage::RoundStart,
            priority: 3,
        },
        skill_execution: None,
        frame_path: Some(skill_path.clone()),
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber: None,
        caster_frame: None,
    }]);

    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 31260181,
        slots: Vec::new(),
    });
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

    assert!(matches!(
        result.events.as_slice(),
        [BattleEvent::GaugeChanged(_), BattleEvent::BuffAdded(_)]
    ));
    assert_eq!(result.frames[0].trigger, trigger);
    let FrameItem::Child(skill) = &result.frames[0].items[0] else {
        panic!("expected the pre-existing skill frame")
    };
    assert_eq!(skill.trigger, trigger);
    assert!(result.frames[0].items.iter().any(
        |item| matches!(item, FrameItem::Change(change) if matches!(change.as_ref(), BattleChange::Gauge(_)))
    ));
    assert!(skill.items.iter().any(
        |item| matches!(item, FrameItem::Change(change) if matches!(change.as_ref(), BattleChange::Buff(_)))
    ));
}

#[test]
fn one_skill_event_groups_all_of_its_subscribed_rules() {
    use crate::engine::{
        event::{kind::EventKind, subscription::SubscriptionKey},
        skill::subscriber::SkillSubscriber,
    };

    let subscriber = |slot_index, key| SkillSubscriber {
        owner_uid: 10,
        skill_id: 100,
        slot_index: Some(slot_index),
        key: SubscriptionKey::new(EventKind::SkillAction, key),
    };
    let op = || {
        RuleOp::Skill(
            SkillRequest {
                source_uid: 10,
                skill_id: 100,
            }
            .into(),
        )
    };
    let queued = queued_reactions(
        &TargetPool::default(),
        dispatcher::DispatchBatch {
            skills: vec![
                (subscriber(0, DefinitionKey::new(208, "None")), op()),
                (subscriber(1, DefinitionKey::new(210, "None")), op()),
            ],
            ..Default::default()
        },
        &BattleEvent::Kind(EventKind::SkillAction),
        Some(&[0]),
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();

    let [first, second] = queued.as_slice() else {
        panic!("expected two subscribed rules")
    };
    assert!(Rc::ptr_eq(
        first.frame_group.as_ref().unwrap(),
        second.frame_group.as_ref().unwrap(),
    ));
}

#[test]
fn event_sibling_rules_share_one_skill_frame() {
    use crate::engine::{
        event::{kind::EventKind, subscription::SubscriptionKey},
        skill::subscriber::SkillSubscriber,
    };

    let subscriber = |slot_index| SkillSubscriber {
        owner_uid: -1,
        skill_id: 109320103,
        slot_index: Some(slot_index),
        key: SubscriptionKey::new(
            EventKind::BuffRemoved,
            DefinitionKey::new(515005, "BuffIdExpireOnly"),
        ),
    };
    let op = || {
        RuleOp::Skill(
            SkillRequest {
                source_uid: -1,
                skill_id: 109320103,
            }
            .into(),
        )
    };
    let queued = queued_reactions(
        &TargetPool::default(),
        dispatcher::DispatchBatch {
            skills: vec![(subscriber(0), op()), (subscriber(1), op())],
            ..Default::default()
        },
        &BattleEvent::Kind(EventKind::BuffRemoved),
        Some(&[4, 2]),
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();

    let [first, second] = queued.as_slice() else {
        panic!("expected two subscribed rules")
    };
    assert!(first.frame_path.is_none());
    assert_eq!(first.parent_path.as_deref(), Some([4].as_slice()));
    assert!(Rc::ptr_eq(
        first.frame_group.as_ref().unwrap(),
        second.frame_group.as_ref().unwrap(),
    ));
}
