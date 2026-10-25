use super::*;

#[test]
fn random_pool_add_emits_distinct_configured_buff_commands() {
    crate::test_support::init_config();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(20021, "AddBuffRanId"),
        vec![30630111, 2],
        Vec::new(),
    );
    let managers = BattleManagers::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = super::super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 0,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &TargetPool::default(),
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();
    let buff_ids = ops
        .iter()
        .filter_map(|op| match op {
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant))) => Some(grant.buff_id),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(buff_ids.len(), 2);
    assert_ne!(buff_ids[0], buff_ids[1]);
    assert!(buff_ids.iter().all(|id| (30630112..=30630115).contains(id)));
}

#[test]
fn random_type_pool_replays_the_captured_configured_choice() {
    crate::test_support::init_config();
    let managers = BattleManagers::default();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(20022, "AddBuffRanTypeId"),
        vec![30830151, 1],
        Vec::new(),
    );
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_random_buffs([308301512]);
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = super::super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 30830161,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &TargetPool::default(),
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
            BuffGrant {
                buff_id: 308301512,
                ..
            }
        )))]
    ));
}

#[test]
fn random_pool_arguments_require_a_configured_distinct_draw() {
    crate::test_support::init_config();
    let behavior = |args| {
        ParsedBehavior::from_spec(
            crate::engine::skill::behavior::classify::BehaviorSpec::new(20021, "AddBuffRanId"),
            args,
            Vec::new(),
        )
    };

    assert!(super::super::grant::supports_random_pool(&behavior(vec![
        22301851, 5
    ])));
    assert!(!super::super::grant::supports_random_pool(&behavior(vec![
        22301851, 0
    ])));
    assert!(!super::super::grant::supports_random_pool(&behavior(vec![
        0, 5
    ])));
}

#[test]
fn ordinary_add_buff_rule_executes_without_a_battle_intent() {
    crate::test_support::init_config();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(1, "AddBuff"),
        vec![101, 2],
        Vec::new(),
    );
    let mut managers = BattleManagers::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let ops = super::super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 0,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &TargetPool::default(),
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();
    let [RuleOp::Command(BattleCommand::Buff(command))] = ops.as_slice() else {
        panic!("expected one buff command");
    };
    let changes = managers.execute_buff(command.clone()).unwrap();

    assert!(matches!(
        changes.events().as_slice(),
        [crate::engine::event::payload::BattleEvent::BuffAdded(event)]
            if event.source_uid == 10
                && event.target_uid == 10
                && event.buff_id == 101
                && event.after_amount == 2
    ));
}

#[test]
fn enemy_burn_conversion_uses_the_settlement_transaction() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }],
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
    let managers = BattleManagers::seeded(&fight);
    let event = crate::engine::event::payload::BattleEvent::BuffsSettled(vec![
        crate::engine::event::payload::BuffChangeEvent {
            source_uid: 10,
            target_uid: -1,
            buff_uid: 20,
            buff_id: 4150001,
            before_amount: 11,
            after_amount: 5,
            act_id: 0,
            act_value: 0,
            duration_expired: false,
        },
    ]);
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(
            50035,
            "AddBuffBasedOnEnemyBurnUseCount",
        ),
        vec![30810108, 1000],
        Vec::new(),
    );
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = super::super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 0,
            transfer_count: 1,
            event: Some(&event),
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Buff(
            BuffCommand::Accumulate(BuffGrant {
                buff_id: 30810108,
                amount: Some(6),
                ..
            })
        ))]
    ));
}

#[test]
fn add_buff_round_extends_existing_type_family_without_granting_a_new_buff() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(31080132),
                    duration: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(20005, "AddBuffRound"),
        vec![31080131, 2],
        Vec::new(),
    );
    let command = change_duration_command(10, &behavior).unwrap();
    let BuffCommand::ChangeDuration(change) = &command else {
        panic!("expected one duration change");
    };

    assert_eq!(change.selector, BuffSelector::IdOrType(31080131));
    assert_eq!(
        change.origin.key,
        crate::engine::skill::rule::DefinitionKey::new(20005, "AddBuffRound")
    );
    let changes = managers.execute_buff(command).unwrap();
    assert!(changes.change.added.is_none());
    assert_eq!(changes.change.refreshed.len(), 1);
    assert_eq!(changes.change.refreshed[0].before.duration, Some(1));
    assert_eq!(changes.change.refreshed[0].after.duration, Some(3));
}

#[test]
fn channel_count_reduction_updates_the_matching_buff_types_private_state() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(31000441),
                    duration: Some(0),
                    ex_info: Some(7),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(
            60094,
            "ReduceCastChannelCount",
        ),
        vec![31000131, 2],
        Vec::new(),
    );

    let command = reduce_channel_count_command(&managers, 10, &behavior).unwrap();
    let changes = managers.execute_buff(command).unwrap();

    assert!(changes.change.added.is_none());
    assert_eq!(changes.change.refreshed.len(), 1);
    assert_eq!(changes.change.refreshed[0].before.ex_info, Some(7));
    assert_eq!(changes.change.refreshed[0].after.ex_info, Some(5));
    assert_eq!(changes.change.refreshed[0].after.duration, Some(0));
}

#[test]
fn add_buff_duration_refreshes_matching_buff_types_to_the_configured_duration() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![
                    BuffInfo {
                        uid: Some(1),
                        buff_id: Some(31270501),
                        duration: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(2),
                        buff_id: Some(31270501),
                        duration: Some(0),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(3),
                        buff_id: Some(31270501),
                        duration: Some(5),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(60145, "AddBuffDuration"),
        vec![5001, 3],
        Vec::new(),
    );
    let command = refresh_duration_command(10, &behavior).unwrap();
    let BuffCommand::RefreshDurationBySelector(refresh) = &command else {
        panic!("expected one duration refresh");
    };

    assert_eq!(refresh.selector, BuffSelector::IdOrType(5001));
    assert_eq!(refresh.minimum_duration, 3);

    let changes = managers.execute_buff(command.clone()).unwrap();

    assert_eq!(changes.change.refreshed.len(), 2);
    assert_eq!(changes.change.refreshed[0].before.duration, Some(1));
    assert_eq!(changes.change.refreshed[0].after.duration, Some(3));
    assert_eq!(changes.change.refreshed[1].before.duration, Some(5));
    assert_eq!(changes.change.refreshed[1].after.duration, Some(5));
    assert_eq!(
        managers
            .buff
            .active_for(10)
            .find(|active| active.uid == Some(2))
            .and_then(|active| active.duration),
        Some(0)
    );

    let changes = managers.execute_buff(command).unwrap();
    assert_eq!(changes.change.refreshed.len(), 2);
    assert_eq!(changes.change.refreshed[0].before.duration, Some(3));
    assert_eq!(changes.change.refreshed[0].after.duration, Some(3));
}
