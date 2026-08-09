use std::collections::HashMap;

use sonettobuf::{Fight, FightEntityInfo, HeroExAttribute, HeroSpAttribute};

use crate::engine::{entity::attr::AttrId, skill::target::TargetPool};

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
            self.register_values(
                target.uid,
                target.attack,
                target.defense,
                target.mdefense,
                target.technic,
                target.crit_rate,
                target.crit_resist,
                target.crit_dmg,
                target.crit_def,
                target.add_dmg,
                target.drop_dmg,
            );
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
        let Some(target) =
            crate::engine::skill::target::TargetEntity::from_fight_entity_with_catalog(
                catalog, entity,
            )
        else {
            return;
        };
        self.register_values(
            target.uid,
            target.attack,
            target.defense,
            target.mdefense,
            target.technic,
            target.crit_rate,
            target.crit_resist,
            target.crit_dmg,
            target.crit_def,
            target.add_dmg,
            target.drop_dmg,
        );
    }

    #[cfg(test)]
    pub fn register(&mut self, entity: &FightEntityInfo) {
        self.register_with_catalog(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            entity,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn register_values(
        &mut self,
        uid: i64,
        attack: i32,
        defense: i32,
        mdefense: i32,
        technic: i32,
        crit_rate: i32,
        crit_resist: i32,
        crit_dmg: i32,
        crit_def: i32,
        add_dmg: i32,
        drop_dmg: i32,
    ) {
        for (id, value) in [
            (AttrId::Attack, attack),
            (AttrId::RealityDef, defense),
            (AttrId::MentalDef, mdefense),
            (AttrId::CriticalTechnique, technic),
        ] {
            self.base_values.insert((uid, id), value);
        }
        self.set(uid, AttrId::CriticalRate, crit_rate);
        self.set(uid, AttrId::CriticalResistRate, crit_resist);
        self.set(uid, AttrId::CriticalDmg, crit_dmg);
        self.set(uid, AttrId::CriticalDef, crit_def);
        self.set(uid, AttrId::DmgBonus, add_dmg);
        self.set(uid, AttrId::DmgTakenReduction, drop_dmg);
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
mod tests {
    use super::*;

    #[test]
    fn account_attributes_override_config_baseline() {
        let mut manager = AttributeManager::default();
        manager.override_ex(
            10,
            &HeroExAttribute {
                cri: Some(385),
                cri_dmg: Some(1480),
                ..Default::default()
            },
        );

        assert_eq!(manager.get(10, AttrId::CriticalRate), 385);
        assert_eq!(manager.get(10, AttrId::CriticalDmg), 1480);
    }

    #[test]
    fn account_action_attributes_keep_their_distinct_lanes() {
        let mut manager = AttributeManager::default();
        manager.override_sp(
            10,
            &HeroSpAttribute {
                rebound_dmg: Some(30),
                extra_dmg: Some(60),
                reuse_dmg: Some(100),
                device_skill_rate: Some(140),
                defense_ignore: Some(80),
                normal_skill_rate: Some(120),
                ..Default::default()
            },
        );

        assert_eq!(manager.get(10, AttrId::ReboundDmg), 30);
        assert_eq!(manager.get(10, AttrId::ExtraDmg), 60);
        assert_eq!(manager.get(10, AttrId::ReuseDmg), 100);
        assert_eq!(manager.get(10, AttrId::ConduitMight), 140);
        assert_eq!(manager.get(10, AttrId::Penetration), 80);
        assert_eq!(manager.get(10, AttrId::IncantationMight), 120);
    }

    #[test]
    fn emitter_inherits_the_team_average_combat_attributes() {
        crate::test_support::init_config();
        let entity = |uid, model_id| sonettobuf::FightEntityInfo {
            uid: Some(uid),
            model_id: Some(model_id),
            entity_type: Some(1),
            level: Some(180),
            current_hp: Some(100),
            ..Default::default()
        };
        let fight = Fight {
            attacker: Some(sonettobuf::FightTeam {
                entitys: vec![entity(10, 3108), entity(11, 3113)],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut manager = AttributeManager::default();

        manager.seed(&fight);
        manager.override_ex(
            10,
            &HeroExAttribute {
                add_dmg: Some(100),
                cri_dmg: Some(1400),
                ..Default::default()
            },
        );
        manager.override_ex(
            11,
            &HeroExAttribute {
                add_dmg: Some(300),
                cri_dmg: Some(1600),
                ..Default::default()
            },
        );
        manager.sync_emitter_average(&fight);

        assert_eq!(
            manager.get(crate::engine::manager::emitter::UID, AttrId::DmgBonus),
            200
        );
        assert_eq!(
            manager.get(crate::engine::manager::emitter::UID, AttrId::CriticalDmg),
            1500
        );
    }
}
