use crate::engine::{
    damage::{DamageFormula, handler as damage},
    manager::{BattleManagers, hp::HpCommand},
    runtime::determinism::RoundDeterminism,
    skill::{
        condition::extra::ExtraSkillKind,
        rule::{CommandOrigin, RuleDomain},
        target::TargetPool,
    },
};

use super::{CounterError, read};
use crate::engine::entity::attr::AttrId;
use crate::engine::skill::buff_act::registry;

pub(crate) struct RiposteDamage {
    source_uid: i64,
    origin: CommandOrigin,
    rate: i32,
}

impl RiposteDamage {
    pub(crate) fn for_action(
        managers: &BattleManagers,
        pool: &TargetPool,
        actor_uid: i64,
        extra_kind: i32,
    ) -> Result<Option<Self>, CounterError> {
        if extra_kind != ExtraSkillKind::Riposte.id() {
            return Ok(None);
        }
        let allies = pool.main_allies(actor_uid);
        let mut counters = managers
            .buff
            .active_features(&managers.hp)
            .into_iter()
            .filter_map(|feature| {
                let definition = registry::find(feature.act_id()?, &feature.act_type)?;
                definition.state.riposte_damage?;
                (feature.owner_uid != actor_uid
                    && feature.owner_alive
                    && allies.iter().any(|ally| ally.uid == feature.owner_uid))
                .then_some((feature, definition))
            });
        let Some((feature, definition)) = counters.next() else {
            return Ok(None);
        };
        if counters.next().is_some() {
            return Err(CounterError::MultipleCarriers);
        }
        let [_, capacity, base, fill] = feature.values.as_slice() else {
            return Err(CounterError::InvalidState);
        };
        if !definition.accepts_route(feature.effect_time, feature.effect_condition)
            || !super::supports(&[*capacity, *base, *fill])
        {
            return Err(CounterError::InvalidState);
        }
        let (base, fill) = (i128::from(*base), i128::from(*fill));
        let origin = CommandOrigin {
            domain: RuleDomain::BuffAct,
            key: definition.key,
        };
        let source_uid = feature.owner_uid;
        let counter = read(managers, feature)?;
        let bonus =
            base * fill * i128::from(counter.current) / (1000 * i128::from(counter.maximum));
        let rate = (base + bonus).min(i128::from(i32::MAX)) as i32;
        Ok(Some(Self {
            source_uid,
            origin,
            rate,
        }))
    }

    pub(crate) fn hit(
        &self,
        main: crate::engine::manager::hp::HpDamage,
        skill_id: i32,
        target_attributes: &[(AttrId, i32)],
        managers: &BattleManagers,
        runtime: damage::DamageRuntime<'_>,
        determinism: &mut RoundDeterminism,
    ) -> Option<HpCommand> {
        let source_uid = self.source_uid;
        let target_uid = main.target_uid;
        let damage_type =
            super::super::emitter_damage_type::source_type(source_uid, runtime.pool, managers);
        let mut attack_attributes = target_attributes.to_vec();
        attack_attributes.extend(super::super::incoming_target_attack_attributes(
            managers,
            target_uid,
            damage_type,
            managers.catalog().skill_is_big(skill_id),
        ));
        let shell_bonus = super::super::target_attack_attribute_delta(
            managers,
            target_uid,
            true,
            AttrId::DmgBonus,
        );
        if shell_bonus != 0 {
            attack_attributes.push((AttrId::DmgBonus, shell_bonus));
        }
        let is_crit = determinism.roll_additional_crit(
            skill_id,
            main.source_uid,
            source_uid,
            target_uid,
            damage::crit_chance_with_attack_attributes(
                source_uid,
                target_uid,
                runtime.pool,
                managers,
                &attack_attributes,
            ),
        );
        let mut command = damage::resolve_additional_damage_command(
            damage::DamageRequest {
                source_uid,
                action_owner_uid: main.source_uid,
                target_uid,
                skill_id,
                rate: self.rate,
                rate_terms: &[],
                attack_bonus: 0,
                attack_attributes: &attack_attributes,
                career_ratio_bonus: 0,
                attack_career: None,
                additional_attack_career: None,
                force_career_restraint: false,
                critical_multiplier_remainder: 0,
                is_conduit: false,
                is_crit,
                extra_skill_kind: ExtraSkillKind::Riposte.id(),
                performs_extra_action: true,
            },
            runtime,
            DamageFormula::CreditedSourceAdditional,
            None,
            source_uid,
            false,
            self.origin,
        )?;
        if let HpCommand::Damage(hit) = &mut command {
            hit.ignore_riposte = main.ignore_riposte;
        }
        Some(command)
    }
}

#[cfg(test)]
mod tests;
