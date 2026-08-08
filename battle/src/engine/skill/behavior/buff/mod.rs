use crate::engine::{
    manager::{
        BattleManagers,
        buff::{
            BuffAmount, BuffChangeDuration, BuffChildUidReservation, BuffCommand, BuffConsume,
            BuffConvert, BuffDispel, BuffGrant, BuffGrantChild, BuffRemove, BuffRemoveSelector,
            BuffReplace, BuffSelector, BuffSetAmount, BuffSetState, BuffStatus, CommandOrigin,
            DepletedBuff,
        },
        card::{CardCommand, CardConsumeForEffect},
        eureka::{EUREKA_RESOURCE_ID, EurekaChange, EurekaCommand},
        shield::ShieldCommand,
    },
    skill::{
        behavior::{
            BehaviorOpContext,
            classify::BehaviorKind,
            registry::{BehaviorHandler, OutputOwner},
        },
        buff_act::{is_kind, registry::BuffActKind},
        effect::ParsedBehavior,
        rule::{
            RuleDomain, RuleReferences,
            output::{BattleCommand, RuleOp},
        },
        target::{TargetRequest, TargetResolver},
    },
};

#[cfg(test)]
use crate::engine::skill::target::TargetPool;

mod application;
mod copy;
mod dispel;
mod distribute;
mod grant;

pub(super) fn supports_random_pool(behavior: &ParsedBehavior) -> bool {
    grant::supports_random_pool(behavior)
}
mod layer;
mod poison;
mod replace;

pub(super) use super::{command_origin, registry};
use application::*;
use copy::copy_status_ops;
pub(super) use copy::supports_status_copy;
use dispel::{
    damage_window_remove_ops, dispel_commands, excluded_dispel_command,
    remove_each_buff_family_ops, sort_buff_by_hp_ops, spread_buff_ops,
};
pub(super) use dispel::{
    supports_dispel, supports_disperse_force, supports_disperse_force3, supports_exact_buff_dispel,
    supports_excluded_dispel, supports_status_dispel, supports_type_family_dispel,
};
use distribute::*;
pub use grant::random_buff_pool;
use grant::{grant_command, random_pool_grant_commands};
use layer::*;
use poison::*;
use replace::*;

pub(super) fn supports_consume_power_add_buff(behavior: &ParsedBehavior) -> bool {
    let exact_shape =
        behavior.raw_args.is_empty() && behavior.args.len() == 2 || behavior.raw_args.len() == 2;
    exact_shape
        && behavior.arg(0).is_some_and(|cost| cost > 0)
        && behavior
            .arg_list(1)
            .is_some_and(|buffs| buffs.iter().all(|buff_id| *buff_id > 0))
}

pub(super) fn supports_consume_card_add_buff(behavior: &ParsedBehavior) -> bool {
    let rewards = if behavior.raw_args.is_empty() {
        behavior.args.get(1..).map(<[i32]>::to_vec)
    } else if behavior.raw_args.len() == 2 {
        behavior.arg_list(1)
    } else {
        None
    };
    let (Some(buff_id), Some(rewards)) = (behavior.arg(0), rewards) else {
        return false;
    };

    rewards.len() == 3
        && (buff_id > 0 && rewards.iter().all(|reward| *reward > 0)
            || buff_id == 0 && rewards.iter().all(|reward| *reward == 0))
}

pub(super) fn supports_consume_power_add_multi_buff(behavior: &ParsedBehavior) -> bool {
    matches!(
        behavior.args.as_slice(),
        [cost, required_allies, required_buff, base_layer, bonus_layer, base_buff, bonus_buff]
            if *cost > 0
                && *required_allies > 0
                && *required_buff > 0
                && *base_layer > 0
                && *bonus_layer > 0
                && *base_buff > 0
                && *bonus_buff > 0
    )
}

pub(super) struct Handler;

pub(super) fn supports_layer_range(behavior: &ParsedBehavior) -> bool {
    let (Some(source_buff_id), Some(buff_ids), Some(thresholds)) =
        (behavior.arg(0), behavior.arg_list(1), behavior.arg_list(2))
    else {
        return false;
    };

    source_buff_id > 0
        && buff_ids.iter().all(|id| *id > 0)
        && thresholds.len() == buff_ids.len() + 1
        && thresholds.iter().all(|value| *value > 0)
        && thresholds.windows(2).all(|pair| pair[0] < pair[1])
}

pub(super) fn supports_distribute(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [source, output] if *source > 0 && *output > 0)
}

pub(super) fn supports_duration_change(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [buff_id_or_type, delta] if *buff_id_or_type > 0 && *delta != 0)
}

pub(super) fn supports_channel_count_reduction(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [buff_id_or_type, amount] if *buff_id_or_type > 0 && *amount > 0)
}

pub(super) fn supports_count_multiplier(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [buff_id, multiplier] if *buff_id > 0 && *multiplier == 2)
}

