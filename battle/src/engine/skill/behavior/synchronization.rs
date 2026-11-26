use crate::engine::{
    damage::{AttackPlan, DamageRateTerm},
    entity::attr::AttrId,
    manager::{
        buff::{BuffCommand, BuffRemove, BuffRemoveSelector, BuffSetState},
        ex_point::{
            ExPointChange, ExPointCommand, ExPointCompleteSynchronization,
            ExPointConfigureSynchronization, ExPointPublishSynchronizationHit,
            ExPointRecordSynchronizationAction, ExPointRecordSynchronizationDamage,
            SynchronizationDefinition,
        },
        hp::{DamageEffectKind, DamageRecord, HpCommand, HurtDamageFromType},
    },
    skill::{
        action::{SkillRequest, SkillTarget},
        behavior::{
            BehaviorOpContext,
            classify::BehaviorKind,
            rate::{self, RateRuntime},
            registry::BehaviorHandler,
        },
        effect::{ParsedBehavior, SkillEffectCatalog},
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
        emit_ops(
            context,
            behavior,
            crate::engine::skill::effect::catalog::global(),
        )
    }

    fn emit_runtime_ops(
        context: BehaviorOpContext<'_>,
        slot: &crate::engine::skill::effect::SkillEffectSlot,
        catalog: &SkillEffectCatalog,
    ) -> Option<Vec<RuleOp>> {
        emit_ops(context, &slot.behavior, catalog)
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        RuleReferences {
            skills: definition(behavior)
                .map(|definition| definition.skills.to_vec())
                .unwrap_or_default(),
            buffs: Vec::new(),
            models: Vec::new(),
            summons: Vec::new(),
        }
    }
}

