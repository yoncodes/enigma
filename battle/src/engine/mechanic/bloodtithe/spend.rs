use crate::engine::manager::{
    BattleManagers,
    buff::{BuffChanges, BuffCommand, BuffCommandError},
    gauge::{GaugeChange, GaugeCommand, GaugeCommandError, GaugeKind, GaugeOperation},
};

#[derive(Debug, Clone, PartialEq)]
pub struct SpendCommand {
    pub gauge: GaugeCommand,
    pub buff: BuffCommand,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpendChanges {
    pub gauge: GaugeChange,
    pub buff: BuffChanges,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpendError {
    InvalidCommand,
    Gauge(GaugeCommandError),
    Buff(BuffCommandError),
}

impl From<GaugeCommandError> for SpendError {
    fn from(value: GaugeCommandError) -> Self {
        Self::Gauge(value)
    }
}

impl From<BuffCommandError> for SpendError {
    fn from(value: BuffCommandError) -> Self {
        Self::Buff(value)
    }
}

pub(crate) fn execute(
    managers: &mut BattleManagers,
    command: SpendCommand,
) -> Result<Option<SpendChanges>, SpendError> {
    let GaugeOperation::ChangeValue { delta } = command.gauge.operation else {
        return Err(SpendError::InvalidCommand);
    };
    if command.gauge.key.kind != GaugeKind::Bloodtithe || delta >= 0 {
        return Err(SpendError::InvalidCommand);
    }
    let cost = delta.saturating_abs();
    if managers
        .gauge
        .get(command.gauge.key)
        .is_none_or(|state| state.current < cost)
    {
        return Ok(None);
    }

    let buff_plan = managers.plan_buff(command.buff)?;
    let mut next_gauge = managers.gauge.clone();
    let gauge = next_gauge.execute_command(command.gauge)?;
    if gauge.applied_delta != delta {
        return Err(SpendError::InvalidCommand);
    }

    managers.gauge = next_gauge;
    let buff = managers.commit_buff(buff_plan);
    Ok(Some(SpendChanges { gauge, buff }))
}

#[cfg(test)]
mod tests {
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
}
