use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    manager::{
        buff::{BuffCommand, BuffGrant, BuffSetState},
        hp::{HpCommand, HpLoss, HurtDamageFromType, HurtInfoData},
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

impl BehaviorHandler for Handler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let origin = super::command_origin(behavior)?;
        match behavior.spec.kind {
            BehaviorKind::OriginDamageFromInjuryBankBuff => {
                let [permille] = behavior.args.as_slice() else {
                    return None;
                };
                if *permille <= 0 {
                    return None;
                }
                let amount = crate::engine::skill::buff_act::injury_bank::state(
                    context.managers,
                    context.source_uid,
                )
                .map(|state| state.current * *permille / 1000)
                .unwrap_or_default();
                if amount <= 0 {
                    return Some(Vec::new());
                }
                Some(vec![RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(
                    HpLoss {
                        origin,
                        source_uid: context.source_uid,
                        target_uid: context.target_uid,
                        amount,
                        config_effect: behavior.config_effect,
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
                            hurt_effect_type: EffectType::Origindamage as i32,
                            display_amount: None,
                        }),
                    },
                )))])
            }
            BehaviorKind::RealDamageSelfAndAddBuffToTarget => {
                let [loss_permille, buff_id] = behavior.args.as_slice() else {
                    return None;
                };
                if *loss_permille <= 0 || *buff_id <= 0 {
                    return None;
                }
                let amount = context.managers.hp.max(context.source_uid) * *loss_permille / 1000;
                Some(vec![
                    RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(HpLoss {
                        origin,
                        source_uid: context.source_uid,
                        target_uid: context.source_uid,
                        amount,
                        config_effect: behavior.config_effect,
                        hurt: None,
                    }))),
                    RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                        origin,
                        source_uid: context.source_uid,
                        target_uid: context.target_uid,
                        buff_id: *buff_id,
                        amount: None,
                        occurrences: 1,
                        child_uid_reservations: 0,
                    }))),
                ])
            }
            BehaviorKind::ClearInjuryBankBuffOriginDamage => {
                let [permille, clear, _] = behavior.args.as_slice() else {
                    return None;
                };
                if *permille < 0 {
                    return None;
                }
                let Some((feature, state)) =
                    crate::engine::skill::buff_act::injury_bank::feature_state(
                        context.managers,
                        context.source_uid,
                    )
                else {
                    return Some(Vec::new());
                };
                let mut ops = Vec::with_capacity(2);
                if *clear != 0 {
                    ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::SetState(
                        BuffSetState {
                            ex_info: None,
                            origin,
                            target_uid: context.source_uid,
                            buff_uid: feature.buff_uid,
                            params: Some(format!("{}#0#{}", feature.act_id()?, state.cap)),
                            act_info: None,
                        },
                    ))));
                }
                let amount = state.current * *permille / 1000;
                if amount > 0 {
                    ops.push(RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(
                        HpLoss {
                            origin,
                            source_uid: context.source_uid,
                            target_uid: context.target_uid,
                            amount,
                            config_effect: behavior.config_effect,
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
                                hurt_effect_type: EffectType::Origindamage as i32,
                                display_amount: None,
                            }),
                        },
                    ))));
                }
                Some(ops)
            }
            _ => None,
        }
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        RuleReferences {
            skills: Vec::new(),
            buffs: matches!(
                behavior.spec.kind,
                BehaviorKind::RealDamageSelfAndAddBuffToTarget
            )
            .then(|| behavior.arg(1))
            .flatten()
            .into_iter()
            .collect(),
            models: Vec::new(),
            summons: Vec::new(),
        }
    }
}
