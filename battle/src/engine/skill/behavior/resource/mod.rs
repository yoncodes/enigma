use crate::engine::{
    entity::attr::AttrId,
    manager::{
        buff::{BuffCommand, BuffConsume, BuffGrant, BuffSelector, DepletedBuff},
        card::{CardCommand, CardConsumeForEffect},
        conduit::{
            ConduitCommand, ConduitCounterChange, ConduitCounterKind, ConduitPowerChange,
            ConduitPowerChangeKind,
        },
        eureka::{EUREKA_RESOURCE_ID, EurekaChange, EurekaCommand, EurekaProgress},
        ex_point::{ExPointChange, ExPointCommand, ExPointKind},
        gauge::{GaugeCommand, GaugeOperation},
        hp::{CurrentHpSet, HpCommand},
    },
    skill::{
        action::{SkillExecutionMode, SkillInvocation, SkillRequest, SkillTarget},
        behavior::{
            BehaviorOpContext,
            classify::BehaviorKind,
            registry::{BehaviorHandler, OutputOwner},
        },
        buff_act,
        effect::ParsedBehavior,
        rule::{
            RuleReferences,
            output::{BattleCommand, RuleOp},
        },
    },
};

#[cfg(test)]
use crate::engine::manager::BattleManagers;
use sonettobuf::effect_type_enum::EffectType;

pub(super) struct Handler;

pub(super) fn supports_recover_power_and_cast_cards(behavior: &ParsedBehavior) -> bool {
    matches!(
        behavior.args.as_slice(),
        [skill_id, target_rule]
            if *skill_id > 0
                && crate::engine::skill::target::is_mapped_target_code(*target_rule)
    )
}

pub(super) fn supports_conduit_counter(behavior: &ParsedBehavior) -> bool {
    matches!(
        behavior.args.as_slice(),
        [kind, delta] if ConduitCounterKind::from_config(*kind).is_some() && *delta > 0
    )
}

pub(super) fn supports_buff_owned_charge(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [delta] if *delta > 0)
}

pub(super) fn supports_consume_buff_into_charge_and_rewards(behavior: &ParsedBehavior) -> bool {
    ConsumeBuffIntoChargeAndRewards::from_behavior(behavior).is_some()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConsumeBuffIntoChargeAndRewards {
    consumed_buff_id: i32,
    consume_amount: i32,
    charge_delta: i32,
    ex_point_delta: i32,
    rewards: Vec<(i32, i32)>,
}

impl ConsumeBuffIntoChargeAndRewards {
    fn from_behavior(behavior: &ParsedBehavior) -> Option<Self> {
        let [
            consumed_buff_id,
            consume_amount,
            charge_delta,
            ex_point_delta,
            rewards,
        ] = behavior.raw_args.as_slice()
        else {
            return None;
        };
        let consumed_buff_id = consumed_buff_id.parse().ok()?;
        let consume_amount = consume_amount.parse().ok()?;
        let charge_delta = charge_delta.parse().ok()?;
        let ex_point_delta = ex_point_delta.parse().ok()?;
        let rewards = rewards
            .split(':')
            .map(|reward| {
                let mut fields = reward.split(',');
                let buff_id = fields.next()?.parse().ok()?;
                let amount = fields.next()?.parse().ok()?;
                (fields.next().is_none() && buff_id > 0 && amount > 0).then_some((buff_id, amount))
            })
            .collect::<Option<Vec<_>>>()?;

        (consumed_buff_id > 0
            && consume_amount > 0
            && charge_delta > 0
            && matches!(ex_point_delta, 0 | 1)
            && !rewards.is_empty())
        .then_some(Self {
            consumed_buff_id,
            consume_amount,
            charge_delta,
            ex_point_delta,
            rewards,
        })
    }
}

fn buff_owned_charge_ops(
    managers: &crate::engine::manager::BattleManagers,
    target_uid: i64,
    origin: crate::engine::skill::rule::CommandOrigin,
    delta: i32,
) -> Option<Vec<RuleOp>> {
    let feature = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find(|feature| {
            feature.owner_uid == target_uid
                && buff_act::is_kind(feature, buff_act::registry::BuffActKind::BuffOwnedCharge)
        })?;
    let act_id = feature.act_id()?;
    let limit = feature.values.get(2).copied()?;
    let act_info = managers
        .buff
        .snapshot(target_uid, feature.buff_uid)
        .map(|buff| buff.act_info)?;
    let mut matching = act_info.iter().filter(|info| info.act_id == Some(act_id));
    let info = matching.next()?;
    if matching.next().is_some() || info.str_param.as_deref() != Some("") {
        return None;
    }
    let [current] = info.param.as_slice() else {
        return None;
    };
    if !(0..=limit).contains(current) {
        return None;
    }
    if *current == limit {
        return Some(Vec::new());
    }
    Some(vec![RuleOp::Command(BattleCommand::Buff(
        BuffCommand::AccumulateCappedActState(
            crate::engine::manager::buff::BuffAccumulateCappedActState {
                origin,
                target_uid,
                buff_uid: feature.buff_uid,
                act_id,
                delta,
                maximum: limit,
            },
        ),
    ))])
}

pub fn supports_average_life(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [0])
}

