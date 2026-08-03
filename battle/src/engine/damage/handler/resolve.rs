use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    damage::{
        AttackPlan, DamageFormula, DamageFormulaInput as DamageInputs, DamageKind, DamageRateTerm,
        calculate_with_trace_for_version as calculate_damage_with_trace,
    },
    entity::attr::AttrId,
    manager::{
        attribute::AttributeManager,
        buff::BuffManager,
        field::FieldManager,
        hp::{DamageEffectKind, HpCommand, HpDamage, HpManager, HurtDamageFromType, HurtInfoData},
    },
    skill::{
        buff_act::{is_kind, registry::BuffActKind},
        rule::CommandOrigin,
        target::{TargetEntity, TargetPool},
    },
};

use super::{
    affinity::career_multiplier_against, critical_technique_bonus, regular_multiplier,
    restrains_target, strongest_career_multiplier,
};

#[derive(Clone, Copy)]
pub struct DamageRequest<'a> {
    pub source_uid: i64,
    pub target_uid: i64,
    pub skill_id: i32,
    pub rate: i32,
    pub rate_terms: &'a [DamageRateTerm],
    pub attack_attributes: &'a [(AttrId, i32)],
    pub career_ratio_bonus: i32,
    pub attack_career: Option<i32>,
    pub is_conduit: bool,
    pub is_crit: bool,
    pub extra_skill_kind: i32,
}

#[derive(Clone, Copy)]
pub struct DamageRuntime<'a> {
    pub fight_version: i32,
    pub pool: &'a TargetPool,
    pub attributes: &'a AttributeManager,
    pub buffs: &'a BuffManager,
    pub target_buffs: &'a BuffManager,
    pub hp: &'a HpManager,
    pub fields: Option<&'a FieldManager>,
    pub emitter: Option<&'a crate::engine::manager::emitter::EmitterManager>,
    pub team_inspiration: i32,
}

pub fn resolve_attack_command(
    plan: &AttackPlan,
    runtime: DamageRuntime<'_>,
    origin: CommandOrigin,
) -> Option<HpCommand> {
    let resolved = resolve_row_damage_result(
        DamageRequest {
            source_uid: plan.source_uid,
            target_uid: plan.target_uid,
            skill_id: plan.skill_id,
            rate: plan.rate,
            rate_terms: &plan.rate_terms,
            attack_attributes: &plan.attack_attributes,
            career_ratio_bonus: plan.career_ratio_bonus,
            attack_career: plan.attack_career,
            is_conduit: plan.is_conduit,
            is_crit: plan.is_crit,
            extra_skill_kind: plan.extra_skill_kind,
        },
        runtime,
    )?;
    Some(HpCommand::Damage(HpDamage {
        origin,
        source_uid: resolved.source_uid,
        target_uid: resolved.target_uid,
        amount: resolved.amount,
        config_effect: -1,
        effect_kind: if plan.is_crit {
            DamageEffectKind::Critical
        } else {
            DamageEffectKind::Normal
        },
        assassinate: plan.assassinate,
        ignore_riposte: false,
        hurt: resolved.hurt,
    }))
}

pub fn resolve_avoided_attack_command(
    source_uid: i64,
    target_uid: i64,
    origin: CommandOrigin,
) -> HpCommand {
    HpCommand::Damage(HpDamage {
        origin,
        source_uid,
        target_uid,
        amount: 0,
        config_effect: -1,
        effect_kind: DamageEffectKind::Avoided,
        assassinate: false,
        ignore_riposte: false,
        hurt: HurtInfoData {
            from_uid: source_uid,
            is_crit: false,
            career_restraint: false,
            reduce_hp: 0,
            effect_id: 0,
            skill_id: 0,
            damage_from: HurtDamageFromType::Skill,
            buff_act_id: 0,
            buff_uid: 0,
            hurt_effect_type: EffectType::Miss as i32,
            display_amount: None,
        },
    })
}