impl BehaviorHandler for Handler {
    fn emit_ops(
        mut context: BehaviorOpContext<'_>,
        behavior: &ParsedBehavior,
    ) -> Option<Vec<RuleOp>> {
        match behavior.spec.kind {
            BehaviorKind::AddTargetBuffByPoison => {
                add_target_buff_by_poison_ops(&context, behavior)
            }
            BehaviorKind::AddBuffRanId | BehaviorKind::AddBuffRanTypeId => {
                random_pool_grant_commands(&mut context, behavior)
            }
            BehaviorKind::AddBuffByHeroId => hero_grant_command(&context, behavior)
                .map(|command| vec![RuleOp::Command(BattleCommand::Buff(command))]),
            BehaviorKind::DisperseForce2 => damage_window_remove_ops(context.target_uid, behavior),
            BehaviorKind::DisperseForce3 | BehaviorKind::DisperseTypeId => {
                remove_each_buff_family_ops(context.target_uid, behavior)
            }
            BehaviorKind::DisperseExclude => excluded_dispel_command(context.target_uid, behavior)
                .map(|command| vec![RuleOp::Command(BattleCommand::Buff(command))]),
            BehaviorKind::Disperse1
            | BehaviorKind::Disperse2
            | BehaviorKind::Purify1
            | BehaviorKind::PurifyX => {
                dispel_commands(context.target_uid, behavior).map(|commands| {
                    commands
                        .into_iter()
                        .map(|command| RuleOp::Command(BattleCommand::Buff(command)))
                        .collect()
                })
            }
            BehaviorKind::ReplaceBuff2 => replace_buff2_ops(&context, behavior),
            BehaviorKind::ConsumePowerAddBuff => consume_power_add_buff_ops(&context, behavior),
            BehaviorKind::ReplaceBuff => replace_buff_ops(&context, behavior),
            BehaviorKind::ConsumePowerAddMultiBuff1 => {
                consume_power_add_multi_buff_ops(&context, behavior)
            }
            BehaviorKind::AddBuffBasedOnEnemyBurnUseCount => {
                add_buff_from_enemy_burn_ops(&context, behavior)
            }
            BehaviorKind::AddBuffBySkillBuffAdditions => {
                add_buff_from_skill_additions_ops(&context, behavior)
            }
            BehaviorKind::AddBuffByBuffLayer => add_buff_by_layer_ops(&mut context, behavior),
            BehaviorKind::BuffSpread => spread_buff_ops(&context, behavior),
            BehaviorKind::BuffCountMulti => multiply_buff_count_ops(&context, behavior),
            BehaviorKind::BuffSortByHp => sort_buff_by_hp_ops(&context, behavior),
            BehaviorKind::AddBuffByBuffLayerRange => {
                add_buff_by_layer_range_ops(&context, behavior)
            }
            BehaviorKind::ConsumeBuffLayerAndOtherAddBuff => {
                consume_buff_layer_and_team_grant_ops(&mut context, behavior)
            }
            BehaviorKind::DistributeBuff => distribute_buff_ops(&context, behavior),
            BehaviorKind::SelfRandomCopyBuffs => copy_status_ops(&mut context, behavior),
            BehaviorKind::ConsumeCardAddBuff => {
                consume_card_grant_ops(&context, context.target_uid, behavior)
            }
            BehaviorKind::ConsumeBuffByTypeId | BehaviorKind::ConsumeBuffByTypeId2 => {
                consume_buff_command(context.target_uid, behavior)
                    .map(|command| vec![RuleOp::Command(BattleCommand::Buff(command))])
            }
            BehaviorKind::AddBuffDuration => {
                change_duration_command(context.target_uid, behavior, BuffSelector::ExactId)
                    .map(|command| vec![RuleOp::Command(BattleCommand::Buff(command))])
            }
            BehaviorKind::AddBuffRound => {
                change_duration_command(context.target_uid, behavior, BuffSelector::IdOrType)
                    .map(|command| vec![RuleOp::Command(BattleCommand::Buff(command))])
            }
            BehaviorKind::ReduceCastChannelCount => {
                reduce_channel_count_command(context.managers, context.target_uid, behavior)
                    .map(|command| vec![RuleOp::Command(BattleCommand::Buff(command))])
            }
            BehaviorKind::AddBuff | BehaviorKind::AddBuffPowerUse | BehaviorKind::AddBuffRound2 => {
                shield_grant_ops(&context, behavior)
                    .or_else(|| heat_scale_snapshot_grant_ops(&context, behavior))
                    .or_else(|| team_energy_snapshot_grant_ops(&context, behavior))
                    .or_else(|| {
                        grant_command(
                            context.source_uid,
                            context.target_uid,
                            context.transfer_count.max(0) as u32,
                            behavior,
                        )
                        .map(|command| vec![RuleOp::Command(BattleCommand::Buff(command))])
                    })
            }
            BehaviorKind::RemoveBuffToAddBuff => remove_buff_to_add_buff_ops(&context, behavior),
            _ => None,
        }
    }

    fn output_owner(behavior: &ParsedBehavior, index: usize) -> Option<OutputOwner> {
        matches!(
            (behavior.spec.kind, index),
            (BehaviorKind::ConsumePowerAddBuff, 0)
        )
        .then_some(OutputOwner::Parent)
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        references(behavior)
    }
}

