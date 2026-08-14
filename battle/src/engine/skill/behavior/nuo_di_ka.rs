use crate::engine::{
    damage::handler::{
        DamageRequest, DamageRuntime, crit_chance, resolve_configured_replacement_damage_command,
    },
    entity::attr::AttrId,
    manager::{
        buff::{BuffCommand, BuffGrant, BuffRemove, BuffRemoveSelector},
        hp::{DamageEffectKind, HpCommand, HpDamage, HpHeal, HpHealKind},
    },
    mechanic::nuo_di_ka::NuoDiKaHit,
    skill::{
        behavior::{
            BehaviorOpContext,
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

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        arguments(behavior).is_some()
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        emit_ops(context, behavior, None)
    }

    fn emit_runtime_ops(
        context: BehaviorOpContext<'_>,
        behavior: &ParsedBehavior,
        catalog: &SkillEffectCatalog,
    ) -> Option<Vec<RuleOp>> {
        emit_ops(context, behavior, Some(catalog))
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        references(behavior)
    }
}

fn emit_ops(
    context: BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
    effects: Option<&SkillEffectCatalog>,
) -> Option<Vec<RuleOp>> {
    let [
        random_replacement,
        random_rate,
        mass_replacement,
        mass_rate,
        heal_per_point,
    ] = arguments(behavior)?;
    let points = context.managers.nuo_di_ka.get(context.source_uid);
    if points <= 0 {
        return Some(Vec::new());
    }
    let enemies = context
        .pool
        .enemies(context.source_uid, true)
        .iter()
        .filter(|enemy| context.managers.hp.current(enemy.uid) > 0)
        .map(|enemy| enemy.uid)
        .collect::<Vec<_>>();
    if enemies.is_empty() {
        return Some(Vec::new());
    }
    let origin = super::command_origin(behavior)?;
    let runtime = DamageRuntime {
        fight_version: context.managers.fight_version(),
        pool: context.pool,
        attributes: &context.managers.attribute,
        buffs: &context.managers.buff,
        target_buffs: &context.managers.buff,
        hp: &context.managers.hp,
        fields: Some((&context.managers.field, context.managers.catalog())),
        emitter: None,
        team_inspiration: 0,
    };
    let random_targets = (0..points)
        .map(|_| {
            context
                .determinism
                .lua_random_index(enemies.len())
                .map(|index| enemies[index])
        })
        .collect::<Option<Vec<_>>>()?;
    let effects = match effects {
        Some(effects) => effects,
        None => crate::engine::skill::effect::catalog::global(),
    };
    let mut damage = |target_uid, replacement_buff_id, rate| {
        let mut modifiers = context.modifiers.clone();
        let mut target_context = *context.target;
        target_context.hit_source_uid = context.source_uid;
        target_context.hit_target_uid = target_uid;
        target_context.runtime_target_uid = target_uid;
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
            RateRuntime {
                effects,
                managers: context.managers,
                pool: context.pool,
                context: target_context,
            },
            context.determinism,
        );
        let incoming_modifiers = rate::incoming_target_attack_modifiers(
            context.source_uid,
            target_uid,
            context.active_skill_id,
            RateRuntime {
                effects,
                managers: context.managers,
                pool: context.pool,
                context: target_context,
            },
            context.determinism,
        );
        modifiers.merge(incoming_modifiers);
        let extra_action = crate::engine::skill::condition::extra::skill_kind_from_is_extra(
            target_context.extra_skill_kind,
        )
        .is_some_and(|kind| kind.is_extra_action());
        for attr_id in [AttrId::CriticalDmg, AttrId::DmgBonus] {
            let delta = crate::engine::skill::buff_act::target_attack_attribute_delta(
                context.managers,
                target_uid,
                extra_action,
                attr_id,
            );
            if delta != 0 {
                modifiers.attack_attributes.push((attr_id, delta));
            }
        }
        let critical_rate = modifiers
            .attack_attributes
            .iter()
            .filter_map(|(attr, delta)| (*attr == AttrId::CriticalRate).then_some(*delta))
            .sum::<i32>();
        let is_crit = context.determinism.roll_hidden_crit(
            context.active_skill_id,
            context.source_uid,
            target_uid,
            crit_chance(
                context.source_uid,
                target_uid,
                context.pool,
                context.managers,
            ) + critical_rate,
        );
        let command = resolve_configured_replacement_damage_command(
            DamageRequest {
                source_uid: context.source_uid,
                target_uid,
                skill_id: context.active_skill_id,
                rate,
                rate_terms: &[],
                attack_attributes: &modifiers.attack_attributes,
                career_ratio_bonus: modifiers.career_ratio_bonus,
                attack_career: modifiers.attack_career,
                additional_attack_career: modifiers.additional_attack_career,
                critical_multiplier_remainder: 0,
                is_conduit: context
                    .managers
                    .conduit
                    .owns_skill(context.source_uid, context.active_skill_id),
                is_crit,
                extra_skill_kind: context.target.extra_skill_kind,
            },
            runtime,
            replacement_buff_id,
            origin,
            behavior.config_effect,
            0,
        )?;
        let HpCommand::Damage(command) = command else {
            return None;
        };
        Some(command)
    };
    let mut hits = Vec::with_capacity(points as usize + enemies.len());
    for (index, target_uid) in random_targets.into_iter().enumerate() {
        if let Some(command) = damage(target_uid, random_replacement, random_rate) {
            hits.push((
                NuoDiKaHit {
                    target_uid,
                    amount: command.amount,
                    effect_kind: command.effect_kind,
                    mass: false,
                    hit_index: index as i32 + 1,
                    points,
                    config_effect: behavior.config_effect,
                    buff_act_id: context.active_skill_id,
                },
                command,
            ));
        }
    }
    for target_uid in enemies {
        if let Some(command) = damage(target_uid, mass_replacement, mass_rate) {
            hits.push((
                NuoDiKaHit {
                    target_uid,
                    amount: command.amount,
                    effect_kind: command.effect_kind,
                    mass: true,
                    hit_index: 0,
                    points,
                    config_effect: behavior.config_effect,
                    buff_act_id: context.active_skill_id,
                },
                command,
            ));
        }
    }
    let mut ops = vec![RuleOp::Command(BattleCommand::NuoDiKa(
        crate::engine::mechanic::nuo_di_ka::NuoDiKaCommand::Clear {
            owner_uid: context.source_uid,
        },
    ))];
    let temporary_buff = |buff_id| {
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: context.source_uid,
            target_uid: context.source_uid,
            buff_id,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        })))
    };
    let remove_temporary_buff = |buff_id| {
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
            origin,
            target_uid: context.source_uid,
            selector: BuffRemoveSelector::ExactId(buff_id),
        })))
    };
    ops.push(temporary_buff(random_replacement));
    ops.extend(
        hits.iter()
            .filter(|(hit, _)| !hit.mass)
            .map(|(hit, _)| RuleOp::NuoDiKaHit(*hit)),
    );
    ops.push(remove_temporary_buff(random_replacement));
    ops.push(temporary_buff(mass_replacement));
    ops.extend(
        hits.iter()
            .filter(|(hit, _)| hit.mass)
            .map(|(hit, _)| RuleOp::NuoDiKaHit(*hit)),
    );
    ops.push(remove_temporary_buff(mass_replacement));
    let mut totals = Vec::<HpDamage>::new();
    for (_, command) in hits {
        if let Some(total) = totals
            .iter_mut()
            .find(|total| total.target_uid == command.target_uid)
        {
            total.amount = total.amount.saturating_add(command.amount);
        } else {
            let mut total = command;
            total.effect_kind = DamageEffectKind::Normal;
            total.hurt.is_crit = false;
            total.hurt.buff_act_id = 0;
            totals.push(total);
        }
    }
    if !totals.is_empty() {
        ops.push(RuleOp::Command(BattleCommand::HpBatch(
            totals.into_iter().map(HpCommand::Damage).collect(),
        )));
    }
    let base_heal = context
        .managers
        .hp
        .max(context.source_uid)
        .max(0)
        .saturating_mul(points)
        .saturating_mul(heal_per_point)
        / 1000;
    let heal = crate::engine::damage::handler::modified_heal(
        base_heal,
        context.source_uid,
        context.source_uid,
        context.managers,
    );
    if heal > 0 {
        ops.push(RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(
            HpHeal {
                origin,
                source_uid: context.source_uid,
                target_uid: context.source_uid,
                amount: heal,
                config_effect: behavior.config_effect,
                kind: HpHealKind::Normal,
            },
        ))));
    }
    Some(ops)
}