impl BehaviorHandler for Handler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        rule_ops(context, behavior)
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        let buffs = match behavior.spec.kind {
            BehaviorKind::PerTypeBuffAddEnergyToTeam => per_type_buff_energy_args(behavior)
                .map(|(buff_id, _)| vec![buff_id])
                .unwrap_or_default(),
            BehaviorKind::PerTypeBuffAddEnergyToEmitter => {
                per_type_buff_emitter_energy_args(behavior)
                    .map(|(buff_id, _)| vec![buff_id])
                    .unwrap_or_default()
            }
            BehaviorKind::ConsumeBuffIntoChargeAndRewards => {
                let Some(parsed) = ConsumeBuffIntoChargeAndRewards::from_behavior(behavior) else {
                    return RuleReferences::default();
                };
                let mut buffs = Vec::with_capacity(parsed.rewards.len() + 1);
                buffs.push(parsed.consumed_buff_id);
                buffs.extend(parsed.rewards.into_iter().map(|(buff_id, _)| buff_id));
                buffs
            }
            _ => Vec::new(),
        };

        RuleReferences {
            skills: matches!(
                behavior.spec.kind,
                BehaviorKind::RecoverPowerAndDelCardsUseSkill
            )
            .then(|| behavior.arg(0))
            .flatten()
            .into_iter()
            .collect(),
            buffs,
            ..Default::default()
        }
    }

    fn output_owner(behavior: &ParsedBehavior, op: &RuleOp, _index: usize) -> Option<OutputOwner> {
        (behavior.spec.kind == BehaviorKind::ConsumeBuffIntoChargeAndRewards
            && matches!(
                op,
                RuleOp::Command(BattleCommand::Buff(
                    BuffCommand::Consume(_) | BuffCommand::Grant(_)
                ))
            ))
        .then_some(OutputOwner::CausingEvent)
    }
}

