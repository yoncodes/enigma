use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        buff::{BuffCommand, BuffRemove, BuffRemoveSelector},
        contract::ContractCommand,
    },
    skill::{
        behavior::{BehaviorOpContext, registry::BehaviorHandler},
        buff_act::contract_cast_channel,
        effect::ParsedBehavior,
        rule::{
            RuleReferences,
            output::{BattleCommand, RuleOp},
        },
    },
};

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        behavior.args.is_empty()
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let origin = super::command_origin(behavior)?;
        let candidates = context
            .pool
            .main_allies(context.source_uid)
            .iter()
            .filter(|entity| entity.uid != context.source_uid)
            .map(|entity| entity.uid)
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return Some(Vec::new());
        }
        Some(vec![RuleOp::Command(BattleCommand::Contract(
            ContractCommand::Offer {
                origin,
                owner_uid: context.source_uid,
                candidates,
            },
        ))])
    }
}

pub(super) struct EndHandler;

impl BehaviorHandler for EndHandler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        related_buff_ids(behavior).is_some()
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        RuleReferences {
            buffs: related_buff_ids(behavior)
                .map(|(owner, bound)| owner.into_iter().chain(bound).collect())
                .unwrap_or_default(),
            ..Default::default()
        }
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let BattleEvent::EntityDied(death) = context.event? else {
            return Some(Vec::new());
        };
        let Some(bound_uid) = context.managers.contract.bound_uid(context.source_uid) else {
            return Some(Vec::new());
        };
        if death.target_uid != bound_uid {
            return Some(Vec::new());
        }
        let origin = super::command_origin(behavior)?;
        let (owner_buffs, bound_buffs) = related_buff_ids(behavior)?;
        let mut ops = owner_buffs
            .into_iter()
            .map(|buff_id| remove_buff(origin, context.source_uid, buff_id))
            .chain(
                bound_buffs
                    .into_iter()
                    .map(|buff_id| remove_buff(origin, bound_uid, buff_id)),
            )
            .collect::<Vec<_>>();
        ops.extend(contract_cast_channel::carrier_removal_rule_ops(
            context.managers,
            context.source_uid,
            origin,
        ));
        ops.push(RuleOp::Command(BattleCommand::Contract(
            ContractCommand::Clear {
                owner_uid: context.source_uid,
                bound_uid,
            },
        )));
        Some(ops)
    }
}

fn related_buff_ids(behavior: &ParsedBehavior) -> Option<(Vec<i32>, Vec<i32>)> {
    let argument_count = if behavior.raw_args.is_empty() {
        behavior.args.len()
    } else {
        behavior.raw_args.len()
    };
    if argument_count != 2 {
        return None;
    }
    let owner = behavior.arg_list(0)?;
    let bound = behavior.arg_list(1)?;
    (!owner.is_empty()
        && !bound.is_empty()
        && owner.iter().chain(&bound).all(|buff_id| *buff_id > 0))
    .then_some((owner, bound))
}

fn remove_buff(
    origin: crate::engine::skill::rule::CommandOrigin,
    target_uid: i64,
    buff_id: i32,
) -> RuleOp {
    RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
        origin,
        target_uid,
        selector: BuffRemoveSelector::ExactId(buff_id),
    })))
}

#[cfg(test)]
mod tests;
