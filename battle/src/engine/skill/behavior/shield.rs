use crate::engine::{
    entity::attr::AttrId,
    manager::shield::{ShieldCarrierUid, ShieldCommand, ShieldScope},
    skill::{
        behavior::{BehaviorOpContext, classify::BehaviorKind, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::{
            RuleReferences,
            output::{BattleCommand, RuleOp},
        },
    },
};

pub(super) struct Handler;
pub(super) struct ChildUidHandler;
pub(super) struct ScaledHandler;

type ShieldFormula = (
    i32,
    AttrId,
    i32,
    AttrId,
    i32,
    Option<(AttrId, i32)>,
    Option<(i32, i32)>,
);

fn formula(behavior: &ParsedBehavior, stack_bonus: bool) -> Option<ShieldFormula> {
    match behavior.args.as_slice() {
        [buff_id, raw_attr, rate, _, raw_max_attr, max_rate] => Some((
            *buff_id,
            AttrId::from_raw(*raw_attr)?,
            *rate,
            AttrId::from_raw(*raw_max_attr)?,
            *max_rate,
            None,
            None,
        )),
        [
            buff_id,
            raw_attr,
            rate,
            _,
            raw_max_attr,
            max_rate,
            raw_bonus_attr,
            bonus_rate,
        ] => Some((
            *buff_id,
            AttrId::from_raw(*raw_attr)?,
            *rate,
            AttrId::from_raw(*raw_max_attr)?,
            *max_rate,
            Some((AttrId::from_raw(*raw_bonus_attr)?, *bonus_rate)),
            None,
        )),
        [
            buff_id,
            raw_attr,
            rate,
            0,
            raw_max_attr,
            max_rate,
            raw_bonus_attr,
            bonus_rate,
            stack_buff_id,
            stack_rate,
            -1,
        ] if stack_bonus && *stack_buff_id > 0 && *stack_rate > 0 => Some((
            *buff_id,
            AttrId::from_raw(*raw_attr)?,
            *rate,
            AttrId::from_raw(*raw_max_attr)?,
            *max_rate,
            Some((AttrId::from_raw(*raw_bonus_attr)?, *bonus_rate)),
            Some((*stack_buff_id, *stack_rate)),
        )),
        _ => None,
    }
}

fn supports(behavior: &ParsedBehavior) -> bool {
    formula(behavior, false).is_some()
}

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        supports(behavior)
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        emit_ops(context, behavior, ShieldCarrierUid::Definition, false)
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        references(behavior, false)
    }
}

impl BehaviorHandler for ChildUidHandler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        formula(behavior, true).is_some()
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        emit_ops(context, behavior, ShieldCarrierUid::Child, true)
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        references(behavior, true)
    }
}

impl BehaviorHandler for ScaledHandler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        supports(behavior)
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        emit_ops(context, behavior, ShieldCarrierUid::SharedChild, false)
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        references(behavior, false)
    }
}

fn emit_ops(
    context: BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
    carrier_uid: ShieldCarrierUid,
    stack_bonus: bool,
) -> Option<Vec<RuleOp>> {
    let (buff_id, amount_attr, rate, max_attr, max_rate, multiplier_bonus, stack_bonus) =
        formula(behavior, stack_bonus)?;
    let rate = stack_bonus.map_or(rate, |(buff_id, per_stack)| {
        rate.saturating_add(
            context
                .managers
                .buff
                .buff_id_amount(context.source_uid, buff_id)
                .saturating_mul(per_stack),
        )
    });
    Some(vec![RuleOp::Command(BattleCommand::Shield(
        ShieldCommand {
            origin: super::command_origin(behavior)?,
            source_uid: context.source_uid,
            target_uid: context.target_uid,
            buff_id,
            amount_attr,
            amount_rate: rate,
            multiplier_bonus,
            max_attr,
            max_rate,
            scope: match behavior.spec.kind {
                BehaviorKind::SupplyShield2 => ShieldScope::Entity,
                BehaviorKind::SupplyTeamShareShield => ShieldScope::TeamShared,
                _ => return None,
            },
            carrier_uid,
        },
    ))])
}

fn references(behavior: &ParsedBehavior, stack_bonus: bool) -> RuleReferences {
    let mut buffs = behavior.arg(0).into_iter().collect::<Vec<_>>();
    if let Some((_, _, _, _, _, _, Some((buff_id, _)))) = formula(behavior, stack_bonus) {
        buffs.push(buff_id);
    }
    RuleReferences {
        buffs,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests;
