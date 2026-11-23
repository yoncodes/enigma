use crate::engine::{
    manager::buff::{
        BuffAmount, BuffChildUidReservation, BuffCommand, BuffGrant, BuffGrantChild, BuffRemove,
        BuffRemoveSelector, BuffSetAmount,
    },
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

pub(super) fn supports_conversion(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [limit, output_buff_id] if *limit > 0 && *output_buff_id > 0)
}

impl BehaviorHandler for Handler {
    fn emit_ops(
        mut context: BehaviorOpContext<'_>,
        behavior: &ParsedBehavior,
    ) -> Option<Vec<RuleOp>> {
        match behavior.spec.kind {
            BehaviorKind::CatapultBuff => catapult_ops(&mut context, behavior),
            BehaviorKind::PoisonConvertToTargetBuff
            | BehaviorKind::PoisonConvertToPowerfulPoisonBuff => {
                convert_poison_ops(&context, behavior)
            }
            BehaviorKind::ConsumePoisonSettleDeadlyPoison => consume_poison_ops(&context, behavior),
            _ => None,
        }
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        RuleReferences {
            skills: Vec::new(),
            buffs: match behavior.spec.kind {
                BehaviorKind::CatapultBuff => behavior.arg(3),
                BehaviorKind::PoisonConvertToTargetBuff
                | BehaviorKind::PoisonConvertToPowerfulPoisonBuff => behavior.arg(1),
                _ => None,
            }
            .into_iter()
            .collect(),
            models: Vec::new(),
            summons: Vec::new(),
        }
    }
}

fn convert_poison_ops(
    context: &BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let [limit, output_buff_id] = behavior.args.as_slice() else {
        return None;
    };
    if *limit <= 0 || *output_buff_id <= 0 {
        return None;
    }
    let mut poison = context
        .managers
        .buff
        .active_features(&context.managers.hp)
        .into_iter()
        .filter(|feature| {
            feature.owner_uid == context.target_uid
                && crate::engine::skill::buff_act::is_kind(
                    feature,
                    crate::engine::skill::buff_act::registry::BuffActKind::Poison,
                )
        })
        .collect::<Vec<_>>();
    poison.sort_by_key(|feature| feature.buff_uid);
    poison.dedup_by_key(|feature| feature.buff_uid);

    let origin = super::command_origin(behavior)?;
    let mut remaining = *limit;
    let mut ops = Vec::new();
    for feature in poison {
        let consumed = remaining.min(feature.amount.max(1));
        ops.push(consume_poison_op(
            origin,
            context.target_uid,
            feature.buff_uid,
            feature.amount,
            consumed,
        ));
        remaining -= consumed;
        if remaining == 0 {
            break;
        }
    }
    let converted = *limit - remaining;
    if converted > 0 {
        ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
            BuffGrant {
                origin,
                source_uid: context.source_uid,
                target_uid: context.target_uid,
                buff_id: *output_buff_id,
                amount: Some(converted),
                occurrences: 1,
                child_uid_reservations: 0,
            },
        ))));
    }
    Some(ops)
}

fn consume_poison_ops(
    context: &BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let [limit] = behavior.args.as_slice() else {
        return None;
    };
    if *limit <= 0 {
        return None;
    }
    let features = context.managers.buff.active_features(&context.managers.hp);
    let deadly = features.iter().find(|feature| {
        feature.owner_uid == context.target_uid
            && crate::engine::skill::buff_act::is_kind(
                feature,
                crate::engine::skill::buff_act::registry::BuffActKind::DeadlyPoison,
            )
    })?;
    let mut poison = features
        .iter()
        .filter(|feature| {
            feature.owner_uid == context.target_uid
                && crate::engine::skill::buff_act::is_kind(
                    feature,
                    crate::engine::skill::buff_act::registry::BuffActKind::Poison,
                )
        })
        .collect::<Vec<_>>();
    poison.sort_by_key(|feature| feature.buff_uid);

    let origin = super::command_origin(behavior)?;
    let mut remaining = *limit;
    let mut ops = Vec::new();
    for feature in poison {
        let consumed = remaining.min(feature.amount.max(1));
        ops.push(consume_poison_op(
            origin,
            context.target_uid,
            feature.buff_uid,
            feature.amount,
            consumed,
        ));
        remaining -= consumed;
        if remaining == 0 {
            break;
        }
    }
    let consumed = *limit - remaining;
    if consumed == 0 {
        return Some(Vec::new());
    }
    ops.extend(
        crate::engine::skill::buff_act::deadly_poison::settlement_ops(
            context.managers,
            deadly,
            origin,
            consumed,
        )?,
    );
    Some(ops)
}

fn consume_poison_op(
    origin: crate::engine::manager::buff::CommandOrigin,
    target_uid: i64,
    buff_uid: i64,
    available: i32,
    consumed: i32,
) -> RuleOp {
    if consumed >= available.max(1) {
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
            origin,
            target_uid,
            selector: BuffRemoveSelector::Uid(buff_uid),
        })))
    } else {
        RuleOp::Command(BattleCommand::Buff(BuffCommand::SetAmount(BuffSetAmount {
            origin,
            target_uid,
            buff_uid,
            amount: BuffAmount::Layer(available - consumed),
        })))
    }
}

fn catapult_ops(
    context: &mut BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let [
        reserved_children,
        duration,
        initial_instances,
        buff_id,
        hop_instances,
        hop_count,
    ] = behavior.args.as_slice()
    else {
        return None;
    };
    if *reserved_children <= 0
        || *duration <= 0
        || *initial_instances <= 0
        || *buff_id <= 0
        || *hop_instances <= 0
        || *hop_count < 0
    {
        return None;
    }
    let origin = super::command_origin(behavior)?;
    let mut current = context.target_uid;
    let mut deliveries = vec![(current, *initial_instances)];
    for _ in 0..*hop_count {
        let candidates = context
            .pool
            .enemies(context.source_uid, false)
            .iter()
            .filter(|entity| entity.uid != current && context.managers.hp.current(entity.uid) > 0)
            .map(|entity| entity.uid)
            .collect::<Vec<_>>();
        let Some(index) = context.determinism.lua_random_index(candidates.len()) else {
            break;
        };
        current = candidates[index];
        deliveries.push((current, *hop_instances));
    }

    let mut ops = Vec::new();
    for (target_uid, instances) in deliveries {
        ops.push(RuleOp::Command(BattleCommand::Buff(
            BuffCommand::ReserveChildUids(BuffChildUidReservation {
                origin,
                target_uid,
                count: *reserved_children,
            }),
        )));
        ops.extend((0..instances).map(|_| {
            RuleOp::Command(BattleCommand::Buff(BuffCommand::GrantChild(
                BuffGrantChild {
                    origin,
                    source_uid: context.source_uid,
                    target_uid,
                    buff_id: *buff_id,
                    amount: None,
                    params: None,
                    act_info: None,
                },
            )))
        }));
    }
    Some(ops)
}

#[cfg(test)]
mod tests;
