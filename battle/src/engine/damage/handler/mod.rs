use crate::engine::{
    damage::butterfly_damage,
    entity::attr::AttrId,
    manager::{
        buff::BuffManager,
        hp::{
            DamageEffectKind, HpCommand, HpDamage, HpHeal, HpHealKind, HpLoss, HurtDamageFromType,
            HurtInfoData,
        },
    },
    skill::{
        behavior::{BehaviorOpContext, classify::BehaviorKind, registry::BehaviorHandler},
        buff_act::{is_kind, registry::BuffActKind},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
    },
};
use sonettobuf::effect_type_enum::EffectType;

mod affinity;
mod critical;
mod heal;
mod loss;
mod origin;
mod resolve;

use affinity::{critical_technique_bonus, regular_multiplier, strongest_career_multiplier};
pub(crate) use affinity::{restrains, restrains_target};
pub(crate) use critical::{
    chance as crit_chance, damage_multiplier as crit_damage_multiplier,
    excess_rate as excess_crit_rate,
};
pub(crate) use heal::modified as modified_heal;
pub(crate) use resolve::{
    DamageRequest, DamageRuntime, resolve_additional_damage_command, resolve_attack_command,
    resolve_avoided_attack_command, resolve_configured_replacement_damage_command,
};
use resolve::{DirectOptions, direct_damage};

#[cfg(test)]
use affinity::technique_bonus;
#[cfg(test)]
use resolve::composed_damage_rates;

#[cfg(test)]
use crate::engine::damage::calculate as calculate_damage;

pub(crate) struct Handler;

pub fn supports_origin_damage(behavior: &ParsedBehavior) -> bool {
    matches!(
        behavior.args.as_slice(),
        [0 | 1, raw_attr, rate] if AttrId::from_raw(*raw_attr).is_some() && *rate >= 0
    )
}

pub fn supports_attribute_damage(behavior: &ParsedBehavior) -> bool {
    matches!(
        behavior.args.as_slice(),
        [1, raw_attr, 100] if AttrId::from_raw(*raw_attr) == Some(AttrId::Hp)
    )
}

pub fn supports_bloodlust(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [rate] if *rate > 0)
}

