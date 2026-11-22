use sonettobuf::FightEntityInfo;

use super::*;
use crate::engine::{
    event::bus::EventBus,
    manager::{
        buff::BuffGrant,
        gauge::{GaugeKey, GaugeOwner},
    },
    runtime::executor::{RuleOutcome, execute_rule_op},
    skill::rule::{
        CommandOrigin, DefinitionKey, RuleDomain,
        output::{BattleCommand, RuleOp},
    },
};

const ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::Behavior,
    key: DefinitionKey::new(60210, "ConsumeBloodAddBuff"),
};
const KEY: GaugeKey = GaugeKey {
    kind: GaugeKind::Bloodtithe,
    owner: GaugeOwner::Team(1),
};

#[test]
fn committed_spend_preserves_outputs_and_cannot_overdraw() {
    crate::test_support::init_config();
    let mut managers = BattleManagers::default();
    for uid in [10, 20] {
        managers.register_entity(&FightEntityInfo {
            uid: Some(uid),
            team_type: Some(1),
            current_hp: Some(100),
            ..Default::default()
        });
    }
    managers
        .gauge
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::Enable { max: Some(10) },
        ))
        .unwrap();
    managers
        .gauge
        .execute_command(GaugeCommand::new(
            ORIGIN,
            KEY,
            GaugeOperation::ChangeValue { delta: 1 },
        ))
        .unwrap();

    let command = SpendCommand {
        gauge: GaugeCommand::new(ORIGIN, KEY, GaugeOperation::ChangeValue { delta: -1 }),
        buff: BuffCommand::Grant(BuffGrant {
            origin: ORIGIN,
            source_uid: 10,
            target_uid: 20,
            buff_id: 31260151,
            amount: Some(1),
            occurrences: 1,
            child_uid_reservations: 0,
        }),
    };

    let mut events = EventBus::default();
    let outcome = execute_rule_op(
        &mut managers,
        &mut events,
        RuleOp::Command(BattleCommand::BloodtitheSpend(command.clone())),
    )
    .unwrap();
    assert!(matches!(outcome, RuleOutcome::BloodtitheSpend(_)));
    assert!(matches!(
        events.pop(),
        Some(crate::engine::event::payload::BattleEvent::GaugeChanged(_))
    ));
    assert!(matches!(
        events.pop(),
        Some(crate::engine::event::payload::BattleEvent::BuffAdded(_))
    ));
    assert!(events.is_empty());

    let repeated = execute_rule_op(
        &mut managers,
        &mut events,
        RuleOp::Command(BattleCommand::BloodtitheSpend(command)),
    )
    .unwrap();
    assert!(matches!(repeated, RuleOutcome::StateChanged));
    assert_eq!(managers.gauge.get(KEY).unwrap().current, 0);
    assert_eq!(managers.buff.buff_id_amount(20, 31260151), 1);
}
