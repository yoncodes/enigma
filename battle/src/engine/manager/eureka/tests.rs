use super::*;

#[test]
fn records_and_resets_round_consumption_and_overflow() {
    let mut manager = EurekaManager::default();
    manager
        .states
        .insert((10, EUREKA_RESOURCE_ID), EurekaState { current: 4, max: 5 });

    manager.add(10, 10, EUREKA_RESOURCE_ID, -3, 0);
    manager.add(10, 10, EUREKA_RESOURCE_ID, 8, 0);

    assert_eq!(
        manager.round_change(10, EUREKA_RESOURCE_ID),
        EurekaRoundChange {
            consumed: 3,
            overflow: 4,
        }
    );
    manager.clear_round_changes_for([10]);
    assert_eq!(
        manager.round_change(10, EUREKA_RESOURCE_ID),
        EurekaRoundChange::default()
    );
}

#[test]
fn assist_boss_power_is_seeded_and_synced_by_the_resource_owner() {
    let mut fight = Fight {
        attacker: Some(sonettobuf::FightTeam {
            assist_boss: Some(FightEntityInfo {
                uid: Some(-1),
                power_infos: vec![PowerInfo {
                    power_id: Some(PowerType::AssistBoss.id()),
                    num: Some(6),
                    max: Some(6),
                }],
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = EurekaManager::default();

    manager.seed(&fight);
    assert_eq!(
        manager.get(-1, PowerType::AssistBoss.id()),
        EurekaState { current: 6, max: 6 }
    );

    manager.set(-1, PowerType::AssistBoss.id(), 0);
    manager.sync_fight(&mut fight);
    assert_eq!(
        fight.attacker.unwrap().assist_boss.unwrap().power_infos[0].num,
        Some(0)
    );
}

#[test]
fn command_publishes_overflow_even_when_the_value_is_capped() {
    use crate::engine::skill::rule::{CommandOrigin, DefinitionKey, RuleDomain};

    let mut manager = EurekaManager::default();
    manager.owners.insert(10);
    manager
        .states
        .insert((10, EUREKA_RESOURCE_ID), EurekaState { current: 5, max: 5 });

    let changes = manager
        .execute_command(EurekaCommand::Change(EurekaChange {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(20010, "AddPower"),
            },
            source_uid: 10,
            target_uid: 10,
            power_id: EUREKA_RESOURCE_ID,
            delta: 3,
            effect_type: 0,
        }))
        .unwrap();

    let EurekaChanges::Changed { change, .. } = changes.clone() else {
        unreachable!()
    };
    assert_eq!(change.applied_delta, 0);
    assert_eq!(change.overflow, 3);
    assert!(matches!(
        changes.events().as_slice(),
        [BattleEvent::EurekaChanged(event)] if event.overflow == 3
    ));
}
