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
mod tests {
    use sonettobuf::{Fight, FightEntityInfo, FightTeam};

    use super::*;
    use crate::engine::manager::BattleManagers;
    use crate::engine::runtime::determinism::RoundDeterminism;
    use crate::engine::skill::target::TargetPool;

    #[test]
    fn kill_emits_a_semantic_kill_command() {
        let fight = Fight {
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(50),
                    shield_value: Some(100),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let behavior = ParsedBehavior::new(60015, "Kill", Vec::new());

        assert!(matches!(
            rule_op(10, -1, &behavior, &managers.hp),
            Some(RuleOp::Command(BattleCommand::Hp(HpCommand::Kill(
                HpKill {
                    source_uid: 10,
                    target_uid: -1,
                    ..
                }
            ))))
        ));
    }

    #[test]
    fn kill_targets_resolves_its_configured_selector_without_killing_the_caster() {
        let entity = |uid, hp| FightEntityInfo {
            uid: Some(uid),
            team_type: Some(1),
            current_hp: Some(hp),
            ..Default::default()
        };
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![
                    entity(10, 100),
                    entity(11, 100),
                    entity(12, 100),
                    entity(13, 0),
                ],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let pool = TargetPool::from_fight(&fight);
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
        let mut target = crate::engine::skill::target::TargetContext::default();
        let behavior = ParsedBehavior::from_spec(
            crate::engine::skill::behavior::classify::BehaviorSpec::new(60019, "KillTargets"),
            vec![0, 102],
            vec!["0".into(), "102".into()],
        );

        let ops = Handler::emit_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 1,
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

        assert_eq!(
            ops.iter()
                .filter_map(|op| match op {
                    RuleOp::Command(BattleCommand::Hp(HpCommand::Kill(kill))) => {
                        Some(kill.target_uid)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>(),
            vec![11, 12]
        );
    }

    #[test]
    fn killing_a_dead_target_is_a_valid_empty_operation() {
        let fight = Fight {
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(0),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
        let mut target = crate::engine::skill::target::TargetContext::default();
        let behavior = ParsedBehavior::from_spec(
            crate::engine::skill::behavior::classify::BehaviorSpec::new(60015, "Kill"),
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(
            super::super::rule_ops(
                BehaviorOpContext {
                    source_uid: 10,
                    source_team: 1,
                    target_uid: -1,
                    active_skill_id: 0,
                    transfer_count: 1,
                    event: None,
                    managers: &managers,
                    pool: &TargetPool::from_fight(&fight),
                    determinism: &mut determinism,
                    modifiers: &mut modifiers,
                    target: &mut target,
                },
                &behavior,
            ),
            Some(Vec::new())
        );
    }

    #[test]
    fn lethal_hp_loss_keeps_hurt_attribution_and_bypasses_shield() {
        let fight = Fight {
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-4),
                    current_hp: Some(12_893),
                    shield_value: Some(100),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        let pool = TargetPool::from_fight(&fight);
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
        let mut target = crate::engine::skill::target::TargetContext::default();
        let behavior = ParsedBehavior::new(60018, "Kill", Vec::new());

        let ops = super::super::rule_ops(
            BehaviorOpContext {
                source_uid: 0,
                source_team: 1,
                target_uid: -4,
                active_skill_id: 530_000_157,
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
        let [RuleOp::Command(BattleCommand::Hp(command))] = ops.as_slice() else {
            panic!("lethal HP loss must emit one HP command");
        };
        let HpCommand::Lose(loss) = *command else {
            panic!("lethal HP loss must use the loss path");
        };
        let changes = managers.hp.execute_command(HpCommand::Lose(loss)).unwrap();
        let hp = changes.hp.unwrap();
        let hurt = hp.hurt.unwrap();

        assert_eq!(behavior.spec.kind, BehaviorKind::LethalHpLoss);
        assert_eq!(hp.delta, -12_893);
        assert_eq!(hurt.reduce_hp, -12_893);
        assert_eq!(hurt.effect_id, 530_000_157);
        assert_eq!(hurt.skill_id, 530_000_157);
        assert_eq!(hurt.damage_from, HurtDamageFromType::SkillEffect);
        assert_eq!(hurt.hurt_effect_type, EffectType::Kill as i32);
        assert_eq!(managers.hp.current(-4), 0);
        assert_eq!(managers.hp.shield(-4), 100);
        assert!(changes.death.is_some());
        assert!(changes.kill.is_none());

        let frame = crate::engine::runtime::record::SemanticFrame {
            owner: crate::engine::runtime::record::FrameOwner::Skill {
                source_uid: 0,
                skill_id: 530_000_157,
                card_index: 0,
                target_uid: Some(-4),
            },
            trigger: crate::engine::runtime::record::FrameTrigger::Active,
            items: vec![crate::engine::runtime::record::FrameItem::Change(Box::new(
                crate::engine::runtime::change::BattleChange::Hp(Box::new(changes)),
            ))],
        };
        let steps = crate::engine::packet::timeline::project_for_version(&[frame], 7).unwrap();
        let effects = &steps[0].act_effect;
        let projected_hurt = effects[0].hurt_info.as_ref().unwrap();

        assert_eq!(effects.len(), 2);
        assert_eq!(effects[0].effect_type, Some(EffectType::Kill as i32));
        assert_eq!(effects[0].effect_num, Some(12_893));
        assert_eq!(effects[0].config_effect, Some(60018));
        assert_eq!(projected_hurt.damage, Some(12_893));
        assert_eq!(projected_hurt.reduce_hp, Some(-12_893));
        assert_eq!(projected_hurt.effect_id, Some(530_000_157));
        assert_eq!(projected_hurt.skill_id, Some(530_000_157));
        assert_eq!(effects[1].effect_type, Some(EffectType::Dead as i32));
    }
}