fn references(behavior: &ParsedBehavior) -> RuleReferences {
    let buffs = match behavior.spec.kind {
        BehaviorKind::AddBuff
        | BehaviorKind::AddBuffPowerUse
        | BehaviorKind::AddBuffRound
        | BehaviorKind::AddBuffRound2
        | BehaviorKind::ConsumeCardAddBuff => behavior.arg(0).into_iter().collect(),
        BehaviorKind::AddTargetBuffByPoison => behavior.arg(2).into_iter().collect(),
        BehaviorKind::ConsumePowerAddBuff => behavior.arg_list(1).unwrap_or_default(),
        BehaviorKind::ConsumePowerAddMultiBuff1 => [2, 5, 6]
            .into_iter()
            .filter_map(|index| behavior.arg(index))
            .collect(),
        BehaviorKind::RemoveBuffToAddBuff => [0, 1]
            .into_iter()
            .filter_map(|index| behavior.arg(index))
            .collect(),
        // Both select existing buff state by id or type; neither introduces a
        // concrete buff dependency.
        BehaviorKind::AddBuffDuration | BehaviorKind::ReduceCastChannelCount => Vec::new(),
        // These own id-or-type selectors, so their operands are not necessarily
        // concrete buff dependencies (for example type 8112).
        BehaviorKind::DisperseForce2 | BehaviorKind::DisperseForce3 => Vec::new(),
        BehaviorKind::BuffSortByHp | BehaviorKind::BuffSpread => {
            behavior.arg(0).into_iter().collect()
        }
        BehaviorKind::BuffCountMulti => behavior.arg(0).into_iter().collect(),
        BehaviorKind::ReplaceBuff => [0, 2, 3]
            .into_iter()
            .filter_map(|index| behavior.arg(index))
            .collect(),
        BehaviorKind::ReplaceBuff2 => behavior
            .arg_list(0)
            .into_iter()
            .flatten()
            .chain(behavior.arg(1))
            .collect(),
        BehaviorKind::AddBuffBasedOnEnemyBurnUseCount => behavior.arg(0).into_iter().collect(),
        BehaviorKind::AddBuffBySkillBuffAdditions => behavior.arg(0).into_iter().collect(),
        BehaviorKind::AddBuffByBuffLayer => [0, 1]
            .into_iter()
            .filter_map(|index| behavior.arg(index))
            .collect(),
        BehaviorKind::AddBuffByBuffLayerRange => behavior
            .arg(0)
            .into_iter()
            .chain(behavior.arg_list(1).into_iter().flatten())
            .collect(),
        BehaviorKind::ConsumeBuffLayerAndOtherAddBuff => [1, 3]
            .into_iter()
            .filter_map(|index| behavior.arg(index))
            .collect(),
        BehaviorKind::DistributeBuff => behavior.args.clone(),
        BehaviorKind::AddBuffByHeroId => behavior
            .raw_args
            .iter()
            .skip(1)
            .flat_map(|raw| raw.split(','))
            .filter_map(|raw| raw.trim().parse().ok())
            .collect(),
        _ => Vec::new(),
    };
    RuleReferences {
        skills: Vec::new(),
        buffs,
        models: Vec::new(),
    }
}

fn add_buff_from_skill_additions_ops(
    context: &BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let buff_id = behavior.arg(0)?;
    let count = match context.event? {
        crate::engine::event::payload::BattleEvent::SkillAction(action) => action
            .buff_additions
            .iter()
            .find_map(|(added_id, amount)| (*added_id == buff_id).then_some(*amount))
            .unwrap_or_default(),
        _ => 0,
    };
    if count <= 0 {
        return Some(Vec::new());
    }
    grant_command(
        context.source_uid,
        context.target_uid,
        count as u32,
        behavior,
    )
    .map(|command| vec![RuleOp::Command(BattleCommand::Buff(command))])
}

fn pool_buff_ids(raw: &str) -> Vec<i32> {
    raw.split('#')
        .filter_map(|entry| entry.split(',').next()?.trim().parse().ok())
        .filter(|buff_id| *buff_id > 0)
        .collect()
}

fn reduce_channel_count_command(
    managers: &BattleManagers,
    target_uid: i64,
    behavior: &ParsedBehavior,
) -> Option<BuffCommand> {
    let [buff_id_or_type, amount] = behavior.args.as_slice() else {
        return None;
    };
    let buff_uid = managers
        .buff
        .buff_id_or_type_uid(target_uid, *buff_id_or_type)?;
    let current = managers
        .buff
        .snapshot(target_uid, buff_uid)?
        .ex_info
        .unwrap_or_default();
    Some(BuffCommand::SetState(BuffSetState {
        origin: command_origin(behavior)?,
        target_uid,
        buff_uid,
        ex_info: Some(current.saturating_sub(*amount).max(0)),
        params: None,
        act_info: None,
    }))
}

#[cfg(test)]
mod tests;
