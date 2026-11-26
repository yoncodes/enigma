use std::collections::HashMap;

use sonettobuf::{Fight, FightEntityInfo, HeroExAttribute, HeroSpAttribute};

use crate::engine::{
    entity::attr::AttrId,
    skill::target::{TargetEntity, TargetPool},
};

#[derive(Debug, Clone, Default)]
pub struct AttributeManager {
    base_values: HashMap<(i64, AttrId), i32>,
    values: HashMap<(i64, AttrId), i32>,
}

impl AttributeManager {
    pub fn seed_with_catalog(&mut self, catalog: crate::catalog::BattleCatalog, fight: &Fight) {
        self.base_values.clear();
        self.values.clear();
        let pool = TargetPool::from_fight_with_catalog(catalog, fight);
        let emitter = pool.entity(crate::engine::manager::emitter::UID);
        for target in pool.entities().chain(emitter) {
            self.register_values(target);
            self.set(
                target.uid,
                AttrId::PoisonDmgBonus,
                crate::engine::entity::stats::destiny_poison_add_rate(
                    target.model_id,
                    target.destiny_rank,
                ),
            );
        }
        for entity in fight
            .attacker
            .iter()
            .chain(fight.defender.iter())
            .filter_map(|team| team.assist_boss.as_ref())
        {
            self.register_with_catalog(catalog, entity);
        }
    }

    #[cfg(test)]
    pub fn seed(&mut self, fight: &Fight) {
        self.seed_with_catalog(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            fight,
        );
    }

    pub fn register_with_catalog(
        &mut self,
        catalog: crate::catalog::BattleCatalog,
        entity: &FightEntityInfo,
    ) {
        let Some(target) = TargetEntity::from_fight_entity_with_catalog(catalog, entity) else {
            return;
        };
        self.register_values(&target);
    }

    #[cfg(test)]
    pub fn register(&mut self, entity: &FightEntityInfo) {
        self.register_with_catalog(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            entity,
        );
    }

    fn register_values(&mut self, target: &TargetEntity) {
        let uid = target.uid;
        for (id, value) in [
            (AttrId::Attack, target.attack),
            (AttrId::RealityDef, target.defense),
            (AttrId::MentalDef, target.mdefense),
            (AttrId::CriticalTechnique, target.technic),
        ] {
            self.base_values.insert((uid, id), value);
        }
        self.set(uid, AttrId::CriticalRate, target.crit_rate);
        self.set(uid, AttrId::CriticalResistRate, target.crit_resist);
        self.set(uid, AttrId::CriticalDmg, target.crit_dmg);
        self.set(uid, AttrId::CriticalDef, target.crit_def);
        self.set(uid, AttrId::DmgBonus, target.add_dmg);
        self.set(uid, AttrId::DmgTakenReduction, target.drop_dmg);
    }

    pub fn override_ex(&mut self, uid: i64, attributes: &HeroExAttribute) {
        for (id, value) in [
            (AttrId::CriticalRate, attributes.cri),
            (AttrId::CriticalResistRate, attributes.recri),
            (AttrId::CriticalDmg, attributes.cri_dmg),
            (AttrId::CriticalDef, attributes.cri_def),
            (AttrId::DmgBonus, attributes.add_dmg),
            (AttrId::DmgTakenReduction, attributes.drop_dmg),
        ] {
            if let Some(value) = value {
                self.set(uid, id, value);
            }
        }
    }

    pub fn override_sp(&mut self, uid: i64, attributes: &HeroSpAttribute) {
        for (id, value) in [
            (AttrId::DmgHeal, attributes.revive),
            (AttrId::HealingDone, attributes.heal),
            (AttrId::LeechRate, attributes.absorb),
            (AttrId::Penetration, attributes.defense_ignore),
            (AttrId::UltimateMight, attributes.clutch),
            (AttrId::FinalDmgBonus, attributes.final_add_dmg),
            (AttrId::FinalDmgReduction, attributes.final_drop_dmg),
            (AttrId::IncantationMight, attributes.normal_skill_rate),
            (AttrId::PlaymodeDmgIncrease, attributes.play_add_rate),
            (AttrId::PlaymodeDmgImmunity, attributes.play_drop_rate),
            (AttrId::ReboundDmg, attributes.rebound_dmg),
            (AttrId::ExtraDmg, attributes.extra_dmg),
            (AttrId::ReuseDmg, attributes.reuse_dmg),
            (AttrId::ConduitMight, attributes.device_skill_rate),
        ] {
            if let Some(value) = value {
                self.set(uid, id, value);
            }
        }
    }

    pub fn sync_emitter_average(&mut self, fight: &Fight) {
        let uids = fight
            .attacker
            .iter()
            .flat_map(|team| team.entitys.iter())
            .filter(|entity| entity.current_hp.unwrap_or(1) > 0)
            .filter_map(|entity| entity.uid)
            .collect::<Vec<_>>();
        if uids.is_empty() {
            return;
        }
        for attr_id in [
            AttrId::CriticalRate,
            AttrId::CriticalResistRate,
            AttrId::CriticalDmg,
            AttrId::CriticalDef,
            AttrId::DmgBonus,
            AttrId::DmgTakenReduction,
            AttrId::DmgHeal,
            AttrId::HealingDone,
            AttrId::LeechRate,
            AttrId::Penetration,
            AttrId::UltimateMight,
            AttrId::FinalDmgBonus,
            AttrId::FinalDmgReduction,
            AttrId::IncantationMight,
            AttrId::PlaymodeDmgIncrease,
            AttrId::PlaymodeDmgImmunity,
            AttrId::ReboundDmg,
            AttrId::ExtraDmg,
            AttrId::ReuseDmg,
        ] {
            let average = uids
                .iter()
                .map(|uid| i64::from(self.get(*uid, attr_id)))
                .sum::<i64>()
                / uids.len() as i64;
            self.set(
                crate::engine::manager::emitter::UID,
                attr_id,
                average as i32,
            );
        }
    }

    pub fn get(&self, uid: i64, id: AttrId) -> i32 {
        self.values.get(&(uid, id)).copied().unwrap_or_default()
    }

    pub fn base(&self, uid: i64, id: AttrId) -> i32 {
        self.base_values
            .get(&(uid, id))
            .copied()
            .unwrap_or_default()
    }

    fn set(&mut self, uid: i64, id: AttrId, value: i32) {
        self.values.insert((uid, id), value);
    }
}

#[cfg(test)]
mod tests;