pub fn rule_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
    let origin = super::command_origin(behavior)?;
    let ex_point_config_effect = match behavior.spec.kind {
        BehaviorKind::AddConduitExPoint => 0,
        _ => behavior.config_effect,
    };
    let ex_point = |target_uid, delta| {
        RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Change(
            ExPointChange {
                origin,
                source_uid: context.source_uid,
                target_uid,
                delta,
                config_effect: ex_point_config_effect,
                effect_type: EffectType::Expointchange as i32,
            },
        )))
    };
    let eureka = |power_id, delta| {
        RuleOp::Command(BattleCommand::Eureka(EurekaCommand::Change(EurekaChange {
            origin,
            source_uid: context.source_uid,
            target_uid: context.target_uid,
            power_id,
            delta,
            effect_type: EffectType::Powerchange as i32,
        })))
    };
    let team_energy = |delta| {
        let Some(team) = context.managers.entity.team_type(context.target_uid) else {
            return Some(Vec::new());
        };
        if delta == 0 {
            return Some(Vec::new());
        }
        let key = crate::engine::mechanic::impromptu::team_energy_key(team);
        let mut ops = Vec::with_capacity(2);
        if context.managers.gauge.get(key).is_none() {
            ops.push(RuleOp::Command(BattleCommand::Gauge(GaugeCommand::new(
                origin,
                key,
                GaugeOperation::Enable { max: None },
            ))));
        }
        ops.push(RuleOp::Command(BattleCommand::Gauge(
            GaugeCommand::new(origin, key, GaugeOperation::ChangeValue { delta })
                .attributed_to(context.source_uid, behavior.config_effect),
        )));
        Some(ops)
    };

    match behavior.spec.kind {
        BehaviorKind::AddExPoint
            if ExPointKind::from_wire(context.managers.ex_point.kind(context.target_uid))
                != ExPointKind::Common =>
        {
            Some(Vec::new())
        }
        BehaviorKind::AddExPoint
        | BehaviorKind::AddAdrenalineExPoint
        | BehaviorKind::AddSynchronization
        | BehaviorKind::AttrFixExPoint
        | BehaviorKind::AddConduitExPoint => behavior.arg(0).map(|delta| {
            vec![ex_point(
                context.target_uid,
                delta.saturating_mul(context.transfer_count),
            )]
        }),
        BehaviorKind::DelExPoint | BehaviorKind::DelExPointNotImmunity => behavior
            .arg(0)
            .map(|amount| vec![ex_point(context.target_uid, -amount.max(0))]),
        BehaviorKind::AbsorbExPoint => {
            let amount = behavior.arg(0)?;
            let removed = amount
                .max(0)
                .min(context.managers.ex_point.get(context.target_uid));
            Some(if removed > 0 {
                vec![
                    ex_point(context.target_uid, -removed),
                    ex_point(context.source_uid, removed),
                ]
            } else {
                Vec::new()
            })
        }
        BehaviorKind::AverageLife => {
            let [0] = behavior.args.as_slice() else {
                return None;
            };
            let allies = context.pool.allies(context.source_uid);
            let total_max = allies
                .iter()
                .map(|ally| context.managers.hp.max(ally.uid) as i64)
                .sum::<i64>();
            let total_current = allies
                .iter()
                .map(|ally| context.managers.hp.current(ally.uid) as i64)
                .sum::<i64>();
            if total_max <= 0 {
                return Some(Vec::new());
            }
            Some(
                allies
                    .iter()
                    .map(|ally| {
                        let value = (context.managers.hp.max(ally.uid) as i64 * total_current
                            / total_max) as i32;
                        RuleOp::Command(BattleCommand::Hp(HpCommand::SetCurrent(CurrentHpSet {
                            origin,
                            source_uid: context.source_uid,
                            target_uid: ally.uid,
                            value,
                            config_effect: behavior.config_effect,
                            effect_type: EffectType::Averagelife as i32,
                        })))
                    })
                    .collect(),
            )
        }
        BehaviorKind::ChangePower | BehaviorKind::RecoverPower => {
            power_args(&behavior.args).map(|(power_id, delta)| vec![eureka(power_id, delta)])
        }
        BehaviorKind::RecoverPowerAndDelCardsUseSkill => {
            let [skill_id, target_rule] = behavior.args.as_slice() else {
                return None;
            };
            let state = context
                .managers
                .eureka
                .get(context.target_uid, EUREKA_RESOURCE_ID);
            let delta = state.max - state.current;
            let cards = context
                .managers
                .card
                .plan_effect_consumption(context.target_uid);
            let mut ops = Vec::with_capacity(cards.len() + 2);
            if delta != 0 {
                ops.push(eureka(EUREKA_RESOURCE_ID, delta));
            }
            if !cards.is_empty() {
                ops.push(RuleOp::Command(BattleCommand::Card(
                    CardCommand::ConsumeForEffect(CardConsumeForEffect {
                        origin,
                        owner_uid: context.target_uid,
                        indices: cards.iter().map(|(index, _)| *index).collect(),
                    }),
                )));
            }
            ops.extend(cards.into_iter().map(|_| {
                let mut invocation: SkillInvocation = SkillRequest {
                    source_uid: context.target_uid,
                    skill_id: *skill_id,
                }
                .into();
                invocation.target = SkillTarget::LogicRule(*target_rule);
                invocation.mode = SkillExecutionMode::Active;
                RuleOp::Skill(invocation)
            }));
            Some(ops)
        }
        BehaviorKind::AddPowerByCritCount => {
            let [threshold, gain] = behavior.args.as_slice() else {
                return None;
            };
            Some(vec![RuleOp::Command(BattleCommand::Eureka(
                EurekaCommand::ChangeByProgress {
                    change: EurekaChange {
                        origin,
                        source_uid: context.source_uid,
                        target_uid: context.target_uid,
                        power_id: EUREKA_RESOURCE_ID,
                        delta: *gain,
                        effect_type: EffectType::Powerchange as i32,
                    },
                    progress: EurekaProgress {
                        owner_uid: context.target_uid,
                        key: origin.key,
                        threshold: *threshold,
                        amount: context
                            .target
                            .critical_action_count
                            .max(i32::from(context.target.action_crit_count > 0)),
                    },
                },
            ))])
        }
        BehaviorKind::TotalSkillRankToPower => {
            let [rate, power_id] = behavior.args.as_slice() else {
                return None;
            };
            let delta = (i64::from(context.managers.card.total_resolving_rank()) * i64::from(*rate)
                / 1000)
                .clamp(0, i64::from(i32::MAX)) as i32;
            Some(
                (delta > 0)
                    .then(|| eureka(*power_id, delta))
                    .into_iter()
                    .collect(),
            )
        }
        BehaviorKind::AddEmitterEnergy => {
            let delta = behavior.arg(0)?;
            let key = crate::engine::mechanic::impromptu::inspiration_key(
                crate::engine::manager::emitter::UID,
            );
            if delta == 0 || context.managers.gauge.get(key).is_none() {
                return Some(Vec::new());
            }
            Some(vec![RuleOp::Command(BattleCommand::Gauge(
                GaugeCommand::new(origin, key, GaugeOperation::ChangeValue { delta })
                    .attributed_to(context.source_uid, behavior.config_effect),
            ))])
        }
        BehaviorKind::AddTeamEnergy => behavior.arg(0).and_then(team_energy),
        BehaviorKind::PerTypeBuffAddEnergyToTeam => {
            let (buff_id, per_layer) = per_type_buff_energy_args(behavior)?;
            let layers = context
                .managers
                .buff
                .buff_id_amount(context.target_uid, buff_id);
            team_energy(layers.saturating_mul(per_layer))
        }
        BehaviorKind::PerTypeBuffAddEnergyToEmitter => {
            let (buff_id, multiplier) = per_type_buff_emitter_energy_args(behavior)?;
            let layers = context
                .managers
                .buff
                .buff_id_amount(context.source_uid, buff_id);
            let delta = layers.saturating_mul(multiplier);
            let key = crate::engine::mechanic::impromptu::inspiration_key(
                crate::engine::manager::emitter::UID,
            );
            if delta == 0 || context.managers.gauge.get(key).is_none() {
                return Some(Vec::new());
            }
            Some(vec![RuleOp::Command(BattleCommand::Gauge(
                GaugeCommand::new(origin, key, GaugeOperation::ChangeValue { delta })
                    .attributed_to(context.source_uid, behavior.config_effect),
            ))])
        }
        BehaviorKind::AddRedOrBlueCount => {
            let [color, count] = behavior.args.as_slice() else {
                return None;
            };
            let (buff_uid, act_id, _) = context.managers.buff.buff_act_carrier(
                context.target_uid,
                buff_act::registry::BuffActKind::RedOrBlueCount,
            )?;
            let current = context
                .managers
                .buff
                .snapshot(context.target_uid, buff_uid)?
                .act_common_params;
            let params =
                buff_act::red_or_blue_count::append(current.as_deref(), act_id, *color, *count)?;
            Some(vec![RuleOp::Command(BattleCommand::Buff(
                crate::engine::manager::buff::BuffCommand::SetStateSnapshot(
                    crate::engine::manager::buff::BuffSetState {
                        origin,
                        target_uid: context.target_uid,
                        buff_uid,
                        params: Some(params),
                        act_info: None,
                        ex_info: None,
                    },
                ),
            ))])
        }
        BehaviorKind::AddBuffOwnedCharge => {
            let [delta] = behavior.args.as_slice() else {
                return None;
            };
            buff_owned_charge_ops(context.managers, context.target_uid, origin, *delta)
        }
        BehaviorKind::ConsumeBuffIntoChargeAndRewards => {
            let parsed = ConsumeBuffIntoChargeAndRewards::from_behavior(behavior)?;
            if context
                .managers
                .buff
                .buff_id_amount(context.target_uid, parsed.consumed_buff_id)
                < parsed.consume_amount
            {
                return Some(Vec::new());
            }

            let mut ops = Vec::with_capacity(parsed.rewards.len() + 4);
            ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(
                BuffConsume {
                    origin,
                    target_uid: context.target_uid,
                    selector: BuffSelector::ExactId(parsed.consumed_buff_id),
                    amount: parsed.consume_amount,
                    depleted: DepletedBuff::Remove,
                },
            ))));
            ops.extend(buff_owned_charge_ops(
                context.managers,
                context.target_uid,
                origin,
                parsed.charge_delta,
            )?);
            if parsed.ex_point_delta > 0 {
                ops.push(ex_point(context.target_uid, parsed.ex_point_delta));
            }
            ops.extend(parsed.rewards.into_iter().map(|(buff_id, amount)| {
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                    origin,
                    source_uid: context.source_uid,
                    target_uid: context.target_uid,
                    buff_id,
                    amount: crate::engine::manager::buff::BuffManager::configured_accepts_explicit_grant_amount(buff_id)
                        .then_some(amount),
                    occurrences: 1,
                    child_uid_reservations: 0,
                })))
            }));
            Some(ops)
        }
        BehaviorKind::AddConduitPower => {
            let (power_id, delta, kind) = conduit_power_args(&behavior.args)?;
            Some(vec![RuleOp::Command(BattleCommand::Conduit(
                ConduitCommand::ChangePower(ConduitPowerChange {
                    origin,
                    source_uid: context.source_uid,
                    team: context.source_team,
                    power_id,
                    delta,
                    kind,
                }),
            ))])
        }
        BehaviorKind::AddConduitCounter => {
            let [kind, delta] = behavior.args.as_slice() else {
                return None;
            };
            Some(vec![RuleOp::Command(BattleCommand::Conduit(
                ConduitCommand::ChangeCounter(ConduitCounterChange {
                    origin,
                    source_uid: context.source_uid,
                    team: context.source_team,
                    kind: ConduitCounterKind::from_config(*kind)?,
                    delta: *delta,
                }),
            ))])
        }
        BehaviorKind::SetConduitSkillGroup => {
            let group = behavior.arg(0)?;
            Some(vec![RuleOp::Command(BattleCommand::Conduit(
                ConduitCommand::SetSkillGroup {
                    origin,
                    source_uid: context.target_uid,
                    group,
                },
            ))])
        }
        BehaviorKind::StopConduitSkill => (context.active_skill_id > 0).then(|| {
            vec![RuleOp::Command(BattleCommand::Conduit(
                ConduitCommand::StopSkill {
                    origin,
                    source_uid: context.source_uid,
                    team: context.source_team,
                    skill_id: context.active_skill_id,
                },
            ))]
        }),
        BehaviorKind::RaspberryAddCount => {
            let [attr_id, rate, _mode] = behavior.args.as_slice() else {
                return None;
            };
            let attr_id = AttrId::from_raw(*attr_id)?;
            Some(buff_act::raspberry::add_count_rule_ops(
                context.managers,
                origin,
                context.source_uid,
                context.target_uid,
                attr_id,
                *rate,
            )?)
        }
        BehaviorKind::RaspberryBigSkill => {
            let [transfer_rate, buff_id] = behavior.args.as_slice() else {
                return None;
            };
            buff_act::raspberry::big_skill_rule_ops(
                context.managers,
                origin,
                context.source_uid,
                context.target_uid,
                *transfer_rate,
                *buff_id,
            )
        }
        _ => None,
    }
}

