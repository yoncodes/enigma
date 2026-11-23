use crate::engine::{
    manager::buff::{BuffCommand, BuffConsume, BuffSelector, DepletedBuff},
    skill::{
        action::SkillRateModifier,
        behavior::{BehaviorOpContext, classify::BehaviorKind, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
    },
};

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        if behavior.spec.kind == BehaviorKind::MustCrit {
            context.modifiers.force_critical = true;
            return Some(Vec::new());
        }
        if behavior.spec.kind == BehaviorKind::IgnoreBeatBack {
            context.modifiers.ignore_riposte = true;
            return Some(Vec::new());
        }
        if behavior.spec.kind == BehaviorKind::ConsumeBuffFixMixedRate {
            return mixed_rate_ops(context, behavior);
        }
        if behavior.spec.kind == BehaviorKind::ConsumeBuffAttrFix {
            let [type_or_buff_id, amount, raw_attr_id, delta] = behavior.args.as_slice() else {
                return None;
            };
            let mut ops = Vec::new();
            if *amount > 0 {
                ops.push(RuleOp::Command(BattleCommand::Buff(
                    BuffCommand::ConsumeCoalesced(BuffConsume {
                        origin: super::command_origin(behavior)?,
                        target_uid: context.target_uid,
                        selector: BuffSelector::IdOrType(*type_or_buff_id),
                        amount: *amount,
                        depleted: DepletedBuff::Remove,
                    }),
                )));
            }
            if *delta != 0 {
                context.modifiers.attack_attributes.push((
                    crate::engine::entity::attr::AttrId::from_raw(*raw_attr_id)?,
                    *delta,
                ));
            }
            return Some(ops);
        }
        let [type_or_buff_id, amount, value] = behavior.args.as_slice() else {
            return None;
        };
        if !matches!(
            behavior.spec.kind,
            BehaviorKind::ConsumeBuffUpSkillDamageRate | BehaviorKind::ConsumeBuffChangeTargets
        ) {
            return None;
        }
        let consume = if *amount > 0 {
            Some(BuffConsume {
                origin: super::command_origin(behavior)?,
                target_uid: context.target_uid,
                selector: BuffSelector::IdOrType(*type_or_buff_id),
                amount: *amount,
                depleted: match behavior.spec.kind {
                    BehaviorKind::ConsumeBuffChangeTargets => DepletedBuff::RemoveWithZeroLayer,
                    _ => DepletedBuff::Remove,
                },
            })
        } else {
            None
        };
        if behavior.spec.kind == BehaviorKind::ConsumeBuffChangeTargets && *value > 0 {
            return Some(vec![RuleOp::ExpandActiveSkillTargets {
                additional_count: *value,
                consume,
            }]);
        }
        let mut ops = Vec::new();
        if let Some(consume) = consume {
            ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(
                consume,
            ))));
        }
        if behavior.spec.kind == BehaviorKind::ConsumeBuffUpSkillDamageRate && *value != 0 {
            context.modifiers.rates.push(SkillRateModifier::fixed(
                0,
                behavior.spec.key.opcode,
                *value,
                true,
            ));
        }
        Some(ops)
    }
}

fn mixed_rate_ops(
    context: BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let [
        buff_id,
        amount,
        raw_attr_id,
        attr_delta,
        rate,
        conditional_rate,
        conditional_buff_id,
    ] = behavior.args.as_slice()
    else {
        return None;
    };
    let attr = crate::engine::entity::attr::AttrId::from_raw(*raw_attr_id)?;
    if *attr_delta != 0 {
        context
            .modifiers
            .attack_attributes
            .push((attr, *attr_delta));
    }
    if context
        .managers
        .buff
        .buff_id_amount(context.target_uid, *buff_id)
        < *amount
    {
        return Some(Vec::new());
    }
    if *attr_delta != 0 {
        context
            .modifiers
            .attack_attributes
            .push((attr, *attr_delta));
    }
    if *rate != 0 {
        context.modifiers.rates.push(SkillRateModifier::fixed(
            0,
            behavior.spec.key.opcode,
            *rate,
            true,
        ));
    }
    if *conditional_rate != 0 {
        context.modifiers.rates.extend(
            context
                .pool
                .enemies(context.source_uid, false)
                .iter()
                .filter(|target| {
                    context
                        .managers
                        .buff
                        .buff_id_or_type_amount(target.uid, *conditional_buff_id)
                        > 0
                })
                .map(|target| {
                    SkillRateModifier::fixed(
                        target.uid,
                        behavior.spec.key.opcode,
                        *conditional_rate,
                        true,
                    )
                }),
        );
    }
    Some(vec![RuleOp::Command(BattleCommand::Buff(
        BuffCommand::ConsumeCoalesced(BuffConsume {
            origin: super::command_origin(behavior)?,
            target_uid: context.target_uid,
            selector: BuffSelector::ExactId(*buff_id),
            amount: *amount,
            depleted: DepletedBuff::Remove,
        }),
    ))])
}

pub(super) fn supports_mixed_rate(behavior: &ParsedBehavior) -> bool {
    matches!(
        behavior.args.as_slice(),
        [buff_id, amount, raw_attr_id, attr_delta, rate, conditional_rate, conditional_buff_id]
            if *buff_id > 0
                && *amount > 0
                && crate::engine::entity::attr::AttrId::from_raw(*raw_attr_id).is_some()
                && *attr_delta >= 0
                && *rate >= 0
                && *conditional_rate >= 0
                && *conditional_buff_id > 0
    )
}

pub(super) fn supports_consume(behavior: &ParsedBehavior) -> bool {
    let argument_count = if behavior.raw_args.is_empty() {
        behavior.args.len()
    } else {
        behavior.raw_args.len()
    };
    argument_count == 3
        && matches!(
            behavior.args.as_slice(),
            [buff_id, amount, rate] if *buff_id > 0 && *amount >= 0 && *rate > 0
        )
}

#[cfg(test)]
mod tests;