impl BehaviorHandler for Handler {
    fn references(behavior: &ParsedBehavior) -> crate::engine::skill::rule::RuleReferences {
        crate::engine::skill::rule::RuleReferences {
            skills: Vec::new(),
            buffs: matches!(
                (behavior.spec.key.opcode, behavior.spec.kind),
                (60216, BehaviorKind::DamageRealLostLife)
            )
            .then(|| behavior.arg(0))
            .flatten()
            .filter(|buff_id| *buff_id > 0)
            .into_iter()
            .collect(),
            models: Vec::new(),
        }
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let origin = crate::engine::skill::behavior::command_origin(behavior)?;
        let source_uid = context.source_uid;
        let target_uid = context.target_uid;
        let extra_action = crate::engine::skill::condition::extra::skill_kind_from_is_extra(
            context.target.extra_skill_kind,
        )
        .is_some_and(|kind| kind.is_extra_action());
        let force_critical = || {
            if !extra_action {
                return false;
            }
            let features = context.managers.buff.active_features(&context.managers.hp);
            crate::engine::skill::buff_act::must_crit_and_fix_temp_attr::forces_critical(
                &features,
                source_uid,
                extra_action,
            )
        };
        let heal = |amount, kind| {
            if amount <= 0 {
                Vec::new()
            } else {
                vec![RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(
                    HpHeal {
                        origin,
                        source_uid,
                        target_uid,
                        amount,
                        config_effect: behavior.config_effect,
                        kind,
                    },
                )))]
            }
        };
        match (behavior.spec.key.opcode, behavior.spec.kind) {
            (20001 | 90001, BehaviorKind::Heal) => {
                let is_crit = context.determinism.roll_hidden_crit(
                    context.active_skill_id,
                    source_uid,
                    target_uid,
                    crit_chance(source_uid, target_uid, context.pool, context.managers),
                );
                return heal::amount(source_uid, target_uid, context.managers, is_crit, behavior)
                    .map(|amount| {
                        heal(
                            amount,
                            if is_crit {
                                HpHealKind::Critical
                            } else {
                                HpHealKind::Normal
                            },
                        )
                    });
            }
            (20016, BehaviorKind::HealCantCrit) => {
                return heal::attribute_amount(
                    context.source_uid,
                    context.target_uid,
                    context.managers,
                    behavior,
                )
                .map(|amount| heal(amount, HpHealKind::Normal));
            }
            (60232, BehaviorKind::HealByTwoAttr) => {
                return heal::two_attribute_amount(
                    context.source_uid,
                    context.target_uid,
                    context.managers,
                    behavior,
                )
                .map(|amount| heal(amount, HpHealKind::Normal));
            }
            (20010, BehaviorKind::Bloodlust) => {
                let [rate] = behavior.args.as_slice() else {
                    return None;
                };
                let leech_rate = rate
                    .saturating_add(
                        context
                            .managers
                            .attribute
                            .get(source_uid, AttrId::LeechRate),
                    )
                    .saturating_add(
                        context
                            .managers
                            .buff
                            .attribute_delta(source_uid, AttrId::LeechRate),
                    );
                let efficacy = 1000_i32
                    .saturating_sub(
                        context
                            .managers
                            .buff
                            .buff_act_scalar(source_uid, BuffActKind::InjuryAbsorb),
                    )
                    .max(0);
                return Some(heal(
                    crate::engine::damage::scale_permille(
                        crate::engine::damage::scale_permille(
                            context.target.action_damage_amount,
                            leech_rate,
                        ),
                        efficacy,
                    ),
                    HpHealKind::Bloodlust,
                ));
            }
            (30014, BehaviorKind::OriginDamage) | (30015, BehaviorKind::OriginDamageCanCrit) => {
                let is_crit = behavior.spec.kind == BehaviorKind::OriginDamageCanCrit
                    && (force_critical()
                        || context.determinism.roll_hidden_crit(
                            context.active_skill_id,
                            source_uid,
                            target_uid,
                            crit_chance(source_uid, target_uid, context.pool, context.managers),
                        ));
                let amount = origin::amount(
                    source_uid,
                    target_uid,
                    origin::OriginRuntime {
                        managers: context.managers,
                        pool: context.pool,
                        extra_action,
                    },
                    &context.modifiers.attack_attributes,
                    is_crit,
                    behavior,
                )?;
                if amount <= 0 {
                    return Some(Vec::new());
                }
                let hurt = HurtInfoData {
                    from_uid: source_uid,
                    is_crit,
                    career_restraint: false,
                    reduce_hp: 0,
                    effect_id: context.active_skill_id,
                    skill_id: context.target.active_skill_id,
                    damage_from: HurtDamageFromType::SkillEffect,
                    buff_act_id: 0,
                    buff_uid: 0,
                    hurt_effect_type: if is_crit {
                        EffectType::Origincrit as i32
                    } else {
                        EffectType::Origindamage as i32
                    },
                    display_amount: None,
                };
                let command = match behavior.spec.kind {
                    BehaviorKind::OriginDamage => HpCommand::Damage(HpDamage {
                        origin,
                        source_uid,
                        target_uid,
                        amount,
                        config_effect: behavior.config_effect,
                        effect_kind: DamageEffectKind::Genesis,
                        assassinate: false,
                        ignore_riposte: false,
                        hurt,
                    }),
                    BehaviorKind::OriginDamageCanCrit => HpCommand::Lose(HpLoss {
                        origin,
                        source_uid,
                        target_uid,
                        amount,
                        config_effect: behavior.config_effect,
                        hurt: Some(hurt),
                    }),
                    _ => unreachable!("exact origin-damage registry branch"),
                };
                return Some(vec![RuleOp::Command(BattleCommand::Hp(command))]);
            }
            (60127, BehaviorKind::OriginDamageByAttrAndBuffGroupSize) => {
                let is_crit = force_critical()
                    || context.determinism.roll_hidden_crit(
                        context.active_skill_id,
                        source_uid,
                        target_uid,
                        crit_chance(source_uid, target_uid, context.pool, context.managers),
                    );
                let amount = origin::buff_group_amount(
                    source_uid,
                    target_uid,
                    origin::OriginRuntime {
                        managers: context.managers,
                        pool: context.pool,
                        extra_action,
                    },
                    &context.modifiers.attack_attributes,
                    is_crit,
                    behavior,
                )?;
                if amount <= 0 {
                    return Some(Vec::new());
                }
                return Some(vec![RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(
                    HpLoss {
                        origin,
                        source_uid,
                        target_uid,
                        amount,
                        config_effect: behavior.config_effect,
                        hurt: Some(HurtInfoData {
                            from_uid: source_uid,
                            is_crit,
                            career_restraint: false,
                            reduce_hp: 0,
                            effect_id: context.active_skill_id,
                            skill_id: context.target.active_skill_id,
                            damage_from: HurtDamageFromType::SkillEffect,
                            buff_act_id: 0,
                            buff_uid: 0,
                            hurt_effect_type: if is_crit {
                                EffectType::Origincrit as i32
                            } else {
                                EffectType::Origindamage as i32
                            },
                            display_amount: None,
                        }),
                    },
                )))]);
            }
            (60146, BehaviorKind::OriginDamageByTeamAttr) => {
                let amount = origin::team_attribute_amount(
                    source_uid,
                    target_uid,
                    origin::OriginRuntime {
                        managers: context.managers,
                        pool: context.pool,
                        extra_action,
                    },
                    behavior,
                )?;
                if amount <= 0 {
                    return Some(Vec::new());
                }
                return Some(vec![RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(
                    HpLoss {
                        origin,
                        source_uid,
                        target_uid,
                        amount,
                        config_effect: behavior.config_effect,
                        hurt: Some(HurtInfoData {
                            from_uid: source_uid,
                            is_crit: false,
                            career_restraint: false,
                            reduce_hp: 0,
                            effect_id: context.active_skill_id,
                            skill_id: context.target.active_skill_id,
                            damage_from: HurtDamageFromType::SkillEffect,
                            buff_act_id: 0,
                            buff_uid: 0,
                            hurt_effect_type: EffectType::Origindamage as i32,
                            display_amount: Some(amount),
                        }),
                    },
                )))]);
            }
            (60282, BehaviorKind::ButterflyDamage) => {
                let [base_rate, raw_step, rate_per_step, raw_cap] = behavior.args.as_slice() else {
                    return None;
                };
                if *raw_step <= 0 || *raw_cap < 0 {
                    return None;
                }
                let gauge_raw = context
                    .managers
                    .gauge
                    .raw_value(crate::engine::mechanic::lingering_glow::key(
                        context.source_team,
                    ))
                    .unwrap_or_default();
                let allied_sources = context
                    .pool
                    .allies(source_uid)
                    .iter()
                    .map(|entity| entity.uid)
                    .collect::<Vec<_>>();
                let recorded_damage = context
                    .managers
                    .hp
                    .skill_damage_from_sources(target_uid, &allied_sources);
                let amount = butterfly_damage(
                    recorded_damage,
                    *base_rate,
                    gauge_raw,
                    *raw_step,
                    *rate_per_step,
                    *raw_cap,
                );
                if amount <= 0 {
                    return Some(Vec::new());
                }
                return Some(vec![RuleOp::Command(BattleCommand::Hp(HpCommand::Damage(
                    HpDamage {
                        origin,
                        source_uid,
                        target_uid,
                        amount,
                        config_effect: behavior.config_effect,
                        effect_kind: DamageEffectKind::Genesis,
                        assassinate: false,
                        ignore_riposte: false,
                        hurt: HurtInfoData {
                            from_uid: source_uid,
                            is_crit: false,
                            career_restraint: false,
                            reduce_hp: 0,
                            effect_id: context.active_skill_id,
                            skill_id: context.active_skill_id,
                            damage_from: HurtDamageFromType::SkillEffect,
                            buff_act_id: 0,
                            buff_uid: 0,
                            hurt_effect_type: EffectType::Origindamage as i32,
                            display_amount: Some(amount),
                        },
                    },
                )))]);
            }
            (30005 | 30006 | 30018 | 60288, BehaviorKind::LostLife)
            | (60310, BehaviorKind::ToughnessOverflowDamage) => {
                let amount = loss::amount(target_uid, context.managers, behavior)?;
                if amount <= 0 {
                    return Some(Vec::new());
                }
                return Some(vec![RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(
                    HpLoss {
                        origin,
                        source_uid,
                        target_uid,
                        amount,
                        config_effect: behavior.config_effect,
                        hurt: (context.active_skill_id != 0).then_some(HurtInfoData {
                            from_uid: source_uid,
                            is_crit: false,
                            career_restraint: false,
                            reduce_hp: 0,
                            effect_id: context.active_skill_id,
                            skill_id: context.target.active_skill_id,
                            damage_from: HurtDamageFromType::SkillEffect,
                            buff_act_id: 0,
                            buff_uid: 0,
                            hurt_effect_type: 0,
                            display_amount: None,
                        }),
                    },
                )))]);
            }
            (60212, BehaviorKind::LostAllLifeByAttr) => {
                let [raw_current_hp, current_rate, raw_max_hp, max_rate] = behavior.args.as_slice()
                else {
                    return None;
                };
                if AttrId::from_raw(*raw_current_hp) != Some(AttrId::CurrentHp)
                    || AttrId::from_raw(*raw_max_hp) != Some(AttrId::Hp)
                    || *current_rate < 0
                    || *max_rate < 0
                {
                    return None;
                }
                let source_max = context.managers.hp.max(source_uid).max(0);
                return Some(
                    context
                        .pool
                        .active_entities()
                        .filter_map(|entity| {
                            let hp = context.managers.hp.get(entity.uid);
                            let floor = crate::engine::damage::scale_permille(
                                hp.max,
                                context.managers.buff.lost_life_floor_permille(entity.uid),
                            );
                            let requested =
                                crate::engine::damage::scale_permille(hp.current, *current_rate)
                                    .min(crate::engine::damage::scale_permille(
                                        source_max, *max_rate,
                                    ))
                                    .min((hp.current - floor).max(0));
                            (requested > 0).then_some(RuleOp::Command(BattleCommand::Hp(
                                HpCommand::Lose(HpLoss {
                                    origin,
                                    source_uid,
                                    target_uid: entity.uid,
                                    amount: requested,
                                    config_effect: behavior.config_effect,
                                    hurt: Some(HurtInfoData {
                                        from_uid: source_uid,
                                        is_crit: false,
                                        career_restraint: false,
                                        reduce_hp: 0,
                                        effect_id: context.active_skill_id,
                                        skill_id: context.target.active_skill_id,
                                        damage_from: HurtDamageFromType::SkillEffect,
                                        buff_act_id: 0,
                                        buff_uid: 0,
                                        hurt_effect_type: 0,
                                        display_amount: None,
                                    }),
                                }),
                            )))
                        })
                        .collect(),
                );
            }
            (60216, BehaviorKind::DamageRealLostLife) => {
                let [replacement_buff_id, count_scope, rate_per_character] =
                    behavior.args.as_slice()
                else {
                    return None;
                };
                if *replacement_buff_id <= 0 || *count_scope != 3 || *rate_per_character <= 0 {
                    return None;
                }
                let mut feature = BuffManager::configured_features(*replacement_buff_id)
                    .into_iter()
                    .find(|feature| is_kind(feature, BuffActKind::AttrOnlyCalDamageReplaceAttr))?;
                feature.owner_uid = source_uid;
                feature.source_uid = source_uid;
                let replacement = crate::engine::skill::buff_act::attack_replacement_rule(
                    &feature,
                    &context.managers.hp,
                )?;
                let character_count = context
                    .pool
                    .active_entities()
                    .filter(|entity| context.managers.hp.current(entity.uid) > 0)
                    .count() as i32;
                let mut source = context.pool.entity(source_uid)?.clone();
                source.damage_type = crate::engine::skill::target::EntityDamageType::Reality;
                let target = context.pool.entity(target_uid)?;
                let is_crit = context.determinism.roll_hidden_crit(
                    context.active_skill_id,
                    source_uid,
                    target_uid,
                    crit_chance(source_uid, target_uid, context.pool, context.managers),
                );
                let rate = character_count * *rate_per_character;
                let amount = direct_damage(
                    DamageRequest {
                        source_uid,
                        target_uid,
                        skill_id: context.active_skill_id,
                        rate,
                        rate_terms: &[],
                        attack_attributes: &context.modifiers.attack_attributes,
                        career_ratio_bonus: context.modifiers.career_ratio_bonus,
                        attack_career: context.modifiers.attack_career,
                        is_conduit: context
                            .managers
                            .conduit
                            .owns_skill(source_uid, context.active_skill_id),
                        is_crit,
                        extra_skill_kind: context.target.extra_skill_kind,
                    },
                    DamageRuntime {
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
                    &source,
                    target,
                    DirectOptions {
                        base_rate: rate,
                        added_rate: 0,
                        formula: replacement.formula,
                        attack_replacement: Some(replacement),
                    },
                );
                return Some(
                    (amount > 0)
                        .then(|| {
                            RuleOp::Command(BattleCommand::Hp(HpCommand::Damage(HpDamage {
                                origin,
                                source_uid,
                                target_uid,
                                amount,
                                config_effect: behavior.config_effect,
                                effect_kind: if is_crit {
                                    DamageEffectKind::Critical
                                } else {
                                    DamageEffectKind::Normal
                                },
                                assassinate: false,
                                ignore_riposte: false,
                                hurt: HurtInfoData {
                                    from_uid: source_uid,
                                    is_crit,
                                    career_restraint: restrains_target(
                                        context.modifiers.attack_career.unwrap_or(source.career),
                                        target,
                                    ),
                                    reduce_hp: 0,
                                    effect_id: context.active_skill_id,
                                    skill_id: context.target.active_skill_id,
                                    damage_from: HurtDamageFromType::SkillEffect,
                                    buff_act_id: 0,
                                    buff_uid: 0,
                                    hurt_effect_type: if is_crit {
                                        EffectType::Crit as i32
                                    } else {
                                        EffectType::Damage as i32
                                    },
                                    display_amount: None,
                                },
                            })))
                        })
                        .into_iter()
                        .collect(),
                );
            }
            _ => {}
        }
        if matches!(
            (behavior.spec.key.opcode, behavior.spec.kind),
            (10006, BehaviorKind::Damage)
        ) {
            let amount = origin::amount(
                context.source_uid,
                context.target_uid,
                origin::OriginRuntime {
                    managers: context.managers,
                    pool: context.pool,
                    extra_action,
                },
                &context.modifiers.attack_attributes,
                false,
                behavior,
            )?;
            return Some(vec![RuleOp::Command(BattleCommand::Hp(HpCommand::Damage(
                HpDamage {
                    origin,
                    source_uid: context.source_uid,
                    target_uid: context.target_uid,
                    amount,
                    config_effect: behavior.config_effect,
                    effect_kind: DamageEffectKind::Genesis,
                    assassinate: false,
                    ignore_riposte: false,
                    hurt: HurtInfoData {
                        from_uid: context.source_uid,
                        is_crit: false,
                        career_restraint: false,
                        reduce_hp: 0,
                        effect_id: context.active_skill_id,
                        skill_id: context.target.active_skill_id,
                        damage_from: HurtDamageFromType::SkillEffect,
                        buff_act_id: 0,
                        buff_uid: 0,
                        hurt_effect_type: EffectType::Origindamage as i32,
                        display_amount: None,
                    },
                },
            )))]);
        }
        if !matches!(
            (behavior.spec.key.opcode, behavior.spec.kind),
            (10008, BehaviorKind::Damage2)
        ) {
            return None;
        }
        let amount = behavior.args.first().copied()?;
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
                    skill_id: context.target.active_skill_id,
                    damage_from: if context.active_skill_id != 0 {
                        HurtDamageFromType::SkillEffect
                    } else {
                        HurtDamageFromType::Skill
                    },
                    buff_act_id: 0,
                    buff_uid: 0,
                    hurt_effect_type: 0,
                    display_amount: None,
                }),
            },
        )))])
    }
}

