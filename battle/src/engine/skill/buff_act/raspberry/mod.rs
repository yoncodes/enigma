use sonettobuf::{BuffActInfo, BuffInfo};

use crate::catalog::BattleCatalog;
use crate::engine::{
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{
            ActiveBuffFeature, BuffChanges, BuffCommand, BuffCommandError, BuffGrantChild,
            BuffManager, BuffSetState,
        },
        ex_point::{ExPointChange, ExPointCommand},
        hp::{
            HpChanges, HpCommand, HpCommandError, HpLoss, HurtDamageFromType, HurtInfoData,
            MaxHpAdjust,
        },
    },
    skill::rule::{
        CommandOrigin,
        output::{BattleCommand, RuleOp},
    },
    skill::subscriber::BuffActSubscriber,
};

use super::{is_kind, registry::BuffActKind, subscriber_is_kind};

pub fn attribute_delta(buffs: &BuffManager, owner_uid: i64, attr_id: AttrId) -> i32 {
    let Some(catalog) = buffs.try_catalog().or_else(BattleCatalog::try_global) else {
        return 0;
    };
    let regular = buffs
        .active_for(owner_uid)
        .filter_map(|buff| {
            let current = buff
                .act_common_params
                .as_deref()
                .and_then(parse_capacity)
                .map(|(current, _)| current)?;
            catalog
                .buff_feature_rows(buff.buff_id?)
                .into_iter()
                .find_map(|raw| raspberry_attribute_rates(catalog, raw, attr_id))
                .map(|rates| stepped_attribute(current, rates.regular))
        })
        .sum::<i32>();
    let Some(rates) = buffs.active_for(owner_uid).find_map(|buff| {
        catalog
            .buff_feature_rows(buff.buff_id?)
            .into_iter()
            .find_map(|raw| raspberry_attribute_rates(catalog, raw, attr_id))
    }) else {
        return regular;
    };
    regular
        + buffs
            .active_for(owner_uid)
            .filter_map(|buff| big_skill_points(catalog, buff))
            .map(|points| feast_attribute(points, rates))
            .sum::<i32>()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RaspberryAttributeRates {
    regular: i32,
    feast_tenths: i32,
}

fn raspberry_attribute_rates(
    catalog: BattleCatalog,
    raw: &str,
    attr_id: AttrId,
) -> Option<RaspberryAttributeRates> {
    let mut cells = raw.split('#');
    let act_id = cells.next()?.trim().parse::<i32>().ok()?;
    let args = cells.map(str::to_owned).collect::<Vec<_>>();
    let values = std::iter::once(act_id)
        .chain(parse_feature(&args)?)
        .collect::<Vec<_>>();
    if !is_raspberry_act_id(catalog, values[0]) {
        return None;
    }
    [(5, 6, 10), (7, 8, 11)]
        .into_iter()
        .find(|(attr, _, _)| AttrId::from_raw(values[*attr]) == Some(attr_id))
        .map(|(_, regular, feast)| RaspberryAttributeRates {
            regular: values[regular],
            feast_tenths: values[feast],
        })
}

pub fn parse_feature(raw_args: &[String]) -> Option<Vec<i32>> {
    if raw_args.len() != 11 {
        return None;
    }
    let mut values = Vec::with_capacity(raw_args.len());
    for part in &raw_args[..9] {
        values.push(part.trim().parse::<i32>().ok()?);
    }
    values.push(parse_tenths(raw_args[9].trim())?);
    values.push(parse_tenths(raw_args[10].trim())?);
    Some(values)
}

fn parse_tenths(raw: &str) -> Option<i32> {
    match raw.split_once('.') {
        Some((whole, fraction)) if fraction.len() == 1 => {
            Some(whole.parse::<i32>().ok()? * 10 + fraction.parse::<i32>().ok()?)
        }
        None => Some(raw.parse::<i32>().ok()? * 10),
        _ => None,
    }
}

fn stepped_attribute(points: i32, rate: i32) -> i32 {
    points.max(0) * rate.max(0) / 1_000
}

fn feast_attribute(points: i32, rates: RaspberryAttributeRates) -> i32 {
    let regular = stepped_attribute(points, rates.regular);
    let denominator = rates.regular.saturating_mul(10);
    if denominator <= 0 {
        return 0;
    }
    regular.saturating_mul(rates.feast_tenths.max(0)) / denominator
}

fn big_skill_points(catalog: BattleCatalog, buff: &BuffInfo) -> Option<i32> {
    let act_id = catalog
        .buff_feature_rows(buff.buff_id?)
        .into_iter()
        .find_map(|raw| {
            let act_id = raw.split('#').next()?.parse().ok()?;
            is_big_skill_act_id(catalog, act_id).then_some(act_id)
        })?;
    buff.act_info
        .iter()
        .find(|info| info.act_id == Some(act_id))?
        .param
        .first()
        .copied()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RaspberryBuffAct {
    pub origin: CommandOrigin,
    pub act_id: i32,
    pub owner_uid: i64,
    pub source_uid: i64,
    pub buff_uid: i64,
    pub buff_id: i32,
    pub team_type: i32,
    pub loss_rate: i32,
    pub shared_gain_rate: i32,
    pub max_cap_rate: i32,
}

impl RaspberryBuffAct {
    pub fn from_feature(feature: &ActiveBuffFeature) -> Option<Self> {
        if !feature.owner_alive || !is_kind(feature, BuffActKind::Raspberry) {
            return None;
        }

        Some(Self {
            origin: super::feature_command_origin(feature)?,
            act_id: feature.act_id()?,
            owner_uid: feature.owner_uid,
            source_uid: feature.source_uid,
            buff_uid: feature.buff_uid,
            buff_id: feature.buff_id,
            team_type: feature.team_type,
            loss_rate: rate(feature, 0),
            shared_gain_rate: rate(feature, 2),
            max_cap_rate: rate(feature, 3),
        })
    }

    pub fn from_subscriber(subscriber: &BuffActSubscriber) -> Option<Self> {
        if !subscriber.owner_alive || !subscriber_is_kind(subscriber, BuffActKind::Raspberry) {
            return None;
        }

        Some(Self {
            origin: super::command_origin(subscriber)?,
            act_id: subscriber.key.definition.opcode,
            owner_uid: subscriber.owner_uid,
            source_uid: subscriber.source_uid,
            buff_uid: subscriber.buff_uid,
            buff_id: subscriber.buff_id,
            team_type: subscriber.team_type,
            loss_rate: subscriber.args.first().copied().unwrap_or_default(),
            shared_gain_rate: subscriber.args.get(2).copied().unwrap_or_default(),
            max_cap_rate: subscriber.args.get(3).copied().unwrap_or_default(),
        })
    }

    pub fn source_or_owner_uid(self) -> i64 {
        if self.source_uid != 0 {
            self.source_uid
        } else {
            self.owner_uid
        }
    }

    pub fn loss_from_current_hp(self, current_hp: i32) -> i32 {
        current_hp.max(0) * self.loss_rate.max(0) / 1000
    }

    pub fn shared_gain_from_loss(self, loss: i32) -> i32 {
        loss.max(0) * self.shared_gain_rate.max(0) / 1000
    }

    pub fn max_cap_from_source_hp(self, source_max_hp: i32) -> i32 {
        source_max_hp.max(0) * self.max_cap_rate.max(0) / 1000
    }

    pub fn crossed_cap(self, before: i32, after: i32, cap: i32) -> bool {
        cap > 0 && before < cap && after >= cap
    }

    pub fn capacity(self, buff: &BuffInfo, fallback_cap: i32) -> (i32, i32) {
        self.capacity_inner(BattleCatalog::try_global(), buff, fallback_cap)
    }

    fn capacity_inner(
        self,
        catalog: Option<BattleCatalog>,
        buff: &BuffInfo,
        fallback_cap: i32,
    ) -> (i32, i32) {
        let parsed = buff
            .act_common_params
            .as_deref()
            .and_then(parse_capacity)
            .or_else(|| {
                buff.act_info
                    .iter()
                    .find(|info| {
                        info.act_id.is_some_and(|act_id| {
                            catalog.is_some_and(|catalog| is_raspberry_act_id(catalog, act_id))
                        })
                    })
                    .and_then(|info| Some((*info.param.first()?, *info.param.get(1)?)))
            });
        parsed
            .map(|(current, cap)| (current, if cap > 0 { cap } else { fallback_cap }))
            .unwrap_or((0, fallback_cap))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapacityCommand {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub buff_uid: i64,
    pub buff_act_id: i32,
    pub current: i32,
    pub cap: i32,
    pub delta: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddCountCommand {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub attr_id: AttrId,
    pub rate: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CapacityChanges {
    pub ex_point: Option<crate::engine::manager::ex_point::ExPointChanges>,
    pub buff: BuffChanges,
    pub hp: HpChanges,
    pub buff_uid: i64,
    pub buff_act_id: i32,
    pub current: i32,
    pub cap: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapacityAtCap {
    pub target_uid: i64,
    pub buff_uid: i64,
    pub buff_act_id: i32,
    pub current: i32,
    pub cap: i32,
    pub max_hp: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CapacityResult {
    Applied(Box<CapacityChanges>),
    AtCap(CapacityAtCap),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapacityError {
    InvalidCommand,
    Buff(BuffCommandError),
    Hp(HpCommandError),
    ExPoint(crate::engine::manager::ex_point::ExPointCommandError),
}

impl From<BuffCommandError> for CapacityError {
    fn from(value: BuffCommandError) -> Self {
        Self::Buff(value)
    }
}

impl From<HpCommandError> for CapacityError {
    fn from(value: HpCommandError) -> Self {
        Self::Hp(value)
    }
}

impl From<crate::engine::manager::ex_point::ExPointCommandError> for CapacityError {
    fn from(value: crate::engine::manager::ex_point::ExPointCommandError) -> Self {
        Self::ExPoint(value)
    }
}

pub fn add_count_rule_ops(
    managers: &BattleManagers,
    origin: CommandOrigin,
    source_uid: i64,
    target_uid: i64,
    attr_id: AttrId,
    rate: i32,
) -> Option<Vec<RuleOp>> {
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| feature.owner_uid == target_uid)
        .find_map(|feature| RaspberryBuffAct::from_feature(&feature))?;
    Some(vec![RuleOp::Command(BattleCommand::RaspberryAddCount(
        AddCountCommand {
            origin,
            source_uid,
            target_uid,
            attr_id,
            rate,
        },
    ))])
}

pub(crate) fn execute_add_count(
    managers: &mut BattleManagers,
    command: AddCountCommand,
) -> Result<Option<CapacityResult>, CapacityError> {
    let act = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| feature.owner_uid == command.target_uid)
        .find_map(|feature| RaspberryBuffAct::from_feature(&feature))
        .ok_or(CapacityError::InvalidCommand)?;
    let buff = managers
        .buff
        .snapshot(command.target_uid, act.buff_uid)
        .ok_or(CapacityError::InvalidCommand)?;
    let fallback_cap = act.max_cap_from_source_hp(managers.hp.max(command.source_uid));
    let (current, cap) = act.capacity_inner(Some(managers.catalog()), &buff, fallback_cap);
    let attr_value = match command.attr_id {
        AttrId::CurrentHp => managers.hp.current(command.source_uid),
        AttrId::Hp => managers.hp.max(command.target_uid),
        _ => return Err(CapacityError::InvalidCommand),
    };
    let gain = attr_value.max(0) * command.rate.max(0) / 1000;
    let next = if cap > 0 {
        (current + gain).min(cap)
    } else {
        current + gain
    };
    let delta = next - current;
    if delta == 0 {
        return Ok(
            (gain > 0 && cap > 0 && current >= cap).then_some(CapacityResult::AtCap(
                CapacityAtCap {
                    target_uid: command.target_uid,
                    buff_uid: act.buff_uid,
                    buff_act_id: act.act_id,
                    current,
                    cap,
                    max_hp: managers.hp.max(command.target_uid),
                },
            )),
        );
    }
    let ex_point = if act.crossed_cap(current, next, cap) {
        Some(
            managers.execute_ex_point(ExPointCommand::Change(ExPointChange {
                origin: command.origin,
                source_uid: command.source_uid,
                target_uid: command.source_uid,
                delta: 1,
                config_effect: 0,
                effect_type: 0,
            }))?,
        )
    } else {
        None
    };
    let mut changes = execute_capacity(
        managers,
        CapacityCommand {
            origin: command.origin,
            source_uid: command.source_uid,
            target_uid: command.target_uid,
            buff_uid: act.buff_uid,
            buff_act_id: act.act_id,
            current: next,
            cap,
            delta,
        },
    )?;
    changes.ex_point = ex_point;
    Ok(Some(CapacityResult::Applied(Box::new(changes))))
}

pub fn capacity_rule_op(
    origin: CommandOrigin,
    source_uid: i64,
    act: RaspberryBuffAct,
    current: i32,
    cap: i32,
    delta: i32,
) -> Option<RuleOp> {
    (delta != 0).then_some({
        RuleOp::Command(BattleCommand::RaspberryCapacity(CapacityCommand {
            origin,
            source_uid,
            target_uid: act.owner_uid,
            buff_uid: act.buff_uid,
            buff_act_id: act.act_id,
            current,
            cap,
            delta,
        }))
    })
}

pub(crate) fn execute_capacity(
    managers: &mut BattleManagers,
    command: CapacityCommand,
) -> Result<CapacityChanges, CapacityError> {
    if command.source_uid == 0
        || command.target_uid == 0
        || command.buff_uid == 0
        || command.delta == 0
    {
        return Err(CapacityError::InvalidCommand);
    }
    let buff = managers.execute_buff(BuffCommand::SetState(BuffSetState {
        ex_info: None,
        origin: command.origin,
        target_uid: command.target_uid,
        buff_uid: command.buff_uid,
        params: Some(format!("{}#{}", command.current, command.cap)),
        act_info: Some(vec![BuffActInfo {
            act_id: Some(command.buff_act_id),
            param: vec![command.current, command.cap],
            str_param: Some(String::new()),
        }]),
    }))?;
    let hp = managers.execute_hp(HpCommand::AdjustMax(MaxHpAdjust {
        origin: command.origin,
        source_uid: command.source_uid,
        target_uid: command.target_uid,
        delta: command.delta,
    }))?;
    Ok(CapacityChanges {
        ex_point: None,
        buff,
        hp,
        buff_uid: command.buff_uid,
        buff_act_id: command.buff_act_id,
        current: command.current,
        cap: command.cap,
    })
}

pub fn big_skill_rule_ops(
    managers: &BattleManagers,
    origin: CommandOrigin,
    source_uid: i64,
    target_uid: i64,
    transfer_rate: i32,
    buff_id: i32,
) -> Option<Vec<RuleOp>> {
    if source_uid == 0 || target_uid == 0 || transfer_rate < 0 || buff_id <= 0 {
        return None;
    }
    let act_id = big_skill_act_id(managers.catalog(), buff_id)?;
    let Some(team) = managers.buff.team_type(target_uid) else {
        return Some(Vec::new());
    };

    let mut transferred = 0_i64;
    let mut ops = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter_map(|feature| {
            let act = RaspberryBuffAct::from_feature(&feature)?;
            if act.team_type != team || act.owner_uid == target_uid {
                return None;
            }
            let buff = managers.buff.snapshot(act.owner_uid, act.buff_uid)?;
            let fallback_cap =
                act.max_cap_from_source_hp(managers.hp.max(act.source_or_owner_uid()));
            let (current, cap) = act.capacity_inner(Some(managers.catalog()), &buff, fallback_cap);
            if current <= 0 {
                return None;
            }
            transferred += i64::from(current);
            Some(RuleOp::Command(BattleCommand::RaspberryCapacity(
                CapacityCommand {
                    origin,
                    source_uid,
                    target_uid: act.owner_uid,
                    buff_uid: act.buff_uid,
                    buff_act_id: act.act_id,
                    current: 0,
                    cap,
                    delta: -current,
                },
            )))
        })
        .collect::<Vec<_>>();
    let bonus = transferred
        .saturating_mul(i64::from(transfer_rate))
        .saturating_div(1000)
        .clamp(0, i64::from(i32::MAX)) as i32;

    ops.push(RuleOp::Command(BattleCommand::Buff(
        BuffCommand::GrantStateful(BuffGrantChild {
            origin,
            source_uid,
            target_uid,
            buff_id,
            amount: None,
            params: None,
            act_info: Some(vec![BuffActInfo {
                act_id: Some(act_id),
                param: vec![bonus],
                str_param: Some(String::new()),
            }]),
        }),
    )));
    if bonus > 0 {
        ops.push(RuleOp::Command(BattleCommand::Hp(HpCommand::AdjustMax(
            MaxHpAdjust {
                origin,
                source_uid,
                target_uid,
                delta: bonus,
            },
        ))));
    }
    Some(ops)
}

fn big_skill_act_id(catalog: BattleCatalog, buff_id: i32) -> Option<i32> {
    catalog
        .buff_feature_rows(buff_id)
        .into_iter()
        .find_map(|raw| {
            let act_id = raw.split('#').next()?.parse().ok()?;
            is_big_skill_act_id(catalog, act_id).then_some(act_id)
        })
}

pub fn round_start_rule_op(
    managers: &BattleManagers,
    feature: &ActiveBuffFeature,
) -> Option<RuleOp> {
    let act = RaspberryBuffAct::from_feature(feature)?;
    let amount = act.loss_from_current_hp(managers.hp.current(act.owner_uid));
    (amount > 0).then(|| {
        RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(HpLoss {
            origin: super::feature_command_origin(feature)
                .expect("a registered Raspberry feature has an origin"),
            source_uid: act.source_or_owner_uid(),
            target_uid: act.owner_uid,
            amount,
            config_effect: 0,
            hurt: Some(HurtInfoData {
                from_uid: act.source_or_owner_uid(),
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 0,
                skill_id: 0,
                damage_from: HurtDamageFromType::Buff,
                buff_act_id: act.act_id,
                buff_uid: act.buff_uid,
                hurt_effect_type: 0,
                display_amount: None,
            }),
        })))
    })
}

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !subscriber_is_kind(subscriber, BuffActKind::Raspberry) {
        return None;
    }
    if let BattleEvent::BuffRemoved(change) = event {
        if !is_big_skill_act_id(managers.catalog(), change.act_id)
            || change.act_value <= 0
            || managers.buff.team_type(change.target_uid) != Some(subscriber.team_type)
            || !super::is_primary_team_subscriber(managers, subscriber, BuffActKind::Raspberry)
        {
            return Some(Vec::new());
        }
        return Some(vec![RuleOp::Command(BattleCommand::Hp(
            HpCommand::AdjustMax(MaxHpAdjust {
                origin: super::command_origin(subscriber)?,
                source_uid: if subscriber.source_uid != 0 {
                    subscriber.source_uid
                } else {
                    subscriber.owner_uid
                },
                target_uid: change.target_uid,
                delta: -change.act_value,
            }),
        ))]);
    }
    if !matches!(event, BattleEvent::RoundStart) {
        return Some(Vec::new());
    }
    let Some(act) = RaspberryBuffAct::from_subscriber(subscriber) else {
        return Some(Vec::new());
    };
    Some(
        round_start_loss_rule_op(managers, act)
            .into_iter()
            .collect(),
    )
}

pub fn round_start_loss_rule_op(
    managers: &BattleManagers,
    act: RaspberryBuffAct,
) -> Option<RuleOp> {
    let amount = act.loss_from_current_hp(managers.hp.current(act.owner_uid));
    let origin = act.origin;
    (amount > 0).then(|| {
        RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(HpLoss {
            origin,
            source_uid: act.source_or_owner_uid(),
            target_uid: act.owner_uid,
            amount,
            config_effect: 0,
            hurt: Some(HurtInfoData {
                from_uid: act.source_or_owner_uid(),
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 0,
                skill_id: 0,
                damage_from: HurtDamageFromType::Buff,
                buff_act_id: act.act_id,
                buff_uid: act.buff_uid,
                hurt_effect_type: 0,
                display_amount: None,
            }),
        })))
    })
}

fn parse_capacity(raw: &str) -> Option<(i32, i32)> {
    let mut parts = raw.split('#');
    Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
}

fn is_raspberry_act_id(catalog: BattleCatalog, act_id: i32) -> bool {
    catalog
        .buff_act_definition(act_id)
        .is_some_and(|definition| definition.kind == BuffActKind::Raspberry)
}

fn is_big_skill_act_id(catalog: BattleCatalog, act_id: i32) -> bool {
    catalog
        .buff_act_definition(act_id)
        .is_some_and(|definition| definition.kind == BuffActKind::RaspberryBigSkill)
}

fn rate(feature: &ActiveBuffFeature, index_after_act_id: usize) -> i32 {
    feature
        .values
        .get(index_after_act_id + 1)
        .copied()
        .unwrap_or_default()
}

#[cfg(test)]
mod test;