#[allow(clippy::too_many_arguments)]
pub fn resolve_configured_replacement_damage_command(
    request: DamageRequest<'_>,
    runtime: DamageRuntime<'_>,
    replacement_buff_id: i32,
    origin: CommandOrigin,
    config_effect: i32,
    hurt_effect_type: i32,
) -> Option<HpCommand> {
    let mut feature = BuffManager::configured_features(replacement_buff_id)
        .into_iter()
        .find(|feature| is_kind(feature, BuffActKind::AttrOnlyCalDamageReplaceAttr))?;
    feature.owner_uid = request.source_uid;
    feature.source_uid = request.source_uid;
    let replacement =
        crate::engine::skill::buff_act::attack_replacement_rule(&feature, runtime.hp)?;
    let source = runtime.pool.entity(request.source_uid)?;
    let target = runtime.pool.entity(request.target_uid)?;
    let (base_rate, added_rate) = composed_damage_rates(request.rate, request.rate_terms);
    let amount = direct_damage(
        request,
        runtime,
        source,
        target,
        DirectOptions {
            base_rate,
            added_rate,
            formula: replacement.formula,
            attack_replacement: Some(replacement),
        },
    );
    (amount > 0).then_some(HpCommand::Damage(HpDamage {
        origin,
        source_uid: request.source_uid,
        target_uid: request.target_uid,
        amount,
        config_effect,
        effect_kind: if request.is_crit {
            DamageEffectKind::Critical
        } else {
            DamageEffectKind::Normal
        },
        assassinate: false,
        ignore_riposte: false,
        hurt: HurtInfoData {
            from_uid: request.source_uid,
            is_crit: request.is_crit,
            career_restraint: restrains_target(
                request.attack_career.unwrap_or(source.career),
                target,
            ),
            reduce_hp: 0,
            effect_id: request.skill_id,
            skill_id: request.skill_id,
            damage_from: HurtDamageFromType::SkillEffect,
            buff_act_id: request.skill_id,
            buff_uid: 0,
            hurt_effect_type,
            display_amount: None,
        },
    }))
}

struct ResolvedRowDamage {
    source_uid: i64,
    target_uid: i64,
    amount: i32,
    hurt: HurtInfoData,
}

fn resolve_row_damage_result(
    request: DamageRequest<'_>,
    runtime: DamageRuntime<'_>,
) -> Option<ResolvedRowDamage> {
    let DamageRequest {
        source_uid,
        target_uid,
        skill_id,
        rate,
        rate_terms,
        is_crit,
        ..
    } = request;
    let DamageRuntime {
        pool, buffs, hp, ..
    } = runtime;
    let source = pool.entity(source_uid)?;
    let target = pool.entity(target_uid)?;
    let attack_replacement = buffs
        .active_features(hp)
        .into_iter()
        .filter(|feature| feature.owner_uid == source_uid)
        .find_map(|feature| {
            crate::engine::skill::buff_act::direct_attack_replacement_rule(&feature, hp)
                .map(|replacement| (feature.buff_id, replacement))
        });
    if crate::engine::damage::trace::enabled()
        && let Some((buff_id, replacement)) = attack_replacement
    {
        eprintln!(
            "damage replacement skill={skill_id} source={source_uid} buff={buff_id} value={replacement:?}"
        );
    }
    let attack_replacement = attack_replacement.map(|(_, replacement)| replacement);
    let formula = attack_replacement
        .map(|replacement| replacement.formula)
        .unwrap_or(DamageFormula::StandardSkill);
    let (base_rate, added_rate) = composed_damage_rates(rate, rate_terms);
    let amount = direct_damage(
        request,
        runtime,
        source,
        target,
        DirectOptions {
            base_rate,
            added_rate,
            formula,
            attack_replacement,
        },
    );
    let career_restraint = restrains_target(request.attack_career.unwrap_or(source.career), target);
    (amount > 0).then_some(ResolvedRowDamage {
        source_uid,
        target_uid,
        amount,
        hurt: HurtInfoData {
            from_uid: source_uid,
            is_crit,
            career_restraint,
            reduce_hp: 0,
            effect_id: skill_id,
            skill_id,
            damage_from: HurtDamageFromType::Skill,
            buff_act_id: 0,
            buff_uid: 0,
            hurt_effect_type: if is_crit {
                EffectType::Crit as i32
            } else {
                EffectType::Damage as i32
            },
            display_amount: None,
        },
    })
}

