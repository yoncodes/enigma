use super::*;

#[test]
fn buff_spread_copies_the_primary_targets_scaled_layers_to_other_enemies() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                team_type: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(100),
                    team_type: Some(2),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(31340022),
                        layer: Some(20),
                        duration: Some(0),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    current_hp: Some(100),
                    team_type: Some(2),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-3),
                    current_hp: Some(100),
                    team_type: Some(2),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(60248, "BuffSpread"),
        vec![31340022, 500],
        Vec::new(),
    );
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 31345111,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();
    let grants = ops
        .iter()
        .filter_map(|op| match op {
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant))) => Some(grant),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        grants
            .iter()
            .map(|grant| (grant.target_uid, grant.amount))
            .collect::<Vec<_>>(),
        vec![(-2, Some(10)), (-3, Some(10))]
    );
}

#[test]
fn buff_sort_by_hp_redistributes_layers_by_manager_hp_and_preserves_uids() {
    crate::test_support::init_config();
    let enemy = |uid, position, hp, buff_uid, layer| FightEntityInfo {
        uid: Some(uid),
        position: Some(position),
        current_hp: Some(hp),
        team_type: Some(2),
        buffs: vec![BuffInfo {
            uid: Some(buff_uid),
            buff_id: Some(31340022),
            layer: Some(layer),
            ..Default::default()
        }],
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                team_type: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                enemy(-1, 1, 300, 20, 5),
                enemy(-2, 2, 100, 21, 30),
                enemy(-3, 3, 200, 22, 25),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(60241, "BuffSortByHp"),
        vec![31340022],
        Vec::new(),
    );
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 31345131,
            transfer_count: 1,
            event: None,
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
        [
            RuleOp::Command(BattleCommand::Buff(BuffCommand::SetAmount(BuffSetAmount {
                target_uid: -1,
                buff_uid: 20,
                amount: BuffAmount::Layer(40),
                ..
            }))),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::SetAmount(BuffSetAmount {
                target_uid: -3,
                buff_uid: 22,
                amount: BuffAmount::Layer(20),
                ..
            }))),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
                target_uid: -2,
                selector: BuffRemoveSelector::Uid(21),
                ..
            })))
        ]
    ));
}

#[test]
fn add_buff_by_hero_id_grants_listed_heroes_the_first_buff_and_others_the_second() {
    crate::test_support::init_config();
    let ally = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(100),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![ally(11, 3124), ally(12, 3109)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(100006, "AddBuffByHeroId"),
        vec![3122, 3124, 2295033, 2295043],
        vec![
            "3122,3124".to_owned(),
            "2295033".to_owned(),
            "2295043".to_owned(),
        ],
    );
    let granted = |target_uid| {
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
        let mut target = crate::engine::skill::target::TargetContext::default();
        let ops = super::super::super::rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid,
                active_skill_id: 312301533,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        )
        .unwrap();
        match ops.as_slice() {
            [RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant)))]
                if grant.target_uid == target_uid =>
            {
                grant.buff_id
            }
            _ => panic!("expected one grant: {ops:?}"),
        }
    };

    // Master Assassin: "70% of effect for [Assassination] characters and 50% for others".
    assert_eq!(granted(11), 2295033);
    assert_eq!(granted(12), 2295043);
}

#[test]
fn add_target_buff_by_poison_focuses_instances_on_the_most_poisoned_enemy() {
    crate::test_support::init_config();
    let enemy = |uid, position| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(2),
        position: Some(position),
        current_hp: Some(100),
        ..Default::default()
    };
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
            entitys: vec![enemy(-1, 1)],
            sp_entitys: vec![enemy(-8, 5)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: crate::engine::skill::rule::DefinitionKey::new(1, "AddBuff"),
    };
    for target_uid in [-1, -8, -8] {
        managers
            .execute_buff(BuffCommand::GrantChild(BuffGrantChild {
                origin,
                source_uid: 10,
                target_uid,
                buff_id: 31040005,
                amount: None,
                params: None,
                act_info: None,
            }))
            .unwrap();
    }
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(60112, "AddTargetBuffByPoison"),
        vec![2, 2, 31040005, 2],
        Vec::new(),
    );
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 0,
            transfer_count: 1,
            event: None,
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
        [
            RuleOp::Command(BattleCommand::Buff(BuffCommand::ReserveChildUids(
                BuffChildUidReservation {
                    target_uid: -8,
                    count: 1,
                    ..
                }
            ))),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::GrantChild(
                BuffGrantChild {
                    target_uid: -8,
                    buff_id: 31040005,
                    ..
                }
            ))),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::GrantChild(
                BuffGrantChild {
                    target_uid: -8,
                    buff_id: 31040005,
                    ..
                }
            )))
        ]
    ));
}

#[test]
fn consume_layer_and_team_grant_uses_the_consumed_amount() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10), entity(11)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: crate::engine::skill::rule::DefinitionKey::new(1, "AddBuff"),
            },
            source_uid: 10,
            target_uid: 10,
            buff_id: 31270101,
            amount: Some(5),
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(
            60260,
            "ConsumeBuffLayerAndOtherAddBuff",
        ),
        vec![103, 31270101, 3, 31270308],
        Vec::new(),
    );
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 31270135,
            transfer_count: 1,
            event: None,
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
        [
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                target_uid: 10,
                amount: 3,
                ..
            }))),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                target_uid: 10,
                buff_id: 31270308,
                amount: Some(3),
                ..
            }))),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                target_uid: 11,
                buff_id: 31270308,
                amount: Some(3),
                ..
            })))
        ]
    ));
}
