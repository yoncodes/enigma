use super::*;

#[test]
fn barcarola_buff_behaviors_require_their_configured_operands() {
    assert!(supports_duration_change(&ParsedBehavior::new(
        20005,
        "AddBuffRound",
        vec![31080131, 1],
    )));
    assert!(!supports_duration_change(&ParsedBehavior::new(
        20005,
        "AddBuffRound",
        vec![31080131, 0],
    )));
    assert!(supports_consume_power_add_multi_buff(&ParsedBehavior::new(
        60150,
        "ConsumePowerAddMultiBuff1",
        vec![2, 3, 31080131, 1, 1, 31080111, 31080111],
    )));
    assert!(!supports_consume_power_add_multi_buff(
        &ParsedBehavior::new(60150, "ConsumePowerAddMultiBuff1", vec![2, 3, 31080131],)
    ));
}

#[test]
fn descriptor_reports_all_multi_buff_dependencies() {
    let behavior = ParsedBehavior::new(
        60150,
        "ConsumePowerAddMultiBuff1",
        vec![5, 3, 101, 1, 2, 102, 103],
    );

    assert_eq!(references(&behavior).buffs, vec![101, 102, 103]);
}

#[test]

fn replace_buff2_compiles_to_one_grant_without_consuming_sources() {
    crate::test_support::init_config();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(60176, "ReplaceBuff2"),
        vec![31140143, 10, 2],
        vec![
            "31140111,31140112".into(),
            "31140143".into(),
            "10".into(),
            "2".into(),
        ],
    );
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(11),
                current_hp: Some(100),
                buffs: vec![
                    BuffInfo {
                        uid: Some(1),
                        buff_id: Some(31140111),
                        layer: Some(12),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(2),
                        buff_id: Some(31140112),
                        layer: Some(9),
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
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let ops = super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 11,
            source_team: 1,
            target_uid: 11,
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
    let [RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant)))] = ops.as_slice() else {
        panic!("expected one buff grant");
    };

    assert_eq!(
        grant.origin.key,
        crate::engine::skill::rule::DefinitionKey::new(60176, "ReplaceBuff2")
    );
    assert_eq!((grant.buff_id, grant.amount), (31140143, Some(2)));
    managers.execute_buff(BuffCommand::Grant(*grant)).unwrap();
    assert_eq!(managers.buff.buff_id_amount(11, 31140111), 12);
    assert_eq!(managers.buff.buff_id_amount(11, 31140112), 9);
    assert_eq!(managers.buff.buff_id_amount(11, 31140143), 2);
}

#[test]
fn replace_buff_uses_the_configured_counter_threshold_and_manager_transaction() {
    crate::test_support::init_config();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(50032, "ReplaceBuff"),
        vec![30810108, 10, 30810101, 30810114],
        vec![
            "30810108".into(),
            "10".into(),
            "30810101".into(),
            "30810114".into(),
        ],
    );
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(11),
                current_hp: Some(100),
                buffs: vec![
                    BuffInfo {
                        uid: Some(1),
                        buff_id: Some(30810108),
                        layer: Some(10),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(2),
                        buff_id: Some(30810101),
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
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 11,
            source_team: 1,
            target_uid: 11,
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
    let [RuleOp::Command(BattleCommand::Buff(command @ BuffCommand::Replace(_)))] = ops.as_slice()
    else {
        panic!("expected one buff replacement");
    };

    managers.execute_buff(command.clone()).unwrap();
    assert_eq!(managers.buff.buff_id_amount(11, 30810101), 0);
    assert_eq!(managers.buff.buff_id_amount(11, 30810114), 1);

    let ops = super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 11,
            source_team: 1,
            target_uid: 11,
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

    assert!(ops.is_empty());
}

#[test]
fn remove_buff_to_add_buff_replaces_only_an_existing_configured_source() {
    crate::test_support::init_config();
    let behavior = ParsedBehavior::new(60029, "RemoveBuffToAddBuff", vec![30870311, 30870321]);
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(11),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(30870311),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 11,
            source_team: 1,
            target_uid: 11,
            active_skill_id: 30870331,
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
    let [RuleOp::Command(BattleCommand::Buff(command @ BuffCommand::Replace(_)))] = ops.as_slice()
    else {
        panic!("expected one buff replacement");
    };

    managers.execute_buff(command.clone()).unwrap();
    assert!(!managers.buff.has_buff_id(11, 30870311));
    assert!(managers.buff.has_buff_id(11, 30870321));
}
