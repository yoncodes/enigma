use super::*;
use crate::engine::{
    event::payload::EurekaChangeEvent,
    manager::gauge::{GaugeCommand, GaugeOperation},
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain, output::BattleCommand},
};
use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

#[test]
fn eureka_spend_transfers_energy_as_a_buff_transaction() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(2_240_000),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let origin = super::super::configured_command_origin(875, BuffActKind::EmitterTag).unwrap();
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            impromptu::team_energy_key(1),
            GaugeOperation::Enable { max: None },
        ))
        .unwrap();
    let event = BattleEvent::EurekaChanged(EurekaChangeEvent {
        origin: CommandOrigin {
            domain: RuleDomain::BuffAct,
            key: DefinitionKey::new(863, "CreateAdditionalDamage"),
        },
        source_uid: 10,
        target_uid: 10,
        power_id: 1,
        before: 4,
        requested_delta: -2,
        applied_delta: -2,
        after: 2,
        overflow: 0,
    });

    let definition =
        crate::engine::skill::buff_act::registry::transaction_definitions(event.kind())
            .find(|definition| definition.key == DefinitionKey::new(875, "EmitterTag"))
            .expect("EmitterTag owns the Eureka transaction route");
    let ops = definition.transaction.handler.unwrap()(&managers, &event);
    let [(feature, RuleOp::Command(BattleCommand::Gauge(command)))] = ops.as_slice() else {
        panic!("expected one team-energy transaction")
    };
    assert_eq!(feature.buff_id, 2_240_000);
    assert_eq!(command.operation, GaugeOperation::ChangeValue { delta: 2 });
}
