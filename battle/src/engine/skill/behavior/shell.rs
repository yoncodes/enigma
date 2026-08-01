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
                let stock_buff_id = context
                    .managers
                    .buff
                    .active_features(&context.managers.hp)
                    .into_iter()
                    .find(|feature| {
                        feature.owner_uid == context.source_uid
                            && crate::engine::skill::buff_act::is_kind(
                                feature,
                                crate::engine::skill::buff_act::registry::BuffActKind::ShellProcess,
                            )
                            && feature.values.get(1) == Some(&feature.buff_id)
                    })?
                    .buff_id;
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
                    origin,
                    source_uid: context.source_uid,
                    target_uid: context.target_uid,
                    threshold: *threshold,
                    delta: context.target.shell_change_amount,
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
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    use super::*;
    use crate::engine::{
        manager::BattleManagers,
        runtime::determinism::RoundDeterminism,
        skill::{
            action::SkillModifiers,
            behavior::{self, classify::BehaviorSpec},
            target::{TargetContext, TargetPool},
        },
    };

    #[test]
    fn shell_assign_moves_the_configured_amount_from_stock_to_target() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    buffs: vec![BuffInfo {
                        uid: Some(52),
                        buff_id: Some(31090111),
                        layer: Some(16),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let pool = TargetPool::from_fight(&fight);
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(60133, "ShellAssign"),
            vec![3, 31090111],
            Vec::new(),
        );
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();

        let ops = behavior::rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: -1,
                active_skill_id: 31090111,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        )
        .unwrap();

        assert!(matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::Shell(
                ShellCommand::Deploy {
                    source_uid: 10,
                    target_uid: -1,
                    stock_buff_id: 31090111,
                    amount: 3,
                    ..
                }
            ))]
        ));
    }

    #[test]
    fn shell_use_skill_forwards_event_progress_and_configured_skill() {
        let managers = BattleManagers::default();
        let pool = TargetPool::default();
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(60135, "ShellUseSkill"),
            vec![5, 31090174],
            Vec::new(),
        );
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext {
            shell_change_amount: 3,
            ..Default::default()
        };

        let ops = behavior::rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: -1,
                active_skill_id: 31090196,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        )
        .unwrap();

        assert!(matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::Shell(
                ShellCommand::AccumulateAndUseSkill {
                    source_uid: 10,
                    target_uid: -1,
                    threshold: 5,
                    delta: 3,
                    skill_id: 31090174,
                    ..
                }
            ))]
        ));
    }
}
