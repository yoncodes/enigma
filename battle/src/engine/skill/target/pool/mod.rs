use std::collections::HashMap;

use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use crate::engine::manager::BattleManagers;

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
    source_uid: i64,
    features: Vec<&'static str>,
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
    pub attacker_main: Vec<TargetEntity>,
    pub attacker_all: Vec<TargetEntity>,
    pub defender_main: Vec<TargetEntity>,
    pub defender_all: Vec<TargetEntity>,
    boss_model_ids: Vec<i32>,
    assist_bosses: HashMap<i32, i64>,
    assist_boss_skills: Vec<(i64, Vec<i32>)>,
    virtual_entities: Vec<TargetEntity>,
    teams: HashMap<i64, i32>,
}

impl TargetPool {
    pub fn from_fight(fight: &Fight) -> Self {
        let mut pool = Self {
            boss_model_ids: configured_boss_model_ids(fight),
            ..Self::default()
        };
        if let Some(team) = &fight.attacker {
            pool.teams
                .extend(team_identities(team).filter_map(|entity| entity.uid.map(|uid| (uid, 1))));
            pool.attacker_main = alive_uids(&team.entitys);
            if let Some(uid) = team.assist_boss.as_ref().and_then(|entity| entity.uid) {
                pool.assist_bosses.insert(1, uid);
                pool.assist_boss_skills.push((
                    uid,
                    team.assist_boss
                        .as_ref()
                        .map(|entity| entity.passive_skill.clone())
                        .unwrap_or_default(),
                ));
                pool.virtual_entities.extend(
                    team.assist_boss
                        .as_ref()
                        .and_then(TargetEntity::from_fight_entity),
                );
            }
            pool.attacker_all = alive_uids(
                team.entitys
                    .iter()
                    .chain(&team.sub_entitys)
                    .chain(&team.sp_entitys),
            );
        }
        if let Some(team) = &fight.defender {
            pool.teams
                .extend(team_identities(team).filter_map(|entity| entity.uid.map(|uid| (uid, 2))));
            pool.defender_main = alive_uids(&team.entitys);
            if let Some(uid) = team.assist_boss.as_ref().and_then(|entity| entity.uid) {
                pool.assist_bosses.insert(2, uid);
                pool.assist_boss_skills.push((
                    uid,
                    team.assist_boss
                        .as_ref()
                        .map(|entity| entity.passive_skill.clone())
                        .unwrap_or_default(),
                ));
                pool.virtual_entities.extend(
                    team.assist_boss
                        .as_ref()
                        .and_then(TargetEntity::from_fight_entity),
                );
            }
            pool.defender_all = alive_uids(
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

    pub(crate) fn runtime_view(&self, managers: &BattleManagers) -> Self {
        self.runtime_view_including(managers, None)
    }

    pub(crate) fn runtime_view_including(
        &self,
        managers: &BattleManagers,
        included_uid: Option<i64>,
    ) -> Self {
        let mut pool = self.clone();
        for entities in [
            &mut pool.attacker_main,
            &mut pool.attacker_all,
            &mut pool.defender_main,
            &mut pool.defender_all,
        ] {
            entities.retain_mut(|entity| {
                if let Some(identity) = managers
                    .entity
                    .snapshot(entity.uid)
                    .as_ref()
                    .and_then(TargetEntity::from_fight_entity)
                {
                    *entity = identity;
                }
                entity.current_hp = managers.hp.current(entity.uid);
                entity.max_hp = managers.hp.max(entity.uid);
                entity.ex_point = managers.ex_point.get(entity.uid);
                entity.buffs = managers
                    .buff
                    .active_for(entity.uid)
                    .map(TargetBuff::from_buff_info)
                    .collect();
                entity.current_hp > 0 || included_uid == Some(entity.uid)
            });
        }
        for entity in &mut pool.virtual_entities {
            if let Some(identity) = managers
                .entity
                .snapshot(entity.uid)
                .as_ref()
                .and_then(TargetEntity::from_fight_entity)
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
                .map(TargetBuff::from_buff_info)
                .collect();
        }
        pool
    }

    pub fn entity(&self, uid: i64) -> Option<&TargetEntity> {
        self.entities()
            .chain(self.virtual_entities.iter())
            .find(|entity| entity.uid == uid)
    }

    pub fn skill_slot(&self, source_uid: i64, skill_id: i32) -> i32 {
        let Some(source) = self.entity(source_uid) else {
            return -1;
        };
        let effect_id = crate::engine::skill::effect::catalog::configured_effect_id(skill_id);
        if source.skill_group1.contains(&skill_id) || source.skill_group1.contains(&effect_id) {
            1
        } else if source.skill_group2.contains(&skill_id)
            || source.skill_group2.contains(&effect_id)
        {
            2
        } else if crate::engine::mechanic::card::CardMechanic.is_ultimate_skill(skill_id, source) {
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

fn configured_boss_model_ids(fight: &Fight) -> Vec<i32> {
    let Some(db) = config::try_get() else {
        return Vec::new();
    };
    let Some(battle) = crate::engine::fight::configured_battle(fight) else {
        return Vec::new();
    };
    let wave = fight.cur_wave.unwrap_or(1).max(1) as usize - 1;
    battle
        .monster_group_ids
        .split('#')
        .filter_map(|id| id.parse::<i32>().ok())
        .nth(wave)
        .and_then(|group_id| db.monster_group.get(group_id))
        .into_iter()
        .flat_map(|group| group.boss_id.split('#'))
        .filter_map(|id| id.parse().ok())
        .collect()
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

fn alive_uids<'a>(entities: impl IntoIterator<Item = &'a FightEntityInfo>) -> Vec<TargetEntity> {
    entities
        .into_iter()
        .filter_map(TargetEntity::from_fight_entity)
        .collect()
}

impl TargetEntity {
    pub(crate) fn from_fight_entity(entity: &FightEntityInfo) -> Option<Self> {
        let current_hp = entity.current_hp.unwrap_or(1);
        if current_hp <= 0 {
            return None;
        }

        let attr = entity.attr.as_ref();
        let ex = base_ex_attributes(entity);
        Some(Self {
            uid: entity.uid?,
            level: entity.level.unwrap_or_default(),
            model_id: entity.model_id.unwrap_or_default(),
            model_label: model_label(entity.model_id.unwrap_or_default()),
            career: entity.career.unwrap_or_default(),
            careers: configured_careers(entity.career.unwrap_or_default()),
            weak_careers: entity.weak_careers.clone(),
            damage_type: damage_type(entity),
            position: entity.position.unwrap_or_default(),
            current_hp,
            max_hp: attr.and_then(|attr| attr.hp).unwrap_or(1),
            attack: attr.and_then(|attr| attr.attack).unwrap_or_default(),
            defense: attr.and_then(|attr| attr.defense).unwrap_or_default(),
            mdefense: attr.and_then(|attr| attr.mdefense).unwrap_or_default(),
            technic: attr.and_then(|attr| attr.technic).unwrap_or_default(),
            base_technic: base_technic(entity),
            crit_rate: ex.crit_rate,
            crit_resist: ex.crit_resist,
            crit_dmg: ex.crit_dmg,
            crit_def: ex.crit_def,
            add_dmg: ex.add_dmg,
            drop_dmg: ex.drop_dmg,
            ex_point: entity.ex_point.unwrap_or_default(),
            ex_skill: entity.ex_skill.unwrap_or_default(),
            ex_skill_level: entity.ex_skill_level.unwrap_or_default(),
            skill_group1: entity.skill_group1.clone(),
            skill_group2: entity.skill_group2.clone(),
            passive_skills: entity.passive_skill.clone(),
            destiny_stone: entity.destiny_stone.unwrap_or_default(),
            destiny_rank: entity.destiny_rank.unwrap_or_default(),
            battle_tags: battle_tags(entity),
            buffs: entity
                .buffs
                .iter()
                .map(TargetBuff::from_buff_info)
                .collect(),
        })
    }

    pub(super) fn has_buff_type(&self, type_id: i32) -> bool {
        self.buffs
            .iter()
            .any(|buff| buff.id == type_id || buff.type_id == type_id)
    }

    pub(super) fn has_buff_status(&self, status: crate::engine::manager::buff::BuffStatus) -> bool {
        self.buffs
            .iter()
            .any(|buff| crate::engine::manager::buff::configured_status(buff.id) == Some(status))
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

fn configured_careers(career: i32) -> Vec<i32> {
    config::try_get()
        .and_then(|db| db.fight_effect_group.get(career))
        .map(|group| {
            group
                .career
                .split('#')
                .filter_map(|value| value.parse().ok())
                .collect::<Vec<_>>()
        })
        .filter(|careers| !careers.is_empty())
        .unwrap_or_else(|| vec![career])
}

fn battle_tags(entity: &FightEntityInfo) -> Vec<i32> {
    let stone_tags = crate::engine::entity::destiny::Destiny::battle_tags(
        entity.destiny_stone.unwrap_or_default(),
        entity.destiny_rank.unwrap_or_default(),
    );
    let mut tags = stone_tags.unwrap_or_else(|| {
        config::try_get()
            .and_then(|db| db.character.get(entity.model_id.unwrap_or_default()))
            .map(|character| {
                character
                    .battle_tag
                    .split('#')
                    .filter_map(|tag| tag.parse().ok())
                    .collect()
            })
            .unwrap_or_default()
    });
    tags.sort_unstable();
    tags.dedup();
    tags
}

fn damage_type(entity: &FightEntityInfo) -> EntityDamageType {
    let Some(db) = config::try_get() else {
        return EntityDamageType::Unknown;
    };
    let model_id = entity.model_id.unwrap_or_default();
    if entity.entity_type == Some(1) {
        return EntityDamageType::from_wire(
            db.character
                .get(model_id)
                .map(|row| row.dmg_type)
                .unwrap_or_default(),
        );
    }
    EntityDamageType::from_wire(
        db.monster
            .get(model_id)
            .and_then(|monster| db.monster_skill_template.get(monster.skill_template))
            .map(|row| row.dmg_type)
            .unwrap_or_default(),
    )
}

fn base_technic(entity: &FightEntityInfo) -> i32 {
    let fallback = entity
        .attr
        .as_ref()
        .and_then(|attr| attr.technic)
        .unwrap_or_default();
    if entity.entity_type != Some(1) {
        return fallback;
    }
    let Some(db) = config::try_get() else {
        return fallback;
    };
    db.character_level
        .iter()
        .find(|row| {
            row.hero_id == entity.model_id.unwrap_or_default()
                && row.level == entity.level.unwrap_or_default()
        })
        .map(|row| row.technic)
        .unwrap_or(fallback)
}

#[derive(Debug, Clone, Copy)]
struct ExAttributes {
    crit_rate: i32,
    crit_resist: i32,
    crit_dmg: i32,
    crit_def: i32,
    add_dmg: i32,
    drop_dmg: i32,
}

impl Default for ExAttributes {
    fn default() -> Self {
        Self {
            crit_rate: 0,
            crit_resist: 0,
            crit_dmg: 1000,
            crit_def: 0,
            add_dmg: 0,
            drop_dmg: 0,
        }
    }
}

fn base_ex_attributes(entity: &FightEntityInfo) -> ExAttributes {
    let Some(db) = config::try_get() else {
        return ExAttributes::default();
    };
    let model_id = entity.model_id.unwrap_or_default();
    if entity.entity_type == Some(1) {
        return db
            .character_level
            .iter()
            .find(|row| row.hero_id == model_id && row.level == entity.level.unwrap_or_default())
            .map(|row| ExAttributes {
                crit_rate: row.cri,
                crit_resist: row.recri,
                crit_dmg: row.cri_dmg,
                crit_def: row.cri_def,
                add_dmg: row.add_dmg,
                drop_dmg: row.drop_dmg,
            })
            .unwrap_or_default();
    }
    let Some(monster) = db.monster.get(model_id) else {
        return ExAttributes::default();
    };
    if let Some(stats) = crate::engine::entity::stats::monster_instance_ex_stats(
        model_id,
        entity.level.unwrap_or_default(),
    ) {
        return ExAttributes {
            crit_rate: stats.cri,
            crit_resist: stats.recri,
            crit_dmg: stats.cri_dmg,
            crit_def: stats.cri_def,
            add_dmg: stats.add_dmg,
            drop_dmg: stats.drop_dmg,
        };
    }
    let level = entity.level.unwrap_or(monster.level_true);
    let template_id = if monster.template != 0 {
        monster.template
    } else {
        monster.id
    };
    db.monster_template
        .iter()
        .find(|row| row.template == template_id)
        .map(|row| ExAttributes {
            crit_rate: row.cri + row.cri_grow * level,
            crit_resist: row.recri + row.recri_grow * level,
            crit_dmg: row.cri_dmg + row.cri_dmg_grow * level,
            crit_def: row.cri_def + row.cri_def_grow * level,
            add_dmg: row.add_dmg + row.add_dmg_grow * level,
            drop_dmg: row.drop_dmg + row.drop_dmg_grow * level,
        })
        .unwrap_or_default()
}

impl TargetBuff {
    fn from_buff_info(buff: &BuffInfo) -> Self {
        let id = buff.buff_id.unwrap_or_default();
        let row = config::try_get().and_then(|db| db.skill_buff.get(id));
        Self {
            id,
            type_id: buff
                .r#type
                .or_else(|| row.map(|row| row.type_id))
                .unwrap_or_default(),
            source_uid: buff.from_uid.unwrap_or_default(),
            features: row
                .map(|row| split_features(&row.features))
                .unwrap_or_default(),
        }
    }

    fn has_buff_act_kind(
        &self,
        kind: crate::engine::skill::buff_act::registry::BuffActKind,
    ) -> bool {
        self.features.iter().any(|feature| {
            let Some(opcode) = feature
                .split('#')
                .next()
                .and_then(|value| value.parse::<i32>().ok())
            else {
                return false;
            };
            config::try_get()
                .and_then(|db| db.buff_act.get(opcode))
                .and_then(|act| crate::engine::skill::buff_act::registry::kind(opcode, &act.r#type))
                == Some(kind)
        })
    }

    fn has_monster_label(&self, label: i32) -> bool {
        self.features.iter().any(|feature| {
            let mut values = feature
                .split('#')
                .filter_map(|value| value.parse::<i32>().ok());
            let Some(act_id) = values.next() else {
                return false;
            };
            let is_label = config::try_get()
                .and_then(|db| db.buff_act.get(act_id))
                .and_then(|act| {
                    crate::engine::skill::buff_act::registry::kind(act.id, &act.r#type)
                })
                == Some(crate::engine::skill::buff_act::registry::BuffActKind::MonsterLabel);
            is_label && values.next() == Some(label)
        })
    }
}

pub(crate) fn model_label(model_id: i32) -> i32 {
    config::try_get()
        .and_then(|db| db.monster.get(model_id))
        .map(|monster| monster.label)
        .unwrap_or_default()
}

fn split_features(raw: &'static str) -> Vec<&'static str> {
    raw.split('|')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect()
}

#[cfg(test)]
mod tests;
