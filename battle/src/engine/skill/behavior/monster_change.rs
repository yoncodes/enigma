use crate::engine::{
    manager::entity::{EntityCommand, EntityOperation},
    mechanic::monster_change::MonsterChangeCommand,
    skill::{
        behavior::{BehaviorOpContext, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
    },
};

pub(super) struct Handler;

pub(super) fn supports(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [model_id, _, _] if *model_id > 0)
}

impl BehaviorHandler for Handler {
    fn references(behavior: &ParsedBehavior) -> crate::engine::skill::rule::RuleReferences {
        crate::engine::skill::rule::RuleReferences {
            models: behavior.arg(0).into_iter().collect(),
            ..Default::default()
        }
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let [model_id, parameter1, parameter2] = behavior.args.as_slice() else {
            return None;
        };
        (*model_id > 0).then(|| {
            vec![RuleOp::Command(BattleCommand::MonsterChange(
                MonsterChangeCommand {
                    entity: EntityCommand {
                        origin: super::command_origin(behavior).expect("registered behavior"),
                        source_uid: context.source_uid,
                        target_uid: context.target_uid,
                        operation: EntityOperation::Transform {
                            model_id: *model_id,
                            parameters: [*parameter1, *parameter2],
                        },
                    },
                },
            ))]
        })
    }
}

#[cfg(test)]
mod tests;