fn arguments(behavior: &ParsedBehavior) -> Option<[i32; 5]> {
    let values: [i32; 5] = behavior.args.as_slice().try_into().ok()?;
    let [
        random_replacement,
        random_rate,
        mass_replacement,
        mass_rate,
        heal_per_point,
    ] = values;
    (random_replacement > 0
        && random_rate > 0
        && mass_replacement > 0
        && mass_rate > 0
        && heal_per_point >= 0)
        .then_some(values)
}

fn references(behavior: &ParsedBehavior) -> RuleReferences {
    RuleReferences {
        skills: Vec::new(),
        buffs: [0, 2]
            .into_iter()
            .filter_map(|index| behavior.arg(index))
            .filter(|buff_id| *buff_id > 0)
            .collect(),
        models: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute};

    use super::*;
    use crate::engine::{
        manager::BattleManagers,
        mechanic::nuo_di_ka::NuoDiKaCommand,
        runtime::determinism::RoundDeterminism,
        skill::{
            behavior::classify::BehaviorSpec,
            target::{TargetContext, TargetPool},
        },
    };

    #[test]
    fn configured_channel_points_drive_random_mass_damage_and_heal() {
        crate::test_support::init_config();
        let entity = |uid, team_type, hp| FightEntityInfo {
            uid: Some(uid),
            current_hp: Some(hp),
            team_type: Some(team_type),
            attr: Some(HeroAttribute {
                hp: Some(hp),
                attack: Some(1_000),
                defense: Some(0),
                mdefense: Some(0),
                ..Default::default()
            }),
            ..Default::default()
        };
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![entity(10, 1, 20_000)],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![entity(-1, 2, 20_000), entity(-2, 2, 20_000)],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        managers
            .nuo_di_ka
            .execute(NuoDiKaCommand::Set {
                owner_uid: 10,
                points: 3,
                bloodtithe_consumed: 6,
                max_points: 30,
            })
            .unwrap();
        let pool = TargetPool::from_fight(&fight);
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
        let mut target = TargetContext::default();
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(60209, "NuoDiKaDamage"),
            vec![31200135, 1000, 31200132, 1000, 100],
            Vec::new(),
        );

        let ops = emit_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 31200173,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
            Some(crate::engine::skill::effect::catalog::global()),
        )
        .unwrap();

        assert_eq!(ops.len(), 12);
        assert_eq!(
            ops.iter()
                .filter(|op| matches!(op, RuleOp::NuoDiKaHit(_)))
                .count(),
            5
        );
        assert!(matches!(
            &ops[1],
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                buff_id: 31200135,
                ..
            })))
        ));
        assert!(matches!(
            &ops[5],
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
                selector: BuffRemoveSelector::ExactId(31200135),
                ..
            })))
        ));
        assert!(matches!(
            &ops[6],
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                buff_id: 31200132,
                ..
            })))
        ));
        let RuleOp::Command(BattleCommand::HpBatch(commands)) = &ops[10] else {
            panic!("expected one aggregated HP batch");
        };
        assert_eq!(commands.len(), 2);
        assert_eq!(
            commands
                .iter()
                .map(|command| match command {
                    HpCommand::Damage(damage) => damage.amount,
                    _ => 0,
                })
                .sum::<i32>(),
            41_000
        );
        assert!(matches!(
            ops.last(),
            Some(RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(
                HpHeal { amount: 6_000, .. }
            ))))
        ));
    }

    #[test]
    fn descriptor_reports_single_and_mass_buffs() {
        let behavior = ParsedBehavior::new(60209, "NuoDiKaDamage", vec![101, 100, 102, 200, 50]);

        assert_eq!(references(&behavior).buffs, vec![101, 102]);
    }
}