pub fn resolve_additional_damage_command(
    request: DamageRequest<'_>,
    runtime: DamageRuntime<'_>,
    attack_replacement: Option<crate::engine::skill::buff_act::AttackReplacement>,
    credited_source_uid: i64,
    assassinate: bool,
    origin: CommandOrigin,
) -> Option<HpCommand> {
    let resolved = resolve_additional_damage_result(
        request,
        runtime,
        attack_replacement,
        credited_source_uid,
    )?;
    Some(HpCommand::Damage(HpDamage {
        origin,
        source_uid: resolved.source_uid,
        target_uid: resolved.target_uid,
        amount: resolved.amount,
        config_effect: -1,
        effect_kind: if resolved.hurt.is_crit {
            DamageEffectKind::Critical
        } else {
            DamageEffectKind::Normal
        },
        assassinate,
        ignore_riposte: false,
        hurt: resolved.hurt,
    }))
}

struct ResolvedAdditionalDamage {
    source_uid: i64,
    target_uid: i64,
    amount: i32,
    hurt: HurtInfoData,
}

fn resolve_additional_damage_result(
    request: DamageRequest<'_>,
    runtime: DamageRuntime<'_>,
    attack_replacement: Option<crate::engine::skill::buff_act::AttackReplacement>,
    credited_source_uid: i64,
) -> Option<ResolvedAdditionalDamage> {
    let DamageRequest {
        target_uid,
        is_crit,
        ..
    } = request;
    if attack_replacement.is_some_and(|replacement| {
        !matches!(
            replacement.formula,
            crate::engine::damage::DamageFormula::AdditionalDamage
                | crate::engine::damage::DamageFormula::AttributeReplacementAdditional
                | crate::engine::damage::DamageFormula::MaxHpAdditionalDamage
        )
    }) {
        return None;
    }
    let amount = additional_replaced_attack_damage_amount(request, runtime, attack_replacement);
    (amount > 0).then_some(ResolvedAdditionalDamage {
        source_uid: credited_source_uid,
        target_uid,
        amount,
        hurt: HurtInfoData {
            from_uid: credited_source_uid,
            is_crit,
            career_restraint: false,
            reduce_hp: 0,
            effect_id: 0,
            skill_id: 0,
            damage_from: HurtDamageFromType::Additional,
            buff_act_id: 0,
            buff_uid: 0,
            hurt_effect_type: if is_crit {
                EffectType::Additionaldamagecrit as i32
            } else {
                EffectType::Additionaldamage as i32
            },
            display_amount: None,
        },
    })
}

fn additional_replaced_attack_damage_amount(
    request: DamageRequest<'_>,
    runtime: DamageRuntime<'_>,
    attack_replacement: Option<crate::engine::skill::buff_act::AttackReplacement>,
) -> i32 {
    let DamageRequest {
        source_uid,
        target_uid,
        ..
    } = request;
    let DamageRuntime { pool, .. } = runtime;
    let Some(source) = pool.entity(source_uid) else {
        return 0;
    };
    let Some(target) = pool.entity(target_uid) else {
        return 0;
    };
    let (base_rate, added_rate) = composed_damage_rates(request.rate, request.rate_terms);
    let formula = attack_replacement
        .map(|replacement| replacement.formula)
        .unwrap_or(DamageFormula::AdditionalDamage);
    direct_damage(
        request,
        runtime,
        source,
        target,
        DirectOptions {
            base_rate,
            added_rate,
            formula,
            attack_replacement,
        },
    )
}

pub(super) fn composed_damage_rates(base_rate: i32, terms: &[DamageRateTerm]) -> (i32, i32) {
    use crate::engine::damage::DamageRateComposition;

    let retribution_bonus = terms
        .iter()
        .filter(|term| term.composition == DamageRateComposition::RetributionLane)
        .map(|term| term.rate)
        .sum::<i32>();
    let mut base_rate = if retribution_bonus != 0 {
        base_rate
            .saturating_add(1000)
            .saturating_add(retribution_bonus)
    } else {
        base_rate
    };
    let producer_multiplier = terms
        .iter()
        .filter(|term| term.composition == DamageRateComposition::ProducerMultiplier)
        .map(|term| term.rate)
        .sum::<i32>();
    if producer_multiplier != 0 {
        base_rate = base_rate.saturating_mul(1000 + producer_multiplier) / 1000;
    }
    let term_rate = |career_scaled| {
        terms
            .iter()
            .filter(|term| term.career_scaled == career_scaled)
            .filter(|term| term.composition == DamageRateComposition::Additive)
            .map(|term| term.rate)
            .sum::<i32>()
    };
    (base_rate + term_rate(true), term_rate(false))
}

