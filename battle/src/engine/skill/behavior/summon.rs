use crate::engine::{
    manager::{
        buff::{BuffChildUidReservation, BuffCommand},
        entity::{EntityCommand, EntityOperation},
        summon::{SummonCommand, SummonOperation, summoned_lane},
    },
    skill::{
        behavior::{BehaviorOpContext, classify::BehaviorKind, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
    },
};

pub fn rule_op(source_uid: i64, target_uid: i64, behavior: &ParsedBehavior) -> Option<RuleOp> {
    let operation = match behavior.spec.kind {
        BehaviorKind::AddSummoned => SummonOperation::Add {
            target_uid,
            count: behavior.arg(1).unwrap_or(1),
            level: behavior.arg(2).unwrap_or(1),
        },
        BehaviorKind::ChangeSummonedLevel => SummonOperation::ChangeLevel {
            level: behavior.arg(1)?,
        },
        BehaviorKind::AddSummonedLevel => SummonOperation::AddLevel {
            delta: behavior.arg(1)?,
        },
        BehaviorKind::RemoveSummoned => SummonOperation::Remove {
            count: behavior.arg(2)?,
        },
        _ => return None,
    };
    Some(RuleOp::Command(BattleCommand::Summon(SummonCommand {
        origin: super::command_origin(behavior)?,
        owner_uid: source_uid,
        summoned_id: behavior.arg(0)?,
        operation,
    })))
}

pub(super) struct Handler;

pub(super) fn supports_combatant(behavior: &ParsedBehavior) -> bool {
    matches!(
        behavior.args.as_slice(),
        [model_id] if *model_id > 0
    ) || matches!(
        behavior.args.as_slice(),
        [model_id, position] if *model_id > 0 && (1..5).contains(position)
    )
}

pub(super) fn supports_special(behavior: &ParsedBehavior) -> bool {
    (behavior.raw_args.is_empty() || behavior.raw_args.len() == 1)
        && matches!(behavior.args.as_slice(), [model_id] if *model_id > 0)
}

pub(super) fn supports_add_summoned(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [summoned_id, count, level] if *summoned_id > 0 && *count > 0 && *level > 0)
}

pub(super) fn supports_change_summoned_level(behavior: &ParsedBehavior) -> bool {
    let exact_shape =
        behavior.raw_args.is_empty() && behavior.args.len() == 2 || behavior.raw_args.len() == 2;
    exact_shape
        && behavior.arg(0).is_some_and(|summoned_id| summoned_id > 0)
        && matches!(behavior.arg(1), Some(3 | 4))
}

pub(super) fn supports_add_summoned_level(behavior: &ParsedBehavior) -> bool {
    let exact_shape =
        behavior.raw_args.is_empty() && behavior.args.len() == 2 || behavior.raw_args.len() == 2;
    exact_shape
        && behavior.arg(0).is_some_and(|summoned_id| summoned_id > 0)
        && behavior.arg(1) == Some(1)
}

pub(super) fn supports_remove_summoned(behavior: &ParsedBehavior) -> bool {
    let exact_shape =
        behavior.raw_args.is_empty() && behavior.args.len() == 3 || behavior.raw_args.len() == 3;
    exact_shape
        && behavior.arg(0).is_some_and(|summoned_id| summoned_id > 0)
        && behavior.arg(1) == Some(4)
        && matches!(behavior.arg(2), Some(1 | 2))
}

impl BehaviorHandler for Handler {
    fn references(behavior: &ParsedBehavior) -> crate::engine::skill::rule::RuleReferences {
        crate::engine::skill::rule::RuleReferences {
            summons: (behavior.spec.kind == BehaviorKind::AddSummoned)
                .then(|| behavior.arg(0))
                .flatten()
                .into_iter()
                .collect(),
            models: matches!(
                behavior.spec.kind,
                BehaviorKind::Summon | BehaviorKind::SummonSp2
            )
            .then(|| behavior.arg(0))
            .flatten()
            .into_iter()
            .collect(),
            ..Default::default()
        }
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        if behavior.spec.kind == BehaviorKind::Summon {
            let (model_id, position) = match behavior.args.as_slice() {
                [model_id] => (
                    *model_id,
                    context
                        .managers
                        .first_open_combat_position(context.source_uid),
                ),
                [model_id, position] => (
                    *model_id,
                    context
                        .managers
                        .combat_position_available(context.source_uid, *position)
                        .then_some(*position),
                ),
                _ => return None,
            };
            let Some(position) = position else {
                return Some(Vec::new());
            };
            return (model_id > 0).then(|| {
                vec![RuleOp::Command(BattleCommand::Entity(EntityCommand {
                    origin: super::command_origin(behavior).expect("registered behavior"),
                    source_uid: context.source_uid,
                    target_uid: context.source_uid,
                    operation: EntityOperation::SummonCombatant { model_id, position },
                }))]
            });
        }
        if behavior.spec.kind == BehaviorKind::SummonSp2 {
            let [model_id] = behavior.args.as_slice() else {
                return None;
            };
            return (*model_id > 0).then(|| {
                vec![RuleOp::Command(BattleCommand::Entity(EntityCommand {
                    origin: super::command_origin(behavior).expect("registered behavior"),
                    source_uid: context.source_uid,
                    target_uid: context.source_uid,
                    operation: EntityOperation::SummonSpecial {
                        model_id: *model_id,
                    },
                }))]
            });
        }
        let summon = rule_op(context.source_uid, context.target_uid, behavior)?;
        let mut ops = vec![summon];
        if behavior.spec.kind == BehaviorKind::AddSummoned {
            let count = summoned_lane(behavior.arg(0)?);
            if count > 0 {
                ops.push(RuleOp::Command(BattleCommand::Buff(
                    BuffCommand::ReserveChildUids(BuffChildUidReservation {
                        origin: super::command_origin(behavior)?,
                        target_uid: context.source_uid,
                        count,
                    }),
                )));
            }
        }
        Some(ops)
    }
}

#[cfg(test)]
mod tests;
