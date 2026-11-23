use crate::engine::{
    mechanic::shell::ShellCommand,
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

pub fn supports_assign(behavior: &ParsedBehavior) -> bool {
    matches!(
        behavior.args.as_slice(),
        [amount, stock_buff_id] if (*amount == -1 || *amount > 0) && *stock_buff_id > 0
    )
}

pub fn supports_recycle(behavior: &ParsedBehavior) -> bool {
    behavior.args.is_empty()
}

pub fn supports_use_skill(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [threshold, skill_id] if *threshold > 0 && *skill_id > 0)
}

impl BehaviorHandler for Handler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let origin = super::command_origin(behavior)?;
        let command = match behavior.spec.kind {
            BehaviorKind::ShellAssign if supports_assign(behavior) => {
                let [amount, stock_buff_id] = behavior.args.as_slice() else {
                    return None;
                };
                ShellCommand::Deploy {
                    origin,
                    source_uid: context.source_uid,
                    target_uid: context.target_uid,
                    stock_buff_id: *stock_buff_id,
                    amount: *amount,
                }
            }
            BehaviorKind::ShellRecycle if behavior.args.is_empty() => {
                let stock_buff_id = crate::engine::skill::buff_act::shell::caster_shell_spec(
                    context.managers,
                    context.source_uid,
                )?
                .stock_buff_id;
                ShellCommand::RetrieveAll {
                    origin,
                    source_uid: context.source_uid,
                    stock_buff_id,
                }
            }
            BehaviorKind::ShellUseSkill => {
                let [threshold, skill_id] = behavior.args.as_slice() else {
                    return None;
                };
                if context.target.shell_change_amount <= 0 {
                    return Some(Vec::new());
                }
                ShellCommand::AccumulateAndUseSkill {
                    rule: crate::engine::skill::rule::ConfiguredRuleKey::new(
                        behavior.effect_id,
                        behavior.slot,
                        origin.key,
                    ),
                    source_uid: context.source_uid,
                    target_uid: context.target_uid,
                    threshold: *threshold,
                    // "A total of N deployments and or retrievals": one per change, whatever it moved.
                    delta: 1,
                    skill_id: *skill_id,
                }
            }
            _ => return None,
        };
        Some(vec![RuleOp::Command(BattleCommand::Shell(command))])
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        if behavior.spec.kind == BehaviorKind::ShellUseSkill {
            return RuleReferences {
                skills: behavior.arg(1).into_iter().collect(),
                ..Default::default()
            };
        }
        let Some(stock_buff_id) = behavior.arg(1) else {
            return RuleReferences::default();
        };
        RuleReferences {
            buffs: std::iter::once(stock_buff_id)
                .chain(crate::engine::skill::buff_act::shell::deployed_buff_id(
                    stock_buff_id,
                ))
                .collect(),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests;
