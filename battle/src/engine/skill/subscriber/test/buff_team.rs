use super::*;

#[test]
fn discovers_active_buff_acts_from_effect_time() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                team_type: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(2),
                    buff_id: Some(8112651),
                    from_uid: Some(11),
                    count: Some(3),
                    layer: Some(3),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);

    let subscribers = for_active_buffs(&managers, EventKind::SkillAction);

    assert_eq!(subscribers.len(), 1, "{subscribers:#?}");
    assert!(subscribers.iter().all(|subscriber| {
        subscriber.owner_uid == 10
            && subscriber.source_uid == 11
            && subscriber.buff_uid == 2
            && subscriber.buff_id == 8112651
            && subscriber.team_type == 1
            && subscriber.amount == 3
    }));
    let add_to_target = subscribers
        .iter()
        .find(|subscriber| subscriber.key.definition.opcode == 518)
        .unwrap();
    assert_eq!(
        add_to_target.key,
        SubscriptionKey::at_phase(
            EventKind::SkillAction,
            DefinitionKey::new(518, "AddToTarget"),
            Some(crate::engine::skill::action::SkillPhase::Immediate),
        )
    );
    assert_eq!(add_to_target.act_type, "AddToTarget");
    assert_eq!(add_to_target.effect_time, 201);
    assert_eq!(add_to_target.args, vec![1, 8112652]);
    let completed = for_active_buffs(&managers, EventKind::SkillCast);
    assert_eq!(completed.len(), 2, "{completed:#?}");
    assert!(completed.iter().any(|subscriber| {
        subscriber.key
            == SubscriptionKey::at_phase_and_publication(
                EventKind::SkillCast,
                DefinitionKey::new(503, "AddToTarget"),
                None,
                crate::engine::event::subscription::PublicationPhase::BeforePublish,
            )
            && subscriber.args == vec![1, 4011]
    }));
    assert!(completed.iter().any(|subscriber| {
        subscriber.key
            == SubscriptionKey::new(EventKind::SkillCast, DefinitionKey::new(827, "Bullet"))
            && subscriber.effect_time == 208
            && subscriber.effect_condition == 3
            && subscriber.args.is_empty()
    }));
    assert!(for_active_buffs(&managers, EventKind::RoundEnd).is_empty());

    managers.buff.delete(10, 2);
    assert!(for_active_buffs(&managers, EventKind::SkillAction).is_empty());
}

#[test]
fn exact_team_tags_compile_runtime_and_setup_subscriptions() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                team_type: Some(1),
                buffs: vec![
                    BuffInfo {
                        uid: Some(20),
                        buff_id: Some(2_240_000),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(21),
                        buff_id: Some(6_270_501),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(22),
                        buff_id: Some(31_340_003),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);

    assert_eq!(for_active_buffs(&managers, EventKind::HpLost).len(), 1);
    assert_eq!(for_active_buffs(&managers, EventKind::BuffAdded).len(), 1);
    assert_eq!(
        for_active_buffs(&managers, EventKind::ActionQueueCommitted).len(),
        1
    );
    let battle_start = buff_acts_for_setup_stage(&managers, SetupStage::BattleStart, 0);
    assert_eq!(battle_start.len(), 3);
    assert!(
        battle_start
            .iter()
            .any(|subscriber| { subscriber.key == DefinitionKey::new(875, "EmitterTag") })
    );
    assert!(
        battle_start
            .iter()
            .any(|subscriber| { subscriber.key == DefinitionKey::new(953, "BloodPoolTag") })
    );
    assert!(
        battle_start
            .iter()
            .any(|subscriber| { subscriber.key == DefinitionKey::new(1052, "HeatScaleTag") })
    );
}

#[test]
fn discovers_damage_calculation_buff_acts_without_publishing_a_fake_runtime_event() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(99_998),
                current_hp: Some(100),
                team_type: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(2),
                    buff_id: Some(31080144),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);

    let subscribers = for_damage_calculation(&managers);

    assert_eq!(subscribers.len(), 2);
    assert!(subscribers.iter().all(|subscriber| {
        subscriber.key.event == EventKind::DamageCalculation
            && subscriber.key.definition.opcode == 884
            && buff_act::subscriber_kind(subscriber)
                == Some(buff_act::registry::BuffActKind::AddBuffAfterAttack)
    }));
}

#[test]
fn an_owners_skills_answer_one_event_in_their_own_order_across_kinds() {
    crate::test_support::init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1),
                passive_skill: vec![109380005, 109380003, 109380004],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let catalog = crate::engine::skill::effect::SkillEffectCatalog::from_fight(
        config::configs::get(),
        &fight,
    );

    // A hit publishes TargetAttacked and BeAttacked together; the capture answers it
    // 109380005, 109380003, 109380004, the boss's configured passive order.
    let skills = for_compiled_events(
        &pool,
        &managers,
        &catalog,
        [EventKind::TargetAttacked, EventKind::BeAttacked],
    )
    .unwrap()
    .skills
    .into_iter()
    .map(|subscriber| subscriber.skill_id)
    .fold(Vec::new(), |mut order, skill_id| {
        if !order.contains(&skill_id) {
            order.push(skill_id);
        }
        order
    });

    assert_eq!(skills, vec![109380005, 109380003, 109380004]);
}