fn emit_ops(
    context: BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
    catalog: &SkillEffectCatalog,
) -> Option<Vec<RuleOp>> {
    if behavior.spec.kind == BehaviorKind::EzioBigSkillCheckTimes {
        return synchronization_progress_ops(context, behavior);
    }
    if behavior.spec.kind != BehaviorKind::EzioProps {
        return damage_rule_ops(context, behavior, catalog);
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
    let completed_actions = before.completed_actions + 1;
    let remaining_actions = definition.action_count.saturating_sub(completed_actions);
    let mut ops = vec![RuleOp::Command(BattleCommand::ExPoint(
        ExPointCommand::RecordSynchronizationAction(ExPointRecordSynchronizationAction {
            origin,
            target_uid: context.source_uid,
            action_target_uid,
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
                before.total_damage,
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
        finisher.mode = crate::engine::skill::action::SkillExecutionMode::Synchronization;
        finisher.start = crate::engine::skill::action::SkillStart::AfterCurrentAction;
        ops.push(RuleOp::Skill(finisher));
    }
    Some(ops)
}

fn damage_rule_ops(
    context: BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
    catalog: &SkillEffectCatalog,
) -> Option<Vec<RuleOp>> {
    let origin = super::command_origin(behavior)?;
    let definition = context
        .managers
        .ex_point
        .synchronization_definition(context.source_uid)?;
    let progress = context
        .managers
        .ex_point
        .synchronization_progress(context.source_uid)?;
    let terminal = if behavior.spec.kind == BehaviorKind::EzioBigSkillEnd {
        Some((
            progress,
            context.managers.buff.buff_act_carrier(
                context.source_uid,
                crate::engine::skill::buff_act::registry::BuffActKind::EzioBigSkill,
            )?,
        ))
    } else {
        None
    };
    let hp = context.managers.hp.get(context.target_uid);
    let (rate, _) = damage_rate(behavior, hp.current, hp.max)?;
    let mut modifiers = context.modifiers.clone();
    let mut target_context = *context.target;
    target_context.hit_source_uid = context.source_uid;
    target_context.hit_target_uid = context.target_uid;
    target_context.runtime_target_uid = context.target_uid;
    let runtime = RateRuntime {
        effects: catalog,
        managers: context.managers,
        pool: context.pool,
        context: target_context,
    };
    rate::emit_active_target_attack_attributes(
        &mut modifiers,
        context.source_uid,
        context.active_skill_id,
        context.active_skill_id,
        runtime,
        context.determinism,
    );
    let passive_skills = context
        .pool
        .entity(context.source_uid)
        .map(|entity| entity.passive_skills.as_slice())
        .unwrap_or_default();
    rate::emit_passive_attack_attributes(
        &mut modifiers,
        context.source_uid,
        context.active_skill_id,
        passive_skills,
        runtime,
        context.determinism,
    );
    modifiers.merge(rate::incoming_target_attack_modifiers(
        context.source_uid,
        context.target_uid,
        context.active_skill_id,
        runtime,
        context.determinism,
    ));
    let extra_action = crate::engine::skill::action::performs_extra_action(
        target_context.active_card_index,
        target_context.active_skill_mode,
        target_context.extra_skill_kind,
    );
    let mut attack_attributes = modifiers.primary_attack_attributes();
    let target_count_damage_bonus =
        crate::engine::skill::buff_act::attr_by_skill_target_count::owner_attribute_delta(
            context.managers,
            context.source_uid,
            target_context.damage_target_count_kind,
            AttrId::DmgBonus,
        );
    if target_count_damage_bonus != 0 {
        attack_attributes.push((AttrId::DmgBonus, target_count_damage_bonus));
    }
    let source_damage_type = crate::engine::skill::buff_act::emitter_damage_type::source_type(
        context.source_uid,
        context.pool,
        context.managers,
    );
    let is_big_skill = catalog.is_big_skill(context.active_skill_id);
    attack_attributes.extend(
        crate::engine::skill::buff_act::incoming_target_attack_attributes(
            context.managers,
            context.target_uid,
            source_damage_type,
            is_big_skill,
        ),
    );
    let assassination = crate::engine::skill::buff_act::assassination::target_modifier(
        context.managers,
        context.source_uid,
        context.target_uid,
        true,
        None,
        None,
    );
    if assassination.final_damage_bonus != 0 {
        attack_attributes.push((AttrId::FinalDmgBonus, assassination.final_damage_bonus));
    }
    for attr_id in [AttrId::CriticalDmg, AttrId::DmgBonus] {
        let delta = crate::engine::skill::buff_act::target_attack_attribute_delta(
            context.managers,
            context.target_uid,
            extra_action,
            attr_id,
        );
        if delta != 0 {
            attack_attributes.push((attr_id, delta));
        }
    }
    let excess_crit = crate::engine::damage::handler::excess_crit_rate(
        context.source_uid,
        context.target_uid,
        context.pool,
        context.managers,
        &attack_attributes,
    );
    let conversion_rate = modifiers.critical_conversion_rate()
        + crate::engine::skill::buff_act::crit_rate_alter2::owner_conversion_rate(
            context.source_uid,
            &context.managers.buff,
            &context.managers.hp,
        )
        + crate::engine::skill::buff_act::crit_rate_alter_by_other_buff::owner_conversion_rate(
            context.source_uid,
            &context.managers.buff,
            &context.managers.hp,
        );
    let conversion_numerator = i64::from(excess_crit) * i64::from(conversion_rate);
    let converted = (conversion_numerator / 1000) as i32;
    let critical_multiplier_remainder = (conversion_numerator % 1000) as i32;
    if converted != 0 {
        attack_attributes.push((AttrId::CriticalDmg, converted));
    }
    let forces_critical = modifiers.force_critical
        || crate::engine::skill::buff_act::must_crit_and_fix_temp_attr::active_forces_critical(
            context.source_uid,
            context.managers,
            extra_action,
        );
    let is_crit = context.determinism.roll_hidden_crit(
        context.active_skill_id,
        context.source_uid,
        context.target_uid,
        if forces_critical {
            1000
        } else {
            crate::engine::damage::handler::crit_chance_with_attack_attributes(
                context.source_uid,
                context.target_uid,
                context.pool,
                context.managers,
                &attack_attributes,
            )
        },
    );
    let rate_terms = modifiers
        .rates
        .iter()
        .filter(|modifier| modifier.target_uid == 0 || modifier.target_uid == context.target_uid)
        .map(|modifier| DamageRateTerm {
            opcode: modifier.opcode,
            rate: modifier.amount.resolve(&context.managers.gauge),
            career_scaled: modifier.career_scaled,
            composition: modifier.composition,
        })
        .chain(
            crate::engine::skill::buff_act::life_attack_fix_rate::active_damage_rate_terms(
                context.source_uid,
                &context.managers.buff,
                &context.managers.hp,
                None,
            ),
        )
        .collect::<Vec<_>>();
    let mut command = crate::engine::damage::handler::resolve_attack_command(
        &AttackPlan {
            source_uid: context.source_uid,
            target_uid: context.target_uid,
            skill_id: context.active_skill_id,
            rate,
            rate_terms,
            attack_attributes,
            attack_bonus: modifiers.caster_attack_bonus,
            career_ratio_bonus: modifiers.career_ratio_bonus,
            attack_career: modifiers.attack_career,
            additional_attack_career: modifiers.additional_attack_career,
            force_career_restraint: modifiers.force_career_restraint,
            critical_multiplier_remainder,
            is_conduit: context
                .managers
                .conduit
                .owns_skill(context.source_uid, context.active_skill_id),
            is_crit,
            assassinate: assassination.assassinate,
            main_target: true,
            extra_skill_kind: context.target.extra_skill_kind,
            performs_extra_action: extra_action,
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
            fields: Some((&context.managers.field, context.managers.catalog())),
            emitter: Some(&context.managers.emitter),
            team_inspiration: 0,
        },
        origin,
    )?;
    let crate::engine::manager::hp::HpCommand::Damage(damage) = &mut command else {
        return None;
    };
    damage.config_effect = behavior.config_effect;
    damage.hurt.damage_from = HurtDamageFromType::SkillEffect;
    let finisher_damage = damage.amount.max(0);
    let record = DamageRecord {
        amount: damage.amount,
        config_effect: damage.config_effect,
        effect_kind: damage.effect_kind,
        assassinate: damage.assassinate,
        ignore_riposte: damage.ignore_riposte,
        hurt: damage.hurt,
    };
    let mut ops = vec![RuleOp::Command(BattleCommand::ExPoint(
        ExPointCommand::PublishSynchronizationHit(ExPointPublishSynchronizationHit {
            origin,
            target_uid: context.source_uid,
            action_target_uid: context.target_uid,
            damage: record,
        }),
    ))];
    if progress.completed_actions == 0 {
        ops.push(RuleOp::Command(BattleCommand::ExPoint(
            ExPointCommand::Spend(ExPointChange {
                origin,
                source_uid: context.source_uid,
                target_uid: context.source_uid,
                delta: -definition.threshold,
                config_effect: 0,
                effect_type: EffectType::Expointchange as i32,
            }),
        )));
    }
    ops.push(RuleOp::Command(BattleCommand::ExPoint(
        ExPointCommand::RecordSynchronizationDamage(ExPointRecordSynchronizationDamage {
            origin,
            target_uid: context.source_uid,
            action_target_uid: context.target_uid,
            damage: record,
        }),
    )));
    if let Some((progress, (buff_uid, act_id, _))) = terminal {
        let total_damage = progress.total_damage.saturating_add(finisher_damage);
        ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::SetState(
            BuffSetState {
                origin,
                target_uid: context.source_uid,
                buff_uid,
                ex_info: None,
                params: Some(format!(
                    "{act_id}#-1,{},{}",
                    total_damage, progress.target_uid,
                )),
                act_info: None,
            },
        ))));
        damage.amount = total_damage;
        damage.effect_kind = DamageEffectKind::Normal;
        damage.hurt.hurt_effect_type = EffectType::Damage as i32;
        command = HpCommand::SettleAccumulatedDamage(*damage);
        ops.push(RuleOp::Command(BattleCommand::Hp(command)));
        ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(
            BuffRemove {
                origin,
                target_uid: context.source_uid,
                selector: BuffRemoveSelector::Uid(buff_uid),
            },
        ))));
        ops.push(RuleOp::Command(BattleCommand::ExPoint(
            ExPointCommand::CompleteSynchronization(ExPointCompleteSynchronization {
                origin,
                target_uid: context.source_uid,
            }),
        )));
    }
    Some(ops)
}

pub(super) fn supports_props(behavior: &ParsedBehavior) -> bool {
    definition(behavior).is_some()
}

pub(super) fn supports_big_skill_rate(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [base, bonus, threshold] if *base > 0 && *bonus >= 0 && *threshold > 0)
}

pub(super) fn supports_big_skill_end(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [rate] if *rate > 0)
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
            Some((*rate, EffectType::Eziobigskilldamage as i32))
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
mod tests;
