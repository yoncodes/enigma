use super::*;

#[test]
fn consume_buff_by_type_id_emits_one_manager_owned_consume() {
    let managers = BattleManagers::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(50014, "ConsumeBuffByTypeId"),
        vec![530000111, 1],
        Vec::new(),
    );

    let ops = super::super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 20,
            active_skill_id: 530000151,
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
        [RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(
            BuffConsume {
                target_uid: 20,
                selector: BuffSelector::IdOrType(530000111),
                amount: 1,
                depleted: DepletedBuff::Remove,
                ..
            }
        )))]
    ));
}

#[test]
fn consume_buff_by_type_id_2_uses_an_exact_type_selector() {
    let managers = BattleManagers::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(50016, "ConsumeBuffByTypeId2"),
        vec![90001, 1],
        Vec::new(),
    );

    let ops = super::super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 20,
            active_skill_id: 22302341,
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
        [RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(
            BuffConsume {
                target_uid: 20,
                selector: BuffSelector::TypeId(90001),
                amount: 1,
                depleted: DepletedBuff::Remove,
                ..
            }
        )))]
    ));
}

#[test]
fn damage_window_force_expires_each_configured_buff_id_or_type() {
    let managers = BattleManagers::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(60010, "DisperseForce2"),
        vec![31140111, 31140112],
        vec!["31140111,31140112".into()],
    );

    let ops = super::super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 20,
            active_skill_id: 31140131,
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
        [
            RuleOp::Command(BattleCommand::Buff(BuffCommand::ForceExpire(BuffRemove {
                target_uid: 20,
                selector: BuffRemoveSelector::IdOrType(31140111),
                ..
            }))),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::ForceExpire(BuffRemove {
                target_uid: 20,
                selector: BuffRemoveSelector::IdOrType(31140112),
                ..
            })))
        ]
    ));
}

#[test]
fn consume_card_add_buff_emits_consumption_before_the_optional_grant() {
    let mut managers = BattleManagers::default();
    managers.card = crate::engine::manager::card::CardManager::new(vec![
        sonettobuf::CardInfo {
            uid: Some(10),
            skill_id: Some(31280121),
            card_effect: Some(1),
            ..Default::default()
        },
        sonettobuf::CardInfo {
            uid: Some(20),
            skill_id: Some(999),
            card_effect: Some(3),
            ..Default::default()
        },
        sonettobuf::CardInfo {
            uid: Some(10),
            skill_id: Some(31280122),
            card_effect: Some(2),
            ..Default::default()
        },
    ]);
    managers.card.seed(&sonettobuf::Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                uid: Some(10),
                skill_group2: vec![31280121, 31280122, 31280123],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(60222, "ConsumeCardAddBuff"),
        vec![31280113, 10],
        vec!["31280113".into(), "10,15,25".into()],
    );

    let ops = super::super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 31280131,
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
        [
            RuleOp::Command(BattleCommand::Card(CardCommand::ConsumeForEffect(
                crate::engine::manager::card::CardConsumeForEffect {
                    owner_uid: 10,
                    indices,
                    ..
                }
            ))),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                buff_id: 31280113,
                amount: Some(25),
                occurrences: 1,
                child_uid_reservations: 0,
                ..
            })))
        ] if indices == &[0, 2]
    ));

    let destroy_only = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(60222, "ConsumeCardAddBuff"),
        vec![0],
        vec!["0".into(), "0,0,0".into()],
    );
    let destroy_ops = super::super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 31370131,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &TargetPool::default(),
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &destroy_only,
    )
    .unwrap();
    let [RuleOp::Command(BattleCommand::Card(command))] = destroy_ops.as_slice() else {
        panic!("destroy-only behavior must emit only card consumption");
    };

    managers.execute_card(command.clone()).unwrap();
    assert_eq!(
        managers
            .card
            .hand()
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![999]
    );
}

#[test]
fn planet_removal_uses_each_configured_buff_family() {
    let managers = BattleManagers::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(60252, "DisperseForce3"),
        vec![307002112, 307002212, 307001312],
        vec!["307002112".into(), "307002212".into(), "307001312".into()],
    );

    let ops = super::super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 307001333,
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

    let selectors = ops
        .into_iter()
        .map(|op| match op {
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
                target_uid: 10,
                selector: BuffRemoveSelector::IdOrType(buff_id),
                ..
            }))) => buff_id,
            other => panic!("unexpected op: {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(selectors, vec![307002112, 307002212, 307001312]);
}
