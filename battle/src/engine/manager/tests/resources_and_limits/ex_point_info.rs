use super::*;

#[test]
fn ex_point_info_uses_synced_manager_state() {
    let mut fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                ex_point: Some(0),
                current_hp: Some(100),
                attr: Some(HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                ex_point_type: Some(1),
                power_infos: vec![PowerInfo {
                    power_id: Some(2),
                    num: Some(0),
                    max: Some(5),
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };

    let mut managers = BattleManagers::seeded(&fight);
    managers.ex_point.add(10, 10, 4, 0);
    managers.hp.lose(10, 25, 0);
    managers.eureka.set(10, 2, 3);
    managers.sync_entities(&mut fight);

    let info = managers.ex_point_info(&fight);
    assert_eq!(info.len(), 1);
    assert_eq!(info[0].uid, Some(10));
    assert_eq!(info[0].ex_point, Some(4));
    assert_eq!(info[0].current_hp, Some(75));
    assert_eq!(info[0].ex_point_type, Some(1));
    assert_eq!(info[0].power_infos[0].num, Some(3));
}

#[test]
fn ex_point_info_uses_current_roster_in_encounter_order() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(2),
        current_hp: Some(100),
        attr: Some(HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![entity(-1), entity(-2)],
            sub_entitys: vec![entity(-3)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = managers.catalog();
    managers
        .entity
        .replace_team_roster(catalog, 2, &[entity(-3)], &[entity(-2)]);
    managers.sync_entities(&mut fight);

    let uids = managers
        .ex_point_info(&fight)
        .into_iter()
        .filter_map(|info| info.uid)
        .collect::<Vec<_>>();
    assert_eq!(uids, vec![-2, -3]);
}

#[test]
fn ex_point_info_keeps_a_defeated_combatant_replaced_by_a_summon() {
    crate::test_support::init_config();
    let entity = |uid, position, current_hp| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(2),
        position: Some(position),
        current_hp: Some(current_hp),
        attr: Some(HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 1, 100), entity(-2, 2, 0)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let summoned = managers
        .execute_entity(crate::engine::manager::entity::EntityCommand {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60008, "Summon"),
            },
            source_uid: -1,
            target_uid: -1,
            operation: crate::engine::manager::entity::EntityOperation::SummonCombatant {
                model_id: 116_330_100,
                position: 2,
            },
        })
        .unwrap()
        .entity
        .uid
        .unwrap();
    managers.sync_entities(&mut fight);

    assert_eq!(
        managers
            .ex_point_info(&fight)
            .into_iter()
            .filter_map(|info| info.uid)
            .collect::<Vec<_>>(),
        vec![-1, summoned, -2]
    );
}

#[test]
fn buff_commands_do_not_mutate_the_separately_owned_eureka_cap() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                power_infos: vec![PowerInfo {
                    power_id: Some(1),
                    num: Some(0),
                    max: Some(6),
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "PowerMaxAdd"),
    };
    let mut managers = BattleManagers::seeded(&fight);

    managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: 10,
            target_uid: 10,
            buff_id: 31050147,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();

    assert_eq!(managers.eureka.get(10, 1).max, 6);
}

#[test]
fn synchronization_reconnect_state_stays_owned_by_buff_manager() {
    crate::test_support::init_config();
    let mut fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                ex_point_type: Some(2),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(229100),
                    from_uid: Some(10),
                    act_common_params: Some("10000#4,0,0".into()),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(100000, "EzioProps"),
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_ex_point(ExPointCommand::ConfigureSynchronization(
            ExPointConfigureSynchronization {
                origin,
                target_uid: 10,
                definition: SynchronizationDefinition::new([1, 2, 3], 4, 100).unwrap(),
            },
        ))
        .unwrap();

    managers.sync_entities(&mut fight);
    assert_eq!(
        fight.attacker.as_ref().unwrap().entitys[0].buffs[0]
            .act_common_params
            .as_deref(),
        Some("10000#4,0,0")
    );

    managers
        .execute_ex_point(ExPointCommand::RecordSynchronizationAction(
            ExPointRecordSynchronizationAction {
                origin,
                target_uid: 10,
                action_target_uid: -1,
            },
        ))
        .unwrap();
    managers
        .execute_buff(BuffCommand::SetState(
            crate::engine::manager::buff::BuffSetState {
                origin,
                target_uid: 10,
                buff_uid: 1,
                ex_info: None,
                params: Some("10000#3,123,-1".into()),
                act_info: None,
            },
        ))
        .unwrap();

    managers.sync_entities(&mut fight);

    let buff = &fight.attacker.as_ref().unwrap().entitys[0].buffs[0];
    assert_eq!(buff.act_common_params.as_deref(), Some("10000#3,123,-1"));
}