fn power_args(args: &[i32]) -> Option<(i32, i32)> {
    match args {
        [amount] => Some((EUREKA_RESOURCE_ID, *amount)),
        [power_id, amount] => Some((*power_id, *amount)),
        _ => None,
    }
}

fn conduit_power_args(args: &[i32]) -> Option<(i32, i32, ConduitPowerChangeKind)> {
    match args {
        [power_id, delta] if *power_id >= 0 => {
            Some((*power_id, *delta, ConduitPowerChangeKind::Standard))
        }
        [power_id, delta, 1] if *power_id >= 0 => {
            Some((*power_id, *delta, ConduitPowerChangeKind::Interval))
        }
        _ => None,
    }
}

pub(super) fn supports_conduit_power(behavior: &ParsedBehavior) -> bool {
    conduit_power_args(&behavior.args).is_some()
}

pub(super) fn supports_ex_point_gain(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [delta] if *delta > 0)
}

pub(super) fn supports_conduit_skill_group(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [group] if *group > 0)
}

pub(super) fn supports_power_change(behavior: &ParsedBehavior) -> bool {
    power_args(&behavior.args).is_some_and(|(power_id, _)| power_id > 0)
}

pub(super) fn supports_recover_power(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [amount] if *amount != 0)
}

pub(super) fn supports_team_energy(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [delta] if *delta > 0)
}

