use crate::engine::{
    entity::attr::AttrId,
    manager::{
        card::{CardCommand, CardConsumeForEffect},
        conduit::{ConduitCommand, ConduitPowerChange, ConduitPowerChangeKind},
        eureka::{EUREKA_RESOURCE_ID, EurekaChange, EurekaCommand, EurekaProgress},
        ex_point::{ExPointChange, ExPointCommand},
        gauge::{GaugeCommand, GaugeOperation},
        hp::{CurrentHpSet, HpCommand},
    },
    skill::{
        action::{SkillExecutionMode, SkillInvocation, SkillRequest, SkillTarget},
        behavior::{BehaviorOpContext, classify::BehaviorKind, registry::BehaviorHandler},
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

impl BehaviorHandler for Handler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        rule_ops(context, behavior)
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        RuleReferences {
            skills: matches!(
                behavior.spec.kind,
                BehaviorKind::RecoverPowerAndDelCardsUseSkill
            )
            .then(|| behavior.arg(0))
            .flatten()
            .into_iter()
            .collect(),
            ..Default::default()
        }
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

    match behavior.spec.kind {
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
        BehaviorKind::AddTeamEnergy => {
            let delta = behavior.arg(0)?;
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
