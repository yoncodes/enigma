use crate::engine::{
    manager::{
        BattleManagers,
        conduit::{
            ConduitChange, ConduitCommand, ConduitError, ConduitPowerChange, ConduitPowerChangeKind,
        },
    },
    skill::{
        behavior::classify::BehaviorKind,
        buff_act::hedone_card::{HedoneCardError, owner_state},
        effect::SkillEffectCatalog,
        rule::CommandOrigin,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HedoneDevicePowerCommand {
    pub origin: CommandOrigin,
    pub owner_uid: i64,
    pub power_id: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HedoneDevicePowerError {
    InvalidCommand,
    State(HedoneCardError),
    Conduit(ConduitError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolvedPowerGain {
    source_uid: i64,
    team: i32,
    power_id: i32,
    delta: i32,
}

impl From<HedoneCardError> for HedoneDevicePowerError {
    fn from(value: HedoneCardError) -> Self {
        Self::State(value)
    }
}

impl From<ConduitError> for HedoneDevicePowerError {
    fn from(value: ConduitError) -> Self {
        Self::Conduit(value)
    }
}

pub(crate) fn execute(
    managers: &mut BattleManagers,
    command: HedoneDevicePowerCommand,
) -> Result<ConduitChange, HedoneDevicePowerError> {
    let gain = resolve_gain(managers, command.owner_uid, command.power_id)?
        .ok_or(HedoneDevicePowerError::InvalidCommand)?;
    Ok(managers
        .conduit
        .execute(ConduitCommand::ChangePower(ConduitPowerChange {
            origin: command.origin,
            source_uid: gain.source_uid,
            team: gain.team,
            power_id: gain.power_id,
            delta: gain.delta,
            kind: ConduitPowerChangeKind::Standard,
        }))?)
}

pub(crate) fn available_gain(
    managers: &BattleManagers,
    owner_uid: i64,
    power_id: i32,
) -> Result<Option<(i32, i32)>, HedoneDevicePowerError> {
    resolve_gain(managers, owner_uid, power_id)
        .map(|gain| gain.map(|gain| (gain.power_id, gain.delta)))
}

pub(crate) fn owns_skill(catalog: &SkillEffectCatalog, skill_id: i32) -> bool {
    catalog.get(skill_id).is_some_and(|effect| {
        effect
            .slots
            .iter()
            .any(|slot| slot.behavior.spec.kind == BehaviorKind::HedoneAddDevicePower)
    })
}

fn resolve_gain(
    managers: &BattleManagers,
    owner_uid: i64,
    power_id: i32,
) -> Result<Option<ResolvedPowerGain>, HedoneDevicePowerError> {
    if owner_uid == 0 || power_id <= 0 {
        return Err(HedoneDevicePowerError::InvalidCommand);
    }
    let Some(state) = owner_state(managers, owner_uid)? else {
        return Ok(None);
    };
    if !state.unlocked || state.stored <= 0 {
        return Ok(None);
    }
    let delta = state
        .stored
        .checked_mul(state.config.power_add)
        .ok_or(HedoneDevicePowerError::InvalidCommand)?;
    Ok(Some(ResolvedPowerGain {
        source_uid: state.owner_uid,
        team: state.team_type,
        power_id,
        delta,
    }))
}
