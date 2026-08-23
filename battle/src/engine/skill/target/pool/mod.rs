use std::collections::{HashMap, HashSet};

use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use crate::engine::{manager::BattleManagers, skill::action::SkillExecutionMode};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(i32)]
pub enum EntityDamageType {
    #[default]
    Unknown = 0,
    Reality = 1,
    Mental = 2,
}

impl EntityDamageType {
    pub fn from_wire(value: i32) -> Self {
        match value {
            1 => Self::Reality,
            2 => Self::Mental,
            _ => Self::Unknown,
        }
    }

    pub const fn id(self) -> i32 {
        self as i32
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TargetEntity {
    pub uid: i64,
    pub level: i32,
    pub model_id: i32,
    pub model_label: i32,
    pub career: i32,
    pub careers: Vec<i32>,
    pub weak_careers: Vec<i32>,
    pub damage_type: EntityDamageType,
    pub position: i32,
    pub current_hp: i32,
    pub max_hp: i32,
    pub attack: i32,
    pub defense: i32,
    pub mdefense: i32,
    pub technic: i32,
    pub base_technic: i32,
    pub crit_rate: i32,
    pub crit_resist: i32,
    pub crit_dmg: i32,
    pub crit_def: i32,
    pub add_dmg: i32,
    pub drop_dmg: i32,
    pub ex_point: i32,
    pub ex_skill: i32,
    pub ex_skill_level: i32,
    pub skill_group1: Vec<i32>,
    pub skill_group2: Vec<i32>,
    pub passive_skills: Vec<i32>,
    pub destiny_stone: i32,
    pub destiny_rank: i32,
    pub battle_tags: Vec<i32>,
    buffs: Vec<TargetBuff>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TargetBuff {
    id: i32,
    type_id: i32,
    status: Option<crate::engine::manager::buff::BuffStatus>,
    source_uid: i64,
    features: Vec<String>,
    act_kinds: Vec<crate::engine::skill::buff_act::registry::BuffActKind>,
    monster_labels: Vec<i32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TargetContext {
    pub battle_id: i32,
    pub current_round: i32,
    pub runtime_target_uid: i64,
    pub event_source_uid: i64,
    pub logic_target: i32,
    pub active_skill_id: i32,
    pub active_skill_source_uid: i64,
    pub active_card_index: i32,
    pub recorded_skill_id: i32,
    pub recorded_skill_source_uid: i64,
    pub shell_change_amount: i32,
    pub shell_deployed_buff_id: i32,
    pub active_skill_slot: i32,
    pub action_order: i32,
    pub active_skill_is_attack: bool,
    pub active_skill_rank: i32,
    pub active_skill_type: i32,
    pub active_skill_effect_tag: i32,
    pub active_skill_assassinate: bool,
    pub active_skill_mode: SkillExecutionMode,
    pub extra_skill_kind: i32,
    pub damage_target_count_kind: i32,
    pub additional_skill_target_count: i32,
    pub extra_damage_target_count: i32,
    pub extra_damage_target_final_damage_delta: i32,
    pub emitter_attack_index: i32,
    pub emitter_attack_max: i32,
    pub ex_point_changed_uid: i64,
    pub ex_point_delta: i32,
    pub additional_moxie: i32,
    pub lost_power_id: i32,
    pub lost_power_amount: i32,
    pub hit_source_uid: i64,
    pub hit_target_uid: i64,
    pub hit_career_restraint: Option<bool>,
    pub hit_damage_from: Option<crate::engine::manager::hp::HurtDamageFromType>,
    pub teammate_injury_count: i32,
    pub teammate_injury_count_not_reset: i32,
    pub team_injury_count_round: i32,
    pub multi_hp_segment: i32,
    pub magic_circle_id: i32,
    pub magic_circle_source_uid: i64,
    pub added_magic_circle_id: i32,
    pub removed_magic_circle_id: i32,
    pub triggered_buff_act_id: i32,
    pub triggered_buff_uid: i64,
    pub added_buff_id: i32,
    pub added_buff_amount: i32,
    pub added_buff_target_uid: i64,
    pub removed_buff_id: i32,
    pub removed_buff_target_uid: i64,
    pub rejected_buff_id: i32,
    pub rejected_buff_type_id: i32,
    pub buff_overflow_amount: i32,
    pub owner_played_card: bool,
    pub direct_skill_body: bool,
    pub action_dealt_damage: bool,
    pub action_damage_amount: i32,
    pub action_crit_count: i32,
    pub critical_action_count: i32,
    pub action_kill_count: i32,
    pub action_guard_break_count: i32,
    pub toughness_broken_uid: i64,
    pub blood_pool_max: i32,
    pub blood_pool_value: i32,
    pub blood_sacrifice_points: i32,
    pub bloodtithe_consumed: i32,
    pub condition_random_roll: Option<i32>,
    pub emanation_crystals: [i32; 3],
    pub heat_scale_value: i32,
    pub heat_scale_raw_value: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TargetPool {
    catalog_data: Option<crate::catalog::BattleCatalog>,
    pub attacker_main: Vec<TargetEntity>,
    pub attacker_all: Vec<TargetEntity>,
    pub defender_main: Vec<TargetEntity>,
    pub defender_all: Vec<TargetEntity>,
    boss_model_ids: Vec<i32>,
    assist_bosses: HashMap<i32, i64>,
    assist_boss_skills: Vec<(i64, Vec<i32>)>,
    reserve_uids: HashSet<i64>,
    virtual_entities: Vec<TargetEntity>,
    teams: HashMap<i64, i32>,
}

impl TargetPool {
    #[cfg(test)]
    pub fn from_fight(fight: &Fight) -> Self {
        Self::from_fight_with_catalog(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            fight,
        )
    }

    pub fn from_fight_with_catalog(catalog: crate::catalog::BattleCatalog, fight: &Fight) -> Self {
        let mut pool = Self {
            catalog_data: Some(catalog),
            boss_model_ids: catalog.boss_model_ids(fight),
            ..Self::default()
        };
        if let Some(team) = &fight.attacker {
            pool.reserve_uids
                .extend(team.sub_entitys.iter().filter_map(|entity| entity.uid));
            pool.teams
                .extend(team_identities(team).filter_map(|entity| entity.uid.map(|uid| (uid, 1))));
            pool.attacker_main = alive_uids(catalog, &team.entitys);
            if let Some(uid) = team.assist_boss.as_ref().and_then(|entity| entity.uid) {
                pool.assist_bosses.insert(1, uid);
                pool.assist_boss_skills.push((
                    uid,
                    team.assist_boss
                        .as_ref()
                        .map(|entity| entity.passive_skill.clone())
                        .unwrap_or_default(),
                ));
                pool.virtual_entities
                    .extend(team.assist_boss.as_ref().and_then(|entity| {
                        TargetEntity::from_fight_entity_with_catalog(catalog, entity)
                    }));
            }
            pool.attacker_all = alive_uids(
                catalog,
                team.entitys
                    .iter()
                    .chain(&team.sub_entitys)
                    .chain(&team.sp_entitys),
            );
        }
        if let Some(team) = &fight.defender {
            pool.reserve_uids
                .extend(team.sub_entitys.iter().filter_map(|entity| entity.uid));
            pool.teams
                .extend(team_identities(team).filter_map(|entity| entity.uid.map(|uid| (uid, 2))));
            pool.defender_main = alive_uids(catalog, &team.entitys);
            if let Some(uid) = team.assist_boss.as_ref().and_then(|entity| entity.uid) {
                pool.assist_bosses.insert(2, uid);
                pool.assist_boss_skills.push((
                    uid,
                    team.assist_boss
                        .as_ref()
                        .map(|entity| entity.passive_skill.clone())
                        .unwrap_or_default(),
                ));
                pool.virtual_entities
                    .extend(team.assist_boss.as_ref().and_then(|entity| {
                        TargetEntity::from_fight_entity_with_catalog(catalog, entity)
                    }));
            }
            pool.defender_all = alive_uids(
                catalog,
                team.entitys
                    .iter()
                    .chain(&team.sub_entitys)
                    .chain(&team.sp_entitys),
            );
        }
        if !pool.attacker_main.is_empty() {
            pool.virtual_entities
                .push(average_emitter(&pool.attacker_main));
            pool.teams.insert(crate::engine::manager::emitter::UID, 1);
        }
        pool
    }

    pub(crate) fn catalog(&self) -> crate::catalog::BattleCatalog {
        self.catalog_data
            .expect("target pool was not constructed with a catalog")
    }

    pub(crate) fn runtime_view(&self, managers: &BattleManagers) -> Self {
        self.runtime_view_including(managers, None)
    }

    pub(crate) fn runtime_view_including(
        &self,
        managers: &BattleManagers,
        included_uid: Option<i64>,
    ) -> Self {
        let mut pool = self.clone();
        let catalog = pool.catalog();
        for entities in [
            &mut pool.attacker_main,
            &mut pool.attacker_all,
            &mut pool.defender_main,
            &mut pool.defender_all,
        ] {
            entities.retain_mut(|entity| {
                if let Some(identity) =
                    managers
                        .entity
                        .snapshot(entity.uid)
                        .as_ref()
                        .and_then(|entity| {
                            TargetEntity::from_fight_entity_with_catalog(catalog, entity)
                        })
                {
                    *entity = identity;
                }
                entity.current_hp = managers.hp.current(entity.uid);
                entity.max_hp = managers.hp.max(entity.uid);
                entity.ex_point = managers.ex_point.get(entity.uid);
                entity.buffs = managers
                    .buff
                    .active_for(entity.uid)
                    .map(|buff| TargetBuff::from_buff_info(catalog, buff))
                    .collect();
                entity.current_hp > 0 || included_uid == Some(entity.uid)
            });
        }
        for entity in &mut pool.virtual_entities {
            if let Some(identity) = managers
                .entity
                .snapshot(entity.uid)
                .as_ref()
                .and_then(|entity| TargetEntity::from_fight_entity_with_catalog(catalog, entity))
            {
                *entity = identity;
            }
            if managers.hp.max(entity.uid) <= 0 {
                continue;
            }
            entity.current_hp = managers.hp.current(entity.uid);
            entity.max_hp = managers.hp.max(entity.uid);
            entity.ex_point = managers.ex_point.get(entity.uid);
            entity.buffs = managers
                .buff
                .active_for(entity.uid)
                .map(|buff| TargetBuff::from_buff_info(catalog, buff))
                .collect();
        }
        pool
    }

    pub fn entity(&self, uid: i64) -> Option<&TargetEntity> {
        self.entities()
            .chain(self.virtual_entities.iter())
            .find(|entity| entity.uid == uid)
    }

    pub fn skill_slot(&self, managers: &BattleManagers, source_uid: i64, skill_id: i32) -> i32 {
        let Some(source) = self.entity(source_uid) else {
            return -1;
        };
        let effect_id = managers.catalog().skill_effect_id(skill_id);
        if source.skill_group1.contains(&skill_id) || source.skill_group1.contains(&effect_id) {
            1
        } else if source.skill_group2.contains(&skill_id)
            || source.skill_group2.contains(&effect_id)
        {
            2
        } else if crate::engine::mechanic::card::CardMechanic
            .is_ultimate_skill(managers, skill_id, source)
        {
            3
        } else {
            -1
        }
    }

    pub fn entities(&self) -> impl Iterator<Item = &TargetEntity> {
        self.attacker_all.iter().chain(self.defender_all.iter())
    }

    pub fn active_entities(&self) -> impl Iterator<Item = &TargetEntity> {
        self.attacker_main.iter().chain(self.defender_main.iter())
    }

    pub fn allies(&self, source_uid: i64) -> &[TargetEntity] {
        match self.team_type(source_uid) {
            Some(1) => &self.attacker_all,
            Some(2) => &self.defender_all,
            _ => &[],
        }
    }

    pub fn main_allies(&self, source_uid: i64) -> &[TargetEntity] {
        match self.team_type(source_uid) {
            Some(1) => &self.attacker_main,
            Some(2) => &self.defender_main,
            _ => &[],
        }
    }

    pub fn is_reserve(&self, uid: i64) -> bool {
        self.reserve_uids.contains(&uid)
    }

    pub fn boss_allies(&self, source_uid: i64) -> Vec<i64> {
        self.allies(source_uid)
            .iter()
            .filter(|entity| self.boss_model_ids.contains(&entity.model_id))
            .map(|entity| entity.uid)
            .collect()
    }

    pub fn first_boss_enemy(&self, source_uid: i64) -> Vec<i64> {
        self.enemies(source_uid, false)
            .iter()
            .find(|entity| self.boss_model_ids.contains(&entity.model_id))
            .map(|entity| entity.uid)
            .into_iter()
            .collect()
    }

    pub fn assist_boss(&self, source_uid: i64) -> Vec<i64> {
        self.team_type(source_uid)
            .and_then(|team| self.assist_bosses.get(&team))
            .copied()
            .into_iter()
            .collect()
    }

    pub(crate) fn assist_boss_skill_owners(&self) -> impl Iterator<Item = (i64, i32)> + '_ {
        self.assist_boss_skills
            .iter()
            .flat_map(|(uid, skills)| skills.iter().map(move |&skill_id| (*uid, skill_id)))
    }

    pub fn enemies(&self, source_uid: i64, main_only: bool) -> &[TargetEntity] {
        match (self.team_type(source_uid), main_only) {
            (Some(1), true) => &self.defender_main,
            (Some(1), false) => &self.defender_all,
            (Some(2), true) => &self.attacker_main,
            (Some(2), false) => &self.attacker_all,
            _ => &[],
        }
    }

    pub fn team_type(&self, uid: i64) -> Option<i32> {
        match uid {
            crate::engine::fight::rules::ATTACKER_SIDE_UID => Some(1),
            crate::engine::fight::rules::DEFENDER_SIDE_UID => Some(2),
            _ => self.teams.get(&uid).copied(),
        }
    }

    pub(crate) fn team_uids(&self, team: i32) -> Vec<i64> {
        let mut uids = self
            .teams
            .iter()
            .filter_map(|(&uid, &entity_team)| (entity_team == team).then_some(uid))
            .collect::<Vec<_>>();
        uids.push(if team == 1 {
            crate::engine::fight::rules::ATTACKER_SIDE_UID
        } else {
            crate::engine::fight::rules::DEFENDER_SIDE_UID
        });
        uids
    }

    pub fn source_is_attacker(&self, source_uid: i64) -> bool {
        self.team_type(source_uid) == Some(1)
    }
}

fn team_identities(team: &FightTeam) -> impl Iterator<Item = &FightEntityInfo> {
    team.entitys
        .iter()
        .chain(&team.sub_entitys)
        .chain(&team.sp_entitys)
        .chain(&team.sp_fight_entities)
        .chain(team.assist_boss.iter())
        .chain(team.emitter.iter())
        .chain(team.player_entity.iter())
        .chain(team.vorpalith.iter())
}

fn average_emitter(allies: &[TargetEntity]) -> TargetEntity {
    let average = |value: fn(&TargetEntity) -> i32| {
        (allies.iter().map(value).map(i64::from).sum::<i64>() / allies.len() as i64) as i32
    };
    TargetEntity {
        uid: crate::engine::manager::emitter::UID,
        level: average(|entity| entity.level),
        damage_type: EntityDamageType::Mental,
        current_hp: average(|entity| entity.current_hp),
        max_hp: average(|entity| entity.max_hp),
        attack: average(|entity| entity.attack),
        defense: average(|entity| entity.defense),
        mdefense: average(|entity| entity.mdefense),
        technic: average(|entity| entity.technic),
        base_technic: average(|entity| entity.base_technic),
        crit_rate: average(|entity| entity.crit_rate),
        crit_resist: average(|entity| entity.crit_resist),
        crit_dmg: average(|entity| entity.crit_dmg),
        crit_def: average(|entity| entity.crit_def),
        add_dmg: average(|entity| entity.add_dmg),
        drop_dmg: average(|entity| entity.drop_dmg),
        ..Default::default()
    }
}

fn alive_uids<'a>(
    catalog: crate::catalog::BattleCatalog,
    entities: impl IntoIterator<Item = &'a FightEntityInfo>,
) -> Vec<TargetEntity> {
    entities
        .into_iter()
        .filter_map(|entity| TargetEntity::from_fight_entity_with_catalog(catalog, entity))
        .collect()
}

impl TargetEntity {
    #[cfg(test)]
    pub(crate) fn from_fight_entity(entity: &FightEntityInfo) -> Option<Self> {
        Self::from_fight_entity_with_catalog(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            entity,
        )
    }

    pub(crate) fn from_fight_entity_with_catalog(
        catalog: crate::catalog::BattleCatalog,
        entity: &FightEntityInfo,
    ) -> Option<Self> {
        let current_hp = entity.current_hp.unwrap_or(1);
        if current_hp <= 0 {
            return None;
        }

        let attr = entity.attr.as_ref();
        let ex = catalog.entity_ex_attributes(
            entity.model_id.unwrap_or_default(),
            entity.level,
            entity.entity_type,
        );
        let configured_groups = catalog.trial_skill_groups(
            entity.trial_id.unwrap_or_default(),
            entity.model_id.unwrap_or_default(),
        );
        let skill_group1 = if entity.skill_group1.is_empty() {
            configured_groups
                .as_ref()
                .map(|groups| groups.group1.clone())
                .unwrap_or_default()
        } else {
            entity.skill_group1.clone()
        };
        let skill_group2 = if entity.skill_group2.is_empty() {
            configured_groups
                .map(|groups| groups.group2)
                .unwrap_or_default()
        } else {
            entity.skill_group2.clone()
        };
        Some(Self {
            uid: entity.uid?,
            level: entity.level.unwrap_or_default(),
            model_id: entity.model_id.unwrap_or_default(),
            model_label: catalog.model_label(entity.model_id.unwrap_or_default()),
            career: entity.career.unwrap_or_default(),
            careers: catalog.careers(entity.career.unwrap_or_default()),
            weak_careers: entity.weak_careers.clone(),
            damage_type: EntityDamageType::from_wire(
                catalog.entity_damage_type(entity.model_id.unwrap_or_default(), entity.entity_type),
            ),
            position: entity.position.unwrap_or_default(),
            current_hp,
            max_hp: attr.and_then(|attr| attr.hp).unwrap_or(1),
            attack: attr.and_then(|attr| attr.attack).unwrap_or_default(),
            defense: attr.and_then(|attr| attr.defense).unwrap_or_default(),
            mdefense: attr.and_then(|attr| attr.mdefense).unwrap_or_default(),
            technic: attr.and_then(|attr| attr.technic).unwrap_or_default(),
            base_technic: catalog.entity_base_technic(
                entity.model_id.unwrap_or_default(),
                entity.level.unwrap_or_default(),
                entity.entity_type,
                attr.and_then(|attr| attr.technic).unwrap_or_default(),
            ),
            crit_rate: ex.crit_rate,
            crit_resist: ex.crit_resist,
            crit_dmg: ex.crit_dmg,
            crit_def: ex.crit_def,
            add_dmg: ex.add_dmg,
            drop_dmg: ex.drop_dmg,
            ex_point: entity.ex_point.unwrap_or_default(),
            ex_skill: entity.ex_skill.unwrap_or_default(),
            ex_skill_level: entity.ex_skill_level.unwrap_or_default(),
            skill_group1,
            skill_group2,
            passive_skills: entity.passive_skill.clone(),
            destiny_stone: entity.destiny_stone.unwrap_or_default(),
            destiny_rank: entity.destiny_rank.unwrap_or_default(),
            battle_tags: catalog.entity_battle_tags(
                entity.model_id.unwrap_or_default(),
                entity.destiny_stone.unwrap_or_default(),
                entity.destiny_rank.unwrap_or_default(),
            ),
            buffs: entity
                .buffs
                .iter()
                .map(|buff| TargetBuff::from_buff_info(catalog, buff))
                .collect(),
        })
    }

    pub(super) fn has_buff_type(&self, type_id: i32) -> bool {
        self.buffs
            .iter()
            .any(|buff| buff.id == type_id || buff.type_id == type_id)
    }

    pub(super) fn has_buff_status(&self, status: crate::engine::manager::buff::BuffStatus) -> bool {
        self.buffs.iter().any(|buff| buff.status == Some(status))
    }

    pub(super) fn has_buff_act_kind(
        &self,
        kind: crate::engine::skill::buff_act::registry::BuffActKind,
    ) -> bool {
        self.buffs.iter().any(|buff| buff.has_buff_act_kind(kind))
    }

    pub(super) fn buff_source_for_kind(
        &self,
        kind: crate::engine::skill::buff_act::registry::BuffActKind,
    ) -> Option<i64> {
        self.buffs
            .iter()
            .find(|buff| buff.has_buff_act_kind(kind))
            .map(|buff| buff.source_uid)
            .filter(|source_uid| *source_uid != 0)
    }

    pub(super) fn has_monster_label(&self, label: i32) -> bool {
        self.model_label == label || self.buffs.iter().any(|buff| buff.has_monster_label(label))
    }

    pub fn has_career(&self, career: i32) -> bool {
        self.careers.contains(&career)
    }

    pub fn shares_career_with(&self, other: &Self) -> bool {
        self.careers
            .iter()
            .any(|career| other.careers.contains(career))
    }
}

impl TargetBuff {
    fn from_buff_info(catalog: crate::catalog::BattleCatalog, buff: &BuffInfo) -> Self {
        let id = buff.buff_id.unwrap_or_default();
        let features = catalog.buff_feature_tokens(id);
        Self {
            id,
            type_id: buff.r#type.unwrap_or_else(|| catalog.buff_type_id(id)),
            status: catalog.buff_status(id),
            source_uid: buff.from_uid.unwrap_or_default(),
            features: features.clone(),
            act_kinds: features
                .iter()
                .filter_map(|feature| {
                    let opcode = feature.split('#').next()?.parse().ok()?;
                    Some(catalog.buff_act_definition(opcode)?.kind)
                })
                .collect(),
            monster_labels: features
                .iter()
                .filter_map(|feature| {
                    let mut values = feature
                        .split('#')
                        .filter_map(|value| value.parse::<i32>().ok());
                    let opcode = values.next()?;
                    (catalog.buff_act_definition(opcode)?.kind
                        == crate::engine::skill::buff_act::registry::BuffActKind::MonsterLabel)
                        .then(|| values.next())?
                })
                .collect(),
        }
    }

    fn has_buff_act_kind(
        &self,
        kind: crate::engine::skill::buff_act::registry::BuffActKind,
    ) -> bool {
        self.act_kinds.contains(&kind)
    }

    fn has_monster_label(&self, label: i32) -> bool {
        self.monster_labels.contains(&label)
    }
}

#[cfg(test)]
mod tests;