pub(crate) fn supports_butterfly_damage(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [_, step, _, cap] if *step > 0 && *cap >= 0)
}

pub(crate) fn supports_heal(behavior: &ParsedBehavior) -> bool {
    match behavior.args.as_slice() {
        [amount] => *amount >= 0,
        [mode @ (0 | 1), raw_attr, rate] => {
            *mode <= 1 && AttrId::from_raw(*raw_attr).is_some() && *rate >= 0
        }
        _ => false,
    }
}

pub(crate) fn supports_attr_heal(behavior: &ParsedBehavior) -> bool {
    matches!(
        behavior.args.as_slice(),
        [mode @ (0 | 1), raw_attr, rate]
            if *mode <= 1 && AttrId::from_raw(*raw_attr).is_some() && *rate >= 0
    )
}

pub(crate) fn supports_two_attr_heal(behavior: &ParsedBehavior) -> bool {
    matches!(
        behavior.args.as_slice(),
        [1, raw_missing_hp, missing_rate, 0, raw_max_hp, max_hp_rate]
            if AttrId::from_raw(*raw_missing_hp) == Some(AttrId::LostHp)
                && AttrId::from_raw(*raw_max_hp) == Some(AttrId::Hp)
                && *missing_rate >= 0
                && *max_hp_rate >= 0
    )
}