pub(super) fn supports_per_type_buff_energy(behavior: &ParsedBehavior) -> bool {
    per_type_buff_energy_args(behavior).is_some()
}

pub(super) fn supports_per_type_buff_emitter_energy(behavior: &ParsedBehavior) -> bool {
    per_type_buff_emitter_energy_args(behavior).is_some()
}

fn per_type_buff_energy_args(behavior: &ParsedBehavior) -> Option<(i32, i32)> {
    let [buff_id, per_layer, mode] = behavior.raw_args.as_slice() else {
        return None;
    };
    let buff_id = buff_id.parse().ok()?;
    let per_layer = per_layer.parse().ok()?;
    let mode: i32 = mode.parse().ok()?;
    (buff_id > 0 && per_layer == 1 && mode == 1).then_some((buff_id, per_layer))
}

fn per_type_buff_emitter_energy_args(behavior: &ParsedBehavior) -> Option<(i32, i32)> {
    let [buff_id, multiplier] = behavior.raw_args.as_slice() else {
        return None;
    };
    let buff_id = buff_id.parse().ok()?;
    let multiplier = multiplier.parse().ok()?;
    (buff_id > 0 && multiplier > 0).then_some((buff_id, multiplier))
}

pub(super) fn supports_red_or_blue_count(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [color @ 1..=3, count] if *color > 0 && *count > 0)
}

pub(super) fn supports_total_skill_rank_power(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [rate, power_id] if *rate > 0 && *power_id > 0)
}

pub(super) fn supports_power_by_critical_count(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [threshold, gain] if *threshold > 0 && *gain > 0)
}

pub(super) fn supports_emitter_energy(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [delta] if *delta > 0)
}

pub(super) fn supports_ex_point_loss(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [amount] if *amount > 0)
}

pub(super) fn supports_raspberry_add_count(behavior: &ParsedBehavior) -> bool {
    matches!(
        behavior.args.as_slice(),
        [raw_attr, rate, 1] if AttrId::from_raw(*raw_attr).is_some() && *rate > 0
    )
}

pub(super) fn supports_raspberry_big_skill(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [transfer_rate, buff_id]
        if *transfer_rate > 0 && *buff_id > 0)
}

#[cfg(test)]
mod tests;
