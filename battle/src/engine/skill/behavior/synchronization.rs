use crate::engine::{
    damage::AttackPlan,
    manager::{
        buff::{BuffCommand, BuffSetState},
        ex_point::{
            ExPointCommand, ExPointConfigureSynchronization, ExPointRecordSynchronizationAction,
            SynchronizationDefinition,
        },
    },
    skill::{
        action::{SkillRequest, SkillTarget},
        behavior::{BehaviorOpContext, classify::BehaviorKind, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::{
            RuleReferences,
            output::{BattleCommand, RuleOp},
        },
    },
};
use sonettobuf::effect_type_enum::EffectType;

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        if behavior.spec.kind == BehaviorKind::EzioBigSkillCheckTimes {
            return synchronization_progress_ops(context, behavior);
        }
        if behavior.spec.kind != BehaviorKind::EzioProps {
            return damage_rule_ops(context, behavior);
        }
        let definition = definition(behavior)?;
        Some(vec![RuleOp::Command(BattleCommand::ExPoint(
            ExPointCommand::ConfigureSynchronization(ExPointConfigureSynchronization {
                origin: super::command_origin(behavior)?,
                target_uid: context.target_uid,
                definition,
            }),
        ))])
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        RuleReferences {
            skills: definition(behavior)
                .map(|definition| definition.skills.to_vec())
                .unwrap_or_default(),
            buffs: Vec::new(),
            models: Vec::new(),
        }
    }
}

fn synchronization_progress_ops(
    context: BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    if !behavior.args.is_empty() {
        return None;
    }
    let origin = super::command_origin(behavior)?;
    let definition = context
        .managers
        .ex_point
        .synchronization_definition(context.source_uid)?;
    let before = context
        .managers
        .ex_point
        .synchronization_progress(context.source_uid)
        .unwrap_or_default();
    let action_target_uid = context.target.runtime_target_uid;
    let damage = context.target.action_damage_amount.max(0);
    let completed_actions = before.completed_actions + 1;
    let remaining_actions = definition.action_count.saturating_sub(completed_actions);
    let mut ops = vec![RuleOp::Command(BattleCommand::ExPoint(
        ExPointCommand::RecordSynchronizationAction(ExPointRecordSynchronizationAction {
            origin,
            target_uid: context.source_uid,
            action_target_uid,
            damage,
        }),
    ))];
    let (buff_uid, act_id, _) = context.managers.buff.buff_act_carrier(
        context.source_uid,
        crate::engine::skill::buff_act::registry::BuffActKind::EzioBigSkill,
    )?;
    ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::SetState(
        BuffSetState {
            origin,
            target_uid: context.source_uid,
            buff_uid,
            ex_info: None,
            params: Some(format!(
                "{act_id}#{},{},{}",
                remaining_actions,
                before.total_damage.saturating_add(damage),
                i32::try_from(action_target_uid).ok()?
            )),
            act_info: None,
        },
    ))));
    if remaining_actions == 0 {
        let mut finisher: crate::engine::skill::action::SkillInvocation = SkillRequest {
            source_uid: context.source_uid,
            skill_id: definition.skills[2],
        }
        .into();
        finisher.target = SkillTarget::Explicit(action_target_uid);
        ops.push(RuleOp::Skill(finisher));
    }
    Some(ops)
}

fn damage_rule_ops(
    context: BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let hp = context.managers.hp.get(context.target_uid);
    let (rate, hurt_effect_type) = damage_rate(behavior, hp.current, hp.max)?;
    let is_crit = context.determinism.roll_hidden_crit(
        context.active_skill_id,
        context.source_uid,
        context.target_uid,
        crate::engine::damage::handler::crit_chance(
            context.source_uid,
            context.target_uid,
            context.pool,
            context.managers,
        ),
    );
    let rate_terms = crate::engine::skill::buff_act::life_attack_fix_rate::active_damage_rate_terms(
        context.source_uid,
        &context.managers.buff,
        &context.managers.hp,
    );
    let mut command = crate::engine::damage::handler::resolve_attack_command(
        &AttackPlan {
            source_uid: context.source_uid,
            target_uid: context.target_uid,
            skill_id: context.active_skill_id,
            rate,
            rate_terms,
            attack_attributes: context.modifiers.attack_attributes.clone(),
            career_ratio_bonus: context.modifiers.career_ratio_bonus,
            attack_career: context.modifiers.attack_career,
            is_conduit: context
                .managers
                .conduit
                .owns_skill(context.source_uid, context.active_skill_id),
            is_crit,
            assassinate: true,
            main_target: true,
            extra_skill_kind: context.target.extra_skill_kind,
            additional_enabled: false,
            additional_is_crit: None,
        },
        crate::engine::damage::handler::DamageRuntime {
            fight_version: context.managers.fight_version(),
            pool: context.pool,
            attributes: &context.managers.attribute,
            buffs: &context.managers.buff,
            target_buffs: &context.managers.buff,
            hp: &context.managers.hp,
            fields: Some(&context.managers.field),
            emitter: None,
            team_inspiration: 0,
        },
        super::command_origin(behavior)?,
    )?;
    let crate::engine::manager::hp::HpCommand::Damage(damage) = &mut command else {
        return None;
    };
    damage.config_effect = behavior.config_effect;
    damage.hurt.damage_from = crate::engine::manager::hp::HurtDamageFromType::SkillEffect;
    damage.hurt.hurt_effect_type = hurt_effect_type;
    Some(vec![RuleOp::Command(BattleCommand::Hp(command))])
}