pub(crate) fn supports_team_attr_damage(behavior: &ParsedBehavior) -> bool {
    matches!(
        behavior.args.as_slice(),
        [1, raw_attr, rate] if AttrId::from_raw(*raw_attr).is_some() && *rate >= 0
    )
}

pub(crate) fn supports_lost_life(behavior: &ParsedBehavior) -> bool {
    matches!(
        behavior.args.as_slice(),
        [mode @ (0 | 1), raw_attr, permille]
            if AttrId::from_raw(*raw_attr).is_some() && *permille >= 0 && *mode <= 1
    )
}

pub(crate) fn supports_lost_all_life_by_attr(behavior: &ParsedBehavior) -> bool {
    matches!(
        behavior.args.as_slice(),
        [raw_current_hp, current_rate, raw_max_hp, max_rate]
            if AttrId::from_raw(*raw_current_hp) == Some(AttrId::CurrentHp)
                && AttrId::from_raw(*raw_max_hp) == Some(AttrId::Hp)
                && *current_rate >= 0
                && *max_rate >= 0
    )
}

pub(crate) fn supports_damage_real_lost_life(behavior: &ParsedBehavior) -> bool {
    matches!(
        behavior.args.as_slice(),
        [replacement_buff_id, 3, rate_per_character]
            if *replacement_buff_id > 0 && *rate_per_character > 0
    )
}

#[cfg(test)]
mod test;
