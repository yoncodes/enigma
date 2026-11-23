use crate::engine::{
    mechanic::buff_precast::{BuffPrecastCommand, BuffPrecastOption},
    skill::{
        behavior::{BehaviorOpContext, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::{
            RuleReferences,
            output::{BattleCommand, RuleOp},
        },
    },
};

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let (buff_id, costs, ranks, group_index) = arguments(behavior)?;
        let owner = context.pool.entity(context.source_uid)?;
        let group = match group_index {
            1 => &owner.skill_group1,
            2 => &owner.skill_group2,
            _ => return None,
        };
        let options = costs
            .into_iter()
            .zip(ranks)
            .map(|(cost, rank)| {
                Some(BuffPrecastOption {
                    cost,
                    skill_id: *group.get(usize::try_from(rank - 1).ok()?)?,
                })
            })
            .collect::<Option<Vec<_>>>()?;

        Some(vec![RuleOp::Command(BattleCommand::BuffPrecast(
            BuffPrecastCommand {
                origin: super::command_origin(behavior)?,
                source_uid: context.source_uid,
                target_uid: context.target_uid,
                buff_id,
                options,
            },
        ))])
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        RuleReferences {
            buffs: behavior.arg(0).into_iter().collect(),
            ..Default::default()
        }
    }
}

fn arguments(behavior: &ParsedBehavior) -> Option<(i32, Vec<i32>, Vec<i32>, i32)> {
    let buff_id = behavior.arg(0)?;
    let costs = behavior.arg_list(1)?;
    let ranks = behavior.arg_list(2)?;
    let group = behavior.arg(3)?;
    (buff_id > 0
        && !costs.is_empty()
        && costs.len() == ranks.len()
        && costs.iter().all(|cost| *cost > 0)
        && ranks.iter().all(|rank| (1..=3).contains(rank))
        && matches!(group, 1 | 2))
    .then_some((buff_id, costs, ranks, group))
}

pub(super) fn supports_arguments(behavior: &ParsedBehavior) -> bool {
    arguments(behavior).is_some()
}

#[cfg(test)]
mod tests;