fn damage_rate(behavior: &ParsedBehavior, current_hp: i32, max_hp: i32) -> Option<(i32, i32)> {
    match (
        behavior.spec.key.opcode,
        behavior.spec.kind,
        behavior.args.as_slice(),
    ) {
        (100001, BehaviorKind::EzioBigSkillType1, [base, bonus, threshold]) => Some((
            *base
                + if current_hp.saturating_mul(1000) < max_hp.saturating_mul(*threshold) {
                    *bonus
                } else {
                    0
                },
            EffectType::Eziobigskilldamage as i32,
        )),
        (100002, BehaviorKind::EzioBigSkillType2, [base, bonus, threshold]) => Some((
            *base
                + if current_hp.saturating_mul(1000) >= max_hp.saturating_mul(*threshold) {
                    *bonus
                } else {
                    0
                },
            EffectType::Eziobigskilldamage as i32,
        )),
        (100003, BehaviorKind::EzioBigSkillEnd, [rate]) => {
            Some((*rate, EffectType::Eziobigskillorigindamage as i32))
        }
        _ => None,
    }
}

fn definition(behavior: &ParsedBehavior) -> Option<SynchronizationDefinition> {
    let [first, second, finisher, action_count, threshold] = behavior.args.as_slice() else {
        return None;
    };
    SynchronizationDefinition::new([*first, *second, *finisher], *action_count, *threshold)
}

#[cfg(test)]
mod tests {
    use sonettobuf::{Fight, FightEntityInfo, FightTeam};

    use super::*;
    use crate::engine::{
        manager::BattleManagers,
        runtime::determinism::RoundDeterminism,
        skill::{
            action::SkillModifiers,
            behavior::classify::BehaviorSpec,
            target::{TargetContext, TargetPool},
        },
    };

    #[test]
    fn props_configure_the_resource_owner_and_reference_data_skills() {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1),
                    ex_point_type: Some(2),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let pool = TargetPool::from_fight(&fight);
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(100000, "EzioProps"),
            vec![101, 102, 103, 4, 100],
            Vec::new(),
        );
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();

        let ops = Handler::emit_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 0,
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
            [RuleOp::Command(BattleCommand::ExPoint(
                ExPointCommand::ConfigureSynchronization(ExPointConfigureSynchronization {
                    definition: SynchronizationDefinition {
                        skills: [101, 102, 103],
                        action_count: 4,
                        threshold: 100
                    },
                    ..
                })
            ))]
        ));
        assert_eq!(Handler::references(&behavior).skills, vec![101, 102, 103]);
    }

    #[test]
    fn malformed_props_fail_closed() {
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(100000, "EzioProps"),
            vec![101, 102, 103, 4],
            Vec::new(),
        );

        assert!(definition(&behavior).is_none());
    }

    #[test]
    fn qte_damage_rates_follow_the_configured_hp_side() {
        let first = ParsedBehavior::from_spec(
            BehaviorSpec::new(100001, "EzioBigSkillTyp1"),
            vec![1400, 500, 500],
            Vec::new(),
        );
        let second = ParsedBehavior::from_spec(
            BehaviorSpec::new(100002, "EzioBigSkillTyp2"),
            vec![1400, 500, 500],
            Vec::new(),
        );
        let finisher = ParsedBehavior::from_spec(
            BehaviorSpec::new(100003, "EzioBigSkillEnd"),
            vec![9000],
            Vec::new(),
        );

        assert_eq!(damage_rate(&first, 49, 100), Some((1900, 1000)));
        assert_eq!(damage_rate(&first, 50, 100), Some((1400, 1000)));
        assert_eq!(damage_rate(&second, 49, 100), Some((1400, 1000)));
        assert_eq!(damage_rate(&second, 50, 100), Some((1900, 1000)));
        assert_eq!(damage_rate(&finisher, 1, 100), Some((9000, 1001)));
    }
}
