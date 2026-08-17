use std::collections::HashMap;

use sonettobuf::{CardInfo, EnhanceInfoBox, FightEntityInfo, HeroAttribute};

use crate::engine::{
    entity::attr::AttrId,
    manager::{buff::BuffManager, card::PlayedCard, hp::HpManager},
    mechanic::impromptu::ImpromptuDefinition,
    skill::{
        buff_act::{is_kind, registry::BuffActKind},
        target::TargetPool,
    },
};

pub const UID: i64 = 99998;

pub(crate) fn activation_entity() -> FightEntityInfo {
    let attr = HeroAttribute {
        hp: Some(0),
        attack: Some(0),
        defense: Some(0),
        mdefense: Some(0),
        technic: Some(0),
        multi_hp_idx: Some(0),
        multi_hp_num: Some(0),
    };
    FightEntityInfo {
        uid: Some(UID),
        model_id: Some(0),
        skin: Some(0),
        position: Some(0),
        entity_type: Some(6),
        user_id: Some(0),
        ex_point: Some(0),
        level: Some(0),
        current_hp: Some(1),
        attr: Some(attr),
        ex_skill: Some(0),
        shield_value: Some(0),
        expoint_max_add: Some(0),
        buff_harm_statistic: Some(0),
        equip_uid: Some(0),
        trial_equip: Some(Default::default()),
        ex_skill_level: Some(0),
        base_attr: Some(attr),
        ex_skill_point_change: Some(0),
        team_type: Some(1),
        enhance_info_box: Some(EnhanceInfoBox {
            uid: Some(UID),
            ..Default::default()
        }),
        trial_id: Some(0),
        career: Some(0),
        status: Some(0),
        guard: Some(-1),
        sub_cd: Some(0),
        ex_point_type: Some(0),
        destiny_stone: Some(0),
        destiny_rank: Some(0),
        custom_unit_id: Some(0),
        ..Default::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmitterCard {
    pub card_index: i32,
    pub skill_id: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmitterAttack {
    pub index: i32,
    pub max: i32,
    pub split_count: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmitterEnergyChange {
    pub requested_delta: i32,
    pub applied_delta: i32,
    pub after: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmitterSkillRate {
    pub act_id: i32,
    pub rate: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitterOperation {
    Enable(ImpromptuDefinition),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmitterCommand {
    pub origin: crate::engine::skill::rule::CommandOrigin,
    pub operation: EmitterOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmitterChange {
    pub origin: crate::engine::skill::rule::CommandOrigin,
    pub enabled_before: bool,
    pub enabled_after: bool,
}

#[derive(Debug, Clone, Default)]
pub struct EmitterManager {
    definition: Option<ImpromptuDefinition>,
    energy: i32,
    ally_attributes: HashMap<AttrId, i32>,
}

impl EmitterManager {
    pub fn execute_command(&mut self, command: EmitterCommand) -> EmitterChange {
        let enabled_before = self.definition.is_some();
        match command.operation {
            EmitterOperation::Enable(definition) => self.enable(definition),
        }
        EmitterChange {
            origin: command.origin,
            enabled_before,
            enabled_after: self.definition.is_some(),
        }
    }

    pub fn enable(&mut self, definition: ImpromptuDefinition) {
        self.definition = Some(definition);
    }

    pub fn inspiration_from_eureka(&self, applied_delta: i32) -> i32 {
        if self.definition.is_some() {
            (-applied_delta).max(0)
        } else {
            0
        }
    }

    pub fn collect_energy(&mut self, played: &[PlayedCard]) -> Option<EmitterEnergyChange> {
        if self.definition.is_none() || played.is_empty() {
            return None;
        }
        let delta = played
            .iter()
            .map(|played| played.card.energy.unwrap_or_default())
            .sum();
        Some(self.add_energy(delta))
    }

    pub fn can_spend(&self, cost: i32) -> bool {
        self.energy >= cost
    }

    pub fn energy(&self) -> i32 {
        self.energy
    }

    pub fn snapshot_ally_attributes(
        &mut self,
        pool: &TargetPool,
        buffs: &BuffManager,
        hp: &HpManager,
    ) {
        self.ally_attributes.clear();
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
            self.ally_attributes.insert(
                attr_id,
                average_ally_derived_attribute(pool, buffs, hp, attr_id),
            );
        }
    }

    pub fn ally_attribute(&self, attr_id: AttrId) -> i32 {
        self.ally_attributes
            .get(&attr_id)
            .copied()
            .unwrap_or_default()
    }

    pub fn add_energy(&mut self, delta: i32) -> EmitterEnergyChange {
        let before = self.energy;
        self.energy = self.energy.saturating_add(delta).max(0);
        EmitterEnergyChange {
            requested_delta: delta,
            applied_delta: self.energy - before,
            after: self.energy,
        }
    }

    pub fn finish_skill(&mut self) -> EmitterEnergyChange {
        self.add_energy(-self.energy)
    }

    pub fn skill_rate(&self, source_uid: i64, skill_id: i32) -> Option<EmitterSkillRate> {
        let definition = self.definition?;
        (source_uid == UID && skill_id == definition.skill_id() && self.energy > 0).then_some(
            EmitterSkillRate {
                act_id: definition.damage_up_act_id(),
                rate: definition.damage_rate(self.energy),
            },
        )
    }

    pub fn queue_card(&self, played_count: usize) -> Option<EmitterCard> {
        let definition = self.definition?;
        (self.energy > 0 && played_count > 0).then_some(EmitterCard {
            card_index: played_count as i32 + 1,
            skill_id: definition.skill_id(),
        })
    }

    pub fn card(&self, skill_id: i32) -> CardInfo {
        CardInfo {
            uid: Some(UID),
            skill_id: Some(skill_id),
            card_effect: Some(0),
            temp_card: Some(false),
            enchants: Vec::new(),
            card_type: Some(0),
            hero_id: Some(0),
            status: Some(0),
            target_uid: Some(0),
            extra_info: None,
            energy: Some(0),
            extra_infos: Vec::new(),
            area_red_or_blue: Some(0),
            heat_id: Some(0),
            music_note: None,
            card_dataes: Vec::new(),
        }
    }

    pub fn begin_attack(
        &self,
        source_uid: i64,
        index: i32,
        max: i32,
        split_count: i32,
    ) -> Option<EmitterAttack> {
        if self.definition.is_none() || source_uid != UID {
            return None;
        }
        Some(EmitterAttack {
            index,
            max,
            split_count,
        })
    }
}

pub fn average_ally_buff_attribute(
    pool: &TargetPool,
    buffs: &BuffManager,
    hp: &HpManager,
    attr_id: AttrId,
) -> i32 {
    let allies = pool
        .main_allies(UID)
        .iter()
        .filter(|ally| hp.current(ally.uid) > 0)
        .collect::<Vec<_>>();
    if allies.is_empty() {
        return 0;
    }
    allies
        .iter()
        .map(|ally| buffs.attribute_delta(ally.uid, attr_id))
        .map(i64::from)
        .sum::<i64>()
        .checked_div(allies.len() as i64)
        .unwrap_or_default() as i32
}

fn average_ally_derived_attribute(
    pool: &TargetPool,
    buffs: &BuffManager,
    hp: &HpManager,
    attr_id: AttrId,
) -> i32 {
    let allies = pool
        .main_allies(UID)
        .iter()
        .filter(|ally| hp.current(ally.uid) > 0)
        .collect::<Vec<_>>();
    if allies.is_empty() {
        return 0;
    }
    let features = buffs.active_features(hp);
    allies
        .iter()
        .map(|ally| {
            features
                .iter()
                .filter(|feature| feature.owner_uid == ally.uid)
                .filter(|feature| is_kind(feature, BuffActKind::AttrByShield))
                .map(|feature| {
                    crate::engine::skill::buff_act::attack_attribute_delta(
                        feature, attr_id, buffs, hp,
                    )
                })
                .sum::<i32>()
        })
        .map(i64::from)
        .sum::<i64>()
        .checked_div(allies.len() as i64)
        .unwrap_or_default() as i32
}

#[cfg(test)]
mod tests {
    use sonettobuf::{Fight, FightEntityInfo, FightTeam};

    use super::*;
    use crate::engine::{
        manager::hp::{HpCommand, MaxHpAdjust},
        skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
    };

    fn definition() -> ImpromptuDefinition {
        ImpromptuDefinition::new(11, 22, 150)
    }

    const ORIGIN: CommandOrigin = CommandOrigin {
        domain: RuleDomain::BuffAct,
        key: DefinitionKey::new(875, "EmitterTag"),
    };

    #[test]
    fn activation_registers_hp_presence_without_replacing_virtual_stats() {
        crate::test_support::init_config();
        let entity = |uid, attack| FightEntityInfo {
            uid: Some(uid),
            team_type: Some(1),
            current_hp: Some(100),
            attr: Some(HeroAttribute {
                hp: Some(100),
                attack: Some(attack),
                ..Default::default()
            }),
            ..Default::default()
        };
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![entity(10, 100), entity(11, 300)],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let mut managers = crate::engine::manager::BattleManagers::seeded(&fight);

        managers.execute_emitter(EmitterCommand {
            origin: ORIGIN,
            operation: EmitterOperation::Enable(definition()),
        });

        assert_eq!(managers.hp.current(UID), 1);
        assert_eq!(managers.hp.max(UID), 0);
        assert!(managers.entity.snapshot(UID).is_none());
        assert_eq!(
            pool.runtime_view(&managers).entity(UID).unwrap().attack,
            200
        );

        managers
            .hp
            .execute_command(HpCommand::AdjustMax(MaxHpAdjust {
                origin: ORIGIN,
                source_uid: 10,
                target_uid: UID,
                delta: 1,
            }))
            .unwrap();
        managers.execute_emitter(EmitterCommand {
            origin: ORIGIN,
            operation: EmitterOperation::Enable(definition()),
        });

        assert_eq!(managers.hp.current(UID), 1);
        assert_eq!(managers.hp.max(UID), 1);
    }

    #[test]
    fn activation_identity_matches_the_captured_emitter_shell() {
        let entity = activation_entity();

        assert_eq!(entity.uid, Some(UID));
        assert_eq!(entity.entity_type, Some(6));
        assert_eq!(entity.team_type, Some(1));
        assert_eq!(entity.current_hp, Some(1));
        assert_eq!(entity.attr.unwrap().hp, Some(0));
        assert_eq!(entity.base_attr.unwrap().hp, Some(0));
        assert_eq!(entity.guard, Some(-1));
        assert_eq!(entity.enhance_info_box.unwrap().uid, Some(UID));
    }

    #[test]
    fn queues_only_when_played_cards_produce_inspiration() {
        let mut emitter = EmitterManager::default();
        emitter.enable(definition());
        let played = [PlayedCard {
            card: CardInfo {
                energy: Some(2),
                ..Default::default()
            },
            caster_uid: 0,
            card_index: 1,
            skill_id: 1,
            rank_change_pending: false,
            rewritten: false,
            target_uid: None,
            recorded_skill: None,
        }];

        assert!(emitter.queue_card(played.len()).is_none());
        assert_eq!(emitter.collect_energy(&played).unwrap().applied_delta, 2);
        assert!(emitter.queue_card(played.len()).is_some());
        assert_eq!(emitter.finish_skill().applied_delta, -2);
        assert!(emitter.queue_card(played.len()).is_none());
    }

    #[test]
    fn inspiration_uses_the_applied_eureka_spend() {
        let mut emitter = EmitterManager::default();
        assert_eq!(emitter.inspiration_from_eureka(-2), 0);

        emitter.enable(definition());
        assert_eq!(emitter.inspiration_from_eureka(-1), 1);
        assert_eq!(emitter.inspiration_from_eureka(2), 0);
    }

    #[test]
    fn impromptu_damage_rate_uses_all_collected_inspiration() {
        let mut emitter = EmitterManager::default();
        emitter.enable(definition());
        emitter.add_energy(6);

        assert_eq!(
            emitter.skill_rate(UID, definition().skill_id()),
            Some(EmitterSkillRate {
                act_id: definition().damage_up_act_id(),
                rate: 900,
            })
        );
        assert_eq!(emitter.skill_rate(1, definition().skill_id()), None);
    }

    #[test]
    fn emitter_definition_is_resolved_from_config_types() {
        crate::test_support::init_config();
        let db = config::get();
        let definition = crate::catalog::impromptu_definition(db).unwrap();

        assert_eq!(
            definition.skill_id(),
            db.fight_asfd_const
                .get(5)
                .unwrap()
                .value
                .parse::<i32>()
                .unwrap()
        );
        assert_eq!(
            db.buff_act
                .get(definition.damage_up_act_id())
                .unwrap()
                .r#type,
            "EmitterDamageUp"
        );
    }

    #[test]
    fn emitter_averages_stored_ally_buff_attributes_including_critical_stats() {
        crate::test_support::init_config();
        let entity = |uid| FightEntityInfo {
            uid: Some(uid),
            current_hp: Some(100),
            ..Default::default()
        };
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![entity(10), entity(11)],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let mut managers = crate::engine::manager::BattleManagers::seeded(&fight);
        managers.buff.add(&managers.hp, 10, 10, 31080121, 0);
        managers.buff.add(&managers.hp, 11, 11, 31080122, 0);
        managers.buff.add(&managers.hp, 10, 10, 31080145, 0);
        managers.buff.add(&managers.hp, 11, 11, 31080146, 0);

        assert_eq!(
            average_ally_buff_attribute(&pool, &managers.buff, &managers.hp, AttrId::DmgBonus),
            275
        );
        assert_eq!(
            average_ally_buff_attribute(&pool, &managers.buff, &managers.hp, AttrId::CriticalRate,),
            37
        );
        assert_eq!(
            average_ally_buff_attribute(&pool, &managers.buff, &managers.hp, AttrId::CriticalDmg,),
            37
        );
    }

    #[test]
    fn emitter_snapshot_excludes_after_round_start_team_energy_attributes() {
        crate::test_support::init_config();
        let entity = |uid| FightEntityInfo {
            uid: Some(uid),
            current_hp: Some(100),
            ..Default::default()
        };
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![entity(10), entity(11)],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let mut managers = crate::engine::manager::BattleManagers::seeded(&fight);
        let added = managers
            .buff
            .add(&managers.hp, 10, 10, 31080143, 0)
            .unwrap();
        managers
            .execute_buff(crate::engine::manager::buff::BuffCommand::SetState(
                crate::engine::manager::buff::BuffSetState {
                    ex_info: None,
                    origin: crate::engine::skill::buff_act::configured_command_origin(
                        882,
                        crate::engine::skill::buff_act::registry::BuffActKind::FixAttrTeamEnergy,
                    )
                    .unwrap(),
                    target_uid: 10,
                    buff_uid: added.buff.uid.unwrap(),
                    params: Some("882#10".into()),
                    act_info: None,
                },
            ))
            .unwrap();

        assert_eq!(
            average_ally_buff_attribute(&pool, &managers.buff, &managers.hp, AttrId::DmgBonus),
            0
        );
    }

    #[test]
    fn emitter_snapshot_includes_existing_conditional_shield_attributes() {
        crate::test_support::init_config();
        let entity = |uid| FightEntityInfo {
            uid: Some(uid),
            current_hp: Some(100),
            ..Default::default()
        };
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![entity(10), entity(11)],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let mut managers = crate::engine::manager::BattleManagers::seeded(&fight);
        managers.buff.add(&managers.hp, 10, 10, 31170002, 0);
        managers.buff.add(&managers.hp, 11, 11, 31170002, 0);
        managers.hp.set_shield(10, 6000);
        managers.hp.set_shield(11, 3000);

        managers
            .emitter
            .snapshot_ally_attributes(&pool, &managers.buff, &managers.hp);

        assert_eq!(managers.emitter.ally_attribute(AttrId::DmgBonus), 90);
    }
}
