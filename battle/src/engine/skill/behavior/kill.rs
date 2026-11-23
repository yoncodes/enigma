use crate::engine::{
    manager::hp::{HpCommand, HpKill, HpLoss, HpManager, HurtDamageFromType, HurtInfoData},
    skill::{
        behavior::{BehaviorOpContext, classify::BehaviorKind, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
        target::{TargetRequest, TargetResolver},
    },
};
use sonettobuf::effect_type_enum::EffectType;

pub fn rule_op(
    source_uid: i64,
    target_uid: i64,
    behavior: &ParsedBehavior,
    hp: &HpManager,
) -> Option<RuleOp> {
    if behavior.spec.kind != BehaviorKind::Kill {
        return None;
    }
    let amount = hp.current(target_uid);
    if amount <= 0 {
        return None;
    }
    Some(RuleOp::Command(BattleCommand::Hp(HpCommand::Kill(
        HpKill {
            origin: super::command_origin(behavior)?,
            source_uid,
            target_uid,
            config_effect: behavior.spec.key.opcode,
        },
    ))))
}

pub(super) fn supports(behavior: &ParsedBehavior) -> bool {
    behavior.args.is_empty()
}

pub(super) fn supports_targets(behavior: &ParsedBehavior) -> bool {
    (behavior.raw_args.is_empty() || behavior.raw_args.len() == 2)
        && matches!(behavior.args.as_slice(), [0, 102 | 103])
}

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        if behavior.spec.kind == BehaviorKind::KillTargets {
            let [0, target_code] = behavior.args.as_slice() else {
                return None;
            };
            let targets = TargetResolver::resolve_with_managers_and_context(
                &TargetRequest {
                    code: *target_code,
                    raw: Vec::new(),
                },
                context.active_skill_id,
                context.source_uid,
                context.pool,
                context.determinism,
                Some(context.managers),
                *context.target,
            );
            let origin = super::command_origin(behavior)?;
            return Some(
                targets
                    .into_iter()
                    .filter(|target_uid| context.managers.hp.current(*target_uid) > 0)
                    .map(|target_uid| {
                        RuleOp::Command(BattleCommand::Hp(HpCommand::Kill(HpKill {
                            origin,
                            source_uid: context.source_uid,
                            target_uid,
                            config_effect: behavior.spec.key.opcode,
                        })))
                    })
                    .collect(),
            );
        }
        if behavior.spec.kind == BehaviorKind::LethalHpLoss {
            let amount = context.managers.hp.current(context.target_uid);
            return Some(if amount <= 0 {
                Vec::new()
            } else {
                let loss = HpLoss {
                    origin: super::command_origin(behavior)?,
                    source_uid: context.source_uid,
                    target_uid: context.target_uid,
                    amount,
                    config_effect: behavior.spec.key.opcode,
                    hurt: Some(HurtInfoData {
                        from_uid: context.source_uid,
                        is_crit: false,
                        career_restraint: false,
                        reduce_hp: 0,
                        effect_id: context.active_skill_id,
                        skill_id: context.active_skill_id,
                        damage_from: HurtDamageFromType::SkillEffect,
                        buff_act_id: 0,
                        buff_uid: 0,
                        hurt_effect_type: EffectType::Kill as i32,
                        display_amount: None,
                    }),
                };
                vec![RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(loss)))]
            });
        }
        if context.managers.hp.current(context.target_uid) <= 0 {
            return Some(Vec::new());
        }
        rule_op(
            context.source_uid,
            context.target_uid,
            behavior,
            &context.managers.hp,
        )
        .map(|op| vec![op])
    }
}

#[cfg(test)]
mod tests;
