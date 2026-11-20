use sonettobuf::BuffActInfo;

use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrant, BuffSetState},
        card::{CardCommand, CardConsumeForEffect, CardConsumptionKind, CardUnlockHedone},
    },
    mechanic::hedone_energy::{self, HedoneEnergyAbsorption, HedoneEnergyRule, HedoneEnergyState},
    skill::{
        buff_act::{BuffActRuleError, BuffActRuleOp, registry::RuntimeContext},
        rule::{
            DefinitionKey,
            output::{BattleCommand, RuleOp},
        },
        subscriber::BuffActSubscriber,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HedoneCardConfig {
    pub skill_id: i32,
    pub card_limit: i32,
    pub reward_buff_id: i32,
    pub grants_reward: bool,
    pub absorption_start_limit: i32,
    pub storage_limit: i32,
    pub unlock_threshold: i32,
    pub power_add: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HedoneCardState {
    pub owner_uid: i64,
    pub buff_uid: i64,
    pub act_id: i32,
    pub team_type: i32,
    pub stored: i32,
    pub maximum: i32,
    pub unlocked: bool,
    pub config: HedoneCardConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HedoneCardFailure {
    InvalidContext,
    MissingState,
    AmbiguousState,
    InvalidState,
    MissingEnergyConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HedoneCardError {
    pub key: DefinitionKey,
    pub owner_uid: i64,
    pub buff_uid: i64,
    pub buff_id: i32,
    pub failure: HedoneCardFailure,
}

pub fn parse(args: &[i32]) -> Option<HedoneCardConfig> {
    let [
        skill_id,
        card_limit,
        reward_buff_id,
        reward,
        absorption_start_limit,
        storage_limit,
        unlock_threshold,
        power_add,
    ] = args
    else {
        return None;
    };
    (*skill_id > 0
        && *card_limit > 0
        && *reward_buff_id > 0
        && matches!(*reward, 0 | 1)
        && *absorption_start_limit > 0
        && *storage_limit > 0
        && *absorption_start_limit <= *storage_limit
        && *unlock_threshold > 0
        && *unlock_threshold <= *storage_limit
        && *power_add > 0)
        .then_some(HedoneCardConfig {
            skill_id: *skill_id,
            card_limit: *card_limit,
            reward_buff_id: *reward_buff_id,
            grants_reward: *reward == 1,
            absorption_start_limit: *absorption_start_limit,
            storage_limit: *storage_limit,
            unlock_threshold: *unlock_threshold,
            power_add: *power_add,
        })
}

pub fn supports(args: &[i32]) -> bool {
    parse(args).is_some()
}

fn error(subscriber: &BuffActSubscriber, failure: HedoneCardFailure) -> HedoneCardError {
    HedoneCardError {
        key: subscriber.key.definition,
        owner_uid: subscriber.owner_uid,
        buff_uid: subscriber.buff_uid,
        buff_id: subscriber.buff_id,
        failure,
    }
}

fn state(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
) -> Result<HedoneCardState, HedoneCardError> {
    let config = parse(&subscriber.args)
        .ok_or_else(|| error(subscriber, HedoneCardFailure::InvalidContext))?;
    let buff = managers
        .buff
        .snapshot(subscriber.owner_uid, subscriber.buff_uid)
        .ok_or_else(|| error(subscriber, HedoneCardFailure::MissingState))?;
    let mut states = buff
        .act_info
        .iter()
        .filter(|info| info.act_id == Some(subscriber.key.definition.opcode));
    let info = states
        .next()
        .ok_or_else(|| error(subscriber, HedoneCardFailure::MissingState))?;
    if states.next().is_some() || info.str_param.as_deref() != Some("") {
        return Err(error(subscriber, HedoneCardFailure::AmbiguousState));
    }
    let [stored, maximum, unlocked] = info.param.as_slice() else {
        return Err(error(subscriber, HedoneCardFailure::InvalidState));
    };
    if *stored < 0
        || *stored > *maximum
        || *maximum != config.storage_limit
        || !matches!(*unlocked, 0 | 1)
        || (*unlocked == 1) != (*stored >= config.unlock_threshold)
    {
        return Err(error(subscriber, HedoneCardFailure::InvalidState));
    }
    Ok(HedoneCardState {
        owner_uid: subscriber.owner_uid,
        buff_uid: subscriber.buff_uid,
        act_id: subscriber.key.definition.opcode,
        team_type: subscriber.team_type,
        stored: *stored,
        maximum: *maximum,
        unlocked: *unlocked == 1,
        config,
    })
}

pub fn owner_state(
    managers: &BattleManagers,
    owner_uid: i64,
) -> Result<Option<HedoneCardState>, HedoneCardError> {
    let mut subscribers = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| {
            feature.owner_uid == owner_uid
                && super::is_kind(feature, super::registry::BuffActKind::HedoneCard)
        })
        .filter_map(|feature| super::subscriber_from_feature(feature, EventKind::RoundEnd));
    let Some(subscriber) = subscribers.next() else {
        return Ok(None);
    };
    if subscribers.next().is_some() {
        return Err(error(&subscriber, HedoneCardFailure::AmbiguousState));
    }
    state(managers, &subscriber).map(Some)
}

fn selected_cards(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    state: HedoneCardState,
) -> Result<Option<HedoneEnergyAbsorption>, HedoneCardError> {
    let candidates = managers
        .card
        .conduit_energy_cards()
        .ok_or_else(|| error(subscriber, HedoneCardFailure::MissingEnergyConfig))?;
    Ok(hedone_energy::absorb(
        HedoneEnergyRule {
            card_limit: state.config.card_limit as usize,
            absorption_start_limit: state.config.absorption_start_limit,
            unlock_threshold: state.config.unlock_threshold,
        },
        HedoneEnergyState {
            stored: state.stored,
            maximum: state.maximum,
            unlocked: state.unlocked,
        },
        candidates,
    ))
}

fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Result<Vec<RuleOp>, HedoneCardError> {
    if !super::subscriber_is_kind(subscriber, super::registry::BuffActKind::HedoneCard)
        || event.kind() != EventKind::RoundEnd
        || subscriber.team_type != 1
    {
        return Err(error(subscriber, HedoneCardFailure::InvalidContext));
    }
    if !subscriber.owner_alive {
        return Ok(Vec::new());
    }
    let state = state(managers, subscriber)?;
    let Some(absorption) = selected_cards(managers, subscriber, state)? else {
        return Ok(Vec::new());
    };
    let indices = absorption.card_indices;
    let origin = super::command_origin(subscriber)
        .ok_or_else(|| error(subscriber, HedoneCardFailure::InvalidContext))?;
    let mut ops = vec![RuleOp::Command(BattleCommand::Card(
        CardCommand::ConsumeForEffect(CardConsumeForEffect {
            origin,
            owner_uid: subscriber.owner_uid,
            indices: indices.clone(),
            kind: CardConsumptionKind::Hedone,
        }),
    ))];
    if state.config.grants_reward {
        ops.extend(indices.iter().map(|_| {
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                origin,
                source_uid: subscriber.owner_uid,
                target_uid: subscriber.owner_uid,
                buff_id: state.config.reward_buff_id,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            })))
        }));
    }
    if absorption.state.unlocked && !state.unlocked {
        ops.push(RuleOp::Command(BattleCommand::Card(
            CardCommand::UnlockHedone(CardUnlockHedone {
                origin,
                owner_uid: subscriber.owner_uid,
                skill_id: state.config.skill_id,
                team_type: subscriber.team_type,
            }),
        )));
    }
    let params = vec![
        absorption.state.stored,
        absorption.state.maximum,
        i32::from(absorption.state.unlocked),
    ];
    ops.extend([
        RuleOp::Command(BattleCommand::Buff(BuffCommand::SetInternalState(
            BuffSetState {
                origin,
                target_uid: subscriber.owner_uid,
                buff_uid: subscriber.buff_uid,
                ex_info: None,
                params: None,
                act_info: Some(vec![BuffActInfo {
                    act_id: Some(subscriber.key.definition.opcode),
                    param: params.clone(),
                    str_param: Some(String::new()),
                }]),
            },
        ))),
        RuleOp::BuffActInfoMarker(crate::engine::manager::buff::BuffActInfoMarkerResult {
            target_uid: subscriber.owner_uid,
            buff_uid: subscriber.buff_uid,
            act_id: subscriber.key.definition.opcode,
            params,
            str_param: Some(String::new()),
            team_type: 0,
        }),
    ]);
    Ok(ops)
}

pub fn scoped_rule_ops(
    context: &mut RuntimeContext<'_>,
) -> Result<Vec<BuffActRuleOp>, BuffActRuleError> {
    Ok(rule_ops(
        context.managers,
        context.subscriber,
        context.event.expect("runtime handler requires an event"),
    )?
    .into_iter()
    .map(BuffActRuleOp::subscriber)
    .collect())
}

#[cfg(test)]
mod tests;
