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
mod tests;