pub(super) struct DirectOptions {
    pub(super) base_rate: i32,
    pub(super) added_rate: i32,
    pub(super) formula: DamageFormula,
    pub(super) attack_replacement: Option<crate::engine::skill::buff_act::AttackReplacement>,
}

pub(super) fn direct_damage(
    request: DamageRequest<'_>,
    runtime: DamageRuntime<'_>,
    source: &TargetEntity,
    target: &TargetEntity,
    options: DirectOptions,
) -> i32 {
    let DamageRequest {
        skill_id,
        rate_terms,
        attack_attributes,
        career_ratio_bonus,
        is_conduit,
        is_crit,
        extra_skill_kind,
        ..
    } = request;
    let DamageRuntime {
        attributes,
        buffs,
        target_buffs,
        hp,
        fields,
        emitter,
        team_inspiration,
        ..
    } = runtime;
    let DirectOptions {
        base_rate,
        added_rate,
        formula,
        attack_replacement,
    } = options;
    let formula_rules = formula.rules();
    let source_active_features = buffs.active_features(hp);
    let target_active_features = target_buffs.active_features(hp);
    let include_trigger_history = formula_rules.includes_trigger_history;
    let attribute_delta = |entity: &TargetEntity, attr_id: AttrId| -> i32 {
        let (entity_buffs, active_features) = if entity.uid == target.uid {
            (target_buffs, &target_active_features)
        } else {
            (buffs, &source_active_features)
        };
        let emitter_attribute = if entity.uid == crate::engine::manager::emitter::UID {
            crate::engine::manager::emitter::average_ally_buff_attribute(
                runtime.pool,
                buffs,
                hp,
                attr_id,
            ) + emitter
                .map(|emitter| emitter.ally_attribute(attr_id))
                .unwrap_or_default()
        } else {
            0
        };
        let components = [
            ("buff", entity_buffs.attribute_delta(entity.uid, attr_id)),
            (
                "from_entity",
                crate::engine::skill::buff_act::attr_from_entity::active_feature_delta(
                active_features,
                entity.uid,
                attr_id,
                attributes,
                entity_buffs,
                hp,
                ),
            ),
            (
                "hero_id",
                active_features
                    .iter()
                    .filter(|feature| feature.owner_uid == entity.uid)
                    .map(|feature| {
                        crate::engine::skill::buff_act::attr_by_hero_id::attribute_delta(
                            feature,
                            entity.model_id,
                            attr_id,
                        )
                    })
                    .sum::<i32>(),
            ),
            (
                "damage_type",
                active_features
                    .iter()
                    .filter(|feature| feature.owner_uid == entity.uid)
                    .map(|feature| {
                        crate::engine::skill::buff_act::attr_by_damage_type::attribute_delta(
                            feature,
                            entity.damage_type,
                            attr_id,
                        )
                    })
                    .sum::<i32>(),
            ),
            ("emitter", emitter_attribute),
            (
                "circle",
                fields
                    .map(|fields| fields.attribute_delta(entity.uid, attr_id, runtime.pool))
                    .unwrap_or_default(),
            ),
            (
                "other_layer",
                active_features
                    .iter()
                    .filter(|feature| feature.owner_uid == entity.uid)
                    .filter(|feature| is_kind(feature, BuffActKind::AddAttrByOtherBuffLayer))
                    .map(|feature| {
                        crate::engine::skill::buff_act::add_attr_by_other_buff_layer::attribute_delta(
                            feature,
                            attr_id,
                            entity_buffs,
                        )
                    })
                    .sum::<i32>(),
            ),
            (
                "sub_layer",
                active_features
                    .iter()
                    .filter(|feature| feature.owner_uid == entity.uid)
                    .filter(|feature| is_kind(feature, BuffActKind::FixAttrBySubBuffLayer))
                    .map(|feature| {
                        crate::engine::skill::buff_act::fix_attr_by_sub_buff_layer::attribute_delta(
                            feature,
                            attr_id,
                            entity_buffs,
                        )
                    })
                    .sum::<i32>(),
            ),
            (
                "team_energy",
                active_features
                    .iter()
                    .filter(|feature| feature.owner_uid == entity.uid)
                    .map(|feature| {
                        crate::engine::skill::buff_act::fix_attr_team_energy::attribute_delta(
                            feature,
                            attr_id,
                            team_inspiration,
                            entity_buffs,
                            runtime.pool,
                        )
                    })
                    .sum::<i32>(),
            ),
            (
                "dynamic",
                active_features
                    .iter()
                    .filter(|feature| feature.owner_uid == entity.uid)
                    .map(|feature| {
                        crate::engine::skill::buff_act::dynamic_attribute_delta(
                            feature,
                            attr_id,
                            entity_buffs,
                            hp,
                            include_trigger_history,
                        )
                    })
                    .sum::<i32>(),
            ),
            (
                "raspberry",
                crate::engine::skill::buff_act::raspberry::attribute_delta(
                    entity_buffs,
                    entity.uid,
                    attr_id,
                ),
            ),
        ];
        if matches!(
            attr_id,
            AttrId::DmgBonus | AttrId::CriticalDmg | AttrId::CriticalDef
        ) && crate::engine::damage::trace::enabled()
        {
            eprintln!(
                "damage attributes source={} attr={attr_id:?} components={:?}",
                entity.uid,
                components
                    .iter()
                    .copied()
                    .filter(|(_, delta)| *delta != 0)
                    .collect::<Vec<_>>()
            );
        }
        components.into_iter().map(|(_, delta)| delta).sum()
    };
    let attack_attribute_delta = |attr_id| {
        attack_attributes
            .iter()
            .filter_map(|(actual, delta)| (*actual == attr_id).then_some(*delta))
            .sum::<i32>()
    };
    let is_ultimate = crate::engine::skill::effect::catalog::configured_is_big_skill(skill_id);
    let extra_action =
        crate::engine::skill::condition::extra::skill_kind_from_is_extra(extra_skill_kind)
            .is_some_and(|kind| kind.is_extra_action());
    let attack_local_attribute = |attr_id| {
        source_active_features
            .iter()
            .filter(|feature| feature.owner_uid == source.uid)
            .map(|feature| {
                crate::engine::skill::buff_act::calculated_attack_attribute_delta_for_skill(
                    feature,
                    attr_id,
                    attributes,
                    buffs,
                    hp,
                    is_ultimate,
                    extra_action,
                )
            })
            .sum::<i32>()
    };
    let flat_attack = source_active_features
        .iter()
        .filter(|feature| feature.owner_uid == source.uid)
        .map(|feature| {
            crate::engine::skill::buff_act::injury_bank::snapshotted_attribute_delta(
                feature,
                AttrId::Attack,
                buffs,
            )
        })
        .sum::<i32>();
    let base_attack = attributes.base(source.uid, AttrId::Attack);
    let attack_rate = 1000
        + attributes.get(source.uid, AttrId::Attack)
        + attribute_delta(source, AttrId::Attack)
        + attack_attribute_delta(AttrId::Attack)
        + attack_local_attribute(AttrId::Attack);
    let attack_input_replacement = attack_replacement.map(|replacement| replacement.amount);
    let final_atk =
        attack_input_replacement.unwrap_or_else(|| base_attack * attack_rate / 1000 + flat_attack);
    let final_atk = if final_atk <= 0 {
        base_attack / 10
    } else {
        final_atk
    };
    let minimum = (final_atk / 10).max(1);
    let defense_attr =
        if source.damage_type == crate::engine::skill::target::EntityDamageType::Mental {
            AttrId::MentalDef
        } else {
            AttrId::RealityDef
        };
    let target_defense = attributes.base(target.uid, defense_attr);
    let defense_multiplier =
        (1000 + attributes.get(target.uid, defense_attr) + attribute_delta(target, defense_attr))
            .max(0);
    let penetration = (attributes.get(source.uid, AttrId::Penetration)
        + attribute_delta(source, AttrId::Penetration)
        + attack_local_attribute(AttrId::Penetration)
        + attack_attributes
            .iter()
            .filter_map(|(attr_id, delta)| (*attr_id == AttrId::Penetration).then_some(*delta))
            .sum::<i32>())
    .clamp(0, 1000);
    let forced_career = source_active_features
        .iter()
        .filter(|feature| feature.owner_uid == source.uid)
        .any(crate::engine::skill::buff_act::forces_career_restraint);
    let source_career = if let Some(career) = request.attack_career {
        career
    } else if source.uid == crate::engine::manager::emitter::UID {
        crate::engine::skill::buff_act::emitter_career::career(&source_active_features)
            .unwrap_or(source.career)
    } else {
        source.career
    };
    let natural_career = career_multiplier_against(source_career, target);
    let career = if formula_rules.applies_career && (natural_career > 1000 || forced_career) {
        (if natural_career > 1000 {
            natural_career
        } else {
            strongest_career_multiplier(source_career)
        }) + source_active_features
            .iter()
            .filter(|feature| feature.owner_uid == source.uid)
            .map(crate::engine::skill::buff_act::career_ratio_bonus)
            .sum::<i32>()
            + career_ratio_bonus
    } else {
        1000
    };

    let source_base_regular = attributes.get(source.uid, AttrId::DmgBonus);
    let source_buff_regular = attribute_delta(source, AttrId::DmgBonus);
    let source_attack_regular = attack_attribute_delta(AttrId::DmgBonus);
    let source_regular = source_base_regular
        + source_buff_regular
        + source_attack_regular
        + attack_local_attribute(AttrId::DmgBonus);
    let attack_attr = |attr_id| {
        attack_attributes
            .iter()
            .filter_map(|(actual, delta)| (*actual == attr_id).then_some(*delta))
            .sum::<i32>()
    };
    let ultimate_might = attributes.get(source.uid, AttrId::UltimateMight)
        + attribute_delta(source, AttrId::UltimateMight)
        + attack_attr(AttrId::UltimateMight)
        + attack_local_attribute(AttrId::UltimateMight);
    let conduit_might = if is_conduit {
        attributes.get(source.uid, AttrId::ConduitMight)
            + attribute_delta(source, AttrId::ConduitMight)
            + attack_attr(AttrId::ConduitMight)
            + attack_local_attribute(AttrId::ConduitMight)
    } else {
        0
    };
    let might = conduit_might
        + if is_ultimate {
            1000 + ultimate_might
        } else {
            let incantation_might = attributes.get(source.uid, AttrId::IncantationMight)
                + attribute_delta(source, AttrId::IncantationMight)
                + attack_attr(AttrId::IncantationMight)
                + attack_local_attribute(AttrId::IncantationMight);
            let cross_multiplier = attributes
                .get(source.uid, AttrId::IncantationSkillUltMightMultiplier)
                + attribute_delta(source, AttrId::IncantationSkillUltMightMultiplier)
                + attack_attr(AttrId::IncantationSkillUltMightMultiplier)
                + attack_local_attribute(AttrId::IncantationSkillUltMightMultiplier);
            1000 + incantation_might + ultimate_might * cross_multiplier / 1000
        };
    let action_attr =
        match crate::engine::skill::condition::extra::skill_kind_from_is_extra(extra_skill_kind) {
            Some(crate::engine::skill::condition::extra::ExtraSkillKind::ExtraAction) => {
                Some(AttrId::ExtraDmg)
            }
            Some(crate::engine::skill::condition::extra::ExtraSkillKind::FollowUp) => {
                Some(AttrId::ReuseDmg)
            }
            Some(crate::engine::skill::condition::extra::ExtraSkillKind::Riposte) => {
                Some(AttrId::ReboundDmg)
            }
            _ => None,
        };
    let action_bonus = action_attr.map_or(0, |attr_id| {
        attributes.get(source.uid, attr_id)
            + attribute_delta(source, attr_id)
            + attack_attr(attr_id)
            + attack_local_attribute(attr_id)
    });
    let action_joins_might = formula_rules.combines_action_with_might;
    let target_base_regular = attributes.get(target.uid, AttrId::DmgTakenReduction);
    let target_buff_regular = attribute_delta(target, AttrId::DmgTakenReduction);
    let target_regular = target_base_regular + target_buff_regular;
    let playmode_rate = attributes.get(source.uid, AttrId::PlaymodeDmgIncrease)
        + attribute_delta(source, AttrId::PlaymodeDmgIncrease)
        + attack_attribute_delta(AttrId::PlaymodeDmgIncrease)
        + attack_local_attribute(AttrId::PlaymodeDmgIncrease)
        - attributes.get(target.uid, AttrId::PlaymodeDmgImmunity)
        - attribute_delta(target, AttrId::PlaymodeDmgImmunity)
        - attack_attribute_delta(AttrId::PlaymodeDmgImmunity);
    let final_delta = attribute_delta(source, AttrId::FinalDmgBonus)
        + attack_attribute_delta(AttrId::FinalDmgBonus)
        - attribute_delta(target, AttrId::FinalDmgReduction)
        + playmode_rate;
    let (regular_final_delta, separate_final_delta) = formula.split_final_damage(final_delta);
    let regular = regular_multiplier(
        source_regular + if action_joins_might { 0 } else { action_bonus } + regular_final_delta,
        target_regular,
    );
    let attack_local = 1000
        + crate::engine::skill::buff_act::electric_transform::team_damage_multiplier_delta(
            buffs, hp, source.uid,
        );
    let action = 1000 + if action_joins_might { action_bonus } else { 0 };
    let (might, action) = formula.compose_might_and_action(might, action);
    let final_rate = (1000 + separate_final_delta).max(300);

    let source_crit = attributes.get(source.uid, AttrId::CriticalDmg);
    let technique_crit = critical_technique_bonus(source, target.level, 12);
    let buff_crit = attribute_delta(source, AttrId::CriticalDmg)
        + attack_attribute_delta(AttrId::CriticalDmg)
        + attack_local_attribute(AttrId::CriticalDmg);
    let target_base_crit = attributes.get(target.uid, AttrId::CriticalDef);
    let target_buff_crit = attribute_delta(target, AttrId::CriticalDef);
    let target_crit = if formula_rules.applies_target_base_critical_defense {
        target_base_crit
    } else {
        0
    } + if formula_rules.applies_target_buff_critical_defense {
        target_buff_crit
    } else {
        0
    };
    let crit_multiplier = (source_crit + technique_crit + buff_crit - target_crit).max(0);
    let inputs = DamageInputs {
        kind: if source.damage_type == crate::engine::skill::target::EntityDamageType::Mental {
            DamageKind::Mental
        } else {
            DamageKind::Reality
        },
        attack: final_atk,
        defense: target_defense,
        defense_multiplier,
        penetration,
        minimum,
        base_rate,
        added_rate,
        career,
        regular,
        attack_local,
        might,
        action,
        genesis: 1000,
        final_rate,
        poison_multiplier: None,
        crit_multiplier,
        is_crit,
    };
    if crate::engine::damage::trace::enabled()
        && (!rate_terms.is_empty() || !attack_attributes.is_empty())
    {
        eprintln!(
            "damage rate terms skill={skill_id} terms={rate_terms:?} attack_attributes={attack_attributes:?}"
        );
    }
    let damage_trace = calculate_damage_with_trace(inputs, runtime.fight_version);
    let amount = damage_trace.amount;
    if crate::engine::damage::trace::enabled() {
        let local_damage_bonus = source_active_features
            .iter()
            .filter(|feature| feature.owner_uid == source.uid)
            .filter_map(|feature| {
                let delta =
                    crate::engine::skill::buff_act::calculated_attack_attribute_delta_for_skill(
                        feature,
                        AttrId::DmgBonus,
                        attributes,
                        buffs,
                        hp,
                        is_ultimate,
                        extra_action,
                    );
                (delta != 0).then_some((feature.buff_id, feature.act_type.as_str(), delta))
            })
            .collect::<Vec<_>>();
        let source_buffs = buffs
            .active_for(source.uid)
            .map(|buff| {
                (
                    buff.buff_id.unwrap_or_default(),
                    buff.layer.unwrap_or_default(),
                    buff.count.unwrap_or_default(),
                    buff.act_common_params.as_deref().unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        let target_buffs = buffs
            .active_for(target.uid)
            .map(|buff| {
                (
                    buff.buff_id.unwrap_or_default(),
                    buff.layer.unwrap_or_default(),
                    buff.count.unwrap_or_default(),
                    buff.act_common_params.as_deref().unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        let emitter_ally_inputs = (source.uid == crate::engine::manager::emitter::UID).then(|| {
            runtime
                .pool
                .main_allies(source.uid)
                .iter()
                .filter(|ally| hp.current(ally.uid) > 0)
                .map(|ally| {
                    let stored_rate = attributes.get(ally.uid, AttrId::Attack);
                    let dynamic_rate = attribute_delta(ally, AttrId::Attack);
                    format!(
                        "uid={} attack={}/{stored_rate}/{dynamic_rate} crit={}/{}/{} damage={}/{} might={}/{}/{}/{} penetration={}/{} final={}/{} playmode={}/{}",
                        ally.uid,
                        attributes.base(ally.uid, AttrId::Attack),
                        attributes.get(ally.uid, AttrId::CriticalDmg),
                        ally.technic,
                        attribute_delta(ally, AttrId::CriticalDmg),
                        attributes.get(ally.uid, AttrId::DmgBonus),
                        attribute_delta(ally, AttrId::DmgBonus),
                        attributes.get(ally.uid, AttrId::IncantationMight),
                        attribute_delta(ally, AttrId::IncantationMight),
                        attributes.get(ally.uid, AttrId::UltimateMight),
                        attribute_delta(ally, AttrId::UltimateMight),
                        attributes.get(ally.uid, AttrId::Penetration),
                        attribute_delta(ally, AttrId::Penetration),
                        attributes.get(ally.uid, AttrId::FinalDmgBonus),
                        attribute_delta(ally, AttrId::FinalDmgBonus),
                        attributes.get(ally.uid, AttrId::PlaymodeDmgIncrease),
                        attribute_delta(ally, AttrId::PlaymodeDmgIncrease),
                    )
                })
                .collect::<Vec<_>>()
        });
        eprintln!(
            "damage formula={formula:?} skill={skill_id} source={} target={} source_hp={}/{} target_hp={}/{} shield={} attack={final_atk}({base_attack}*{attack_rate}/1000+{flat_attack}) emitter_ally_inputs={emitter_ally_inputs:?} source_buffs={source_buffs:?} target_buffs={target_buffs:?} penetration={penetration} regular={source_regular}({source_base_regular}+{source_buff_regular}+{source_attack_regular}+{})+action({action_bonus}, {})-{target_regular}({target_base_regular}+{target_buff_regular}) local_bonus={local_damage_bonus:?} ultimate_might={}+{}+{} extra={} rebound={} reuse={} hp_pct={} atk_pct={} crit={source_crit}+{technique_crit}+{buff_crit}-({target_base_crit}+{target_buff_crit}) trace={damage_trace:?}",
            source.uid,
            target.uid,
            hp.current(source.uid),
            hp.max(source.uid),
            hp.current(target.uid),
            hp.max(target.uid),
            hp.shield(source.uid),
            attack_local_attribute(AttrId::DmgBonus),
            if action_joins_might {
                "might"
            } else {
                "regular"
            },
            attributes.get(source.uid, AttrId::UltimateMight),
            attribute_delta(source, AttrId::UltimateMight),
            attack_attr(AttrId::UltimateMight),
            attributes.get(source.uid, AttrId::ExtraDmg),
            attributes.get(source.uid, AttrId::ReboundDmg),
            attributes.get(source.uid, AttrId::ReuseDmg),
            attributes.get(source.uid, AttrId::HpPercent),
            attributes.get(source.uid, AttrId::AttackPercent),
        );
    }
    amount
}
