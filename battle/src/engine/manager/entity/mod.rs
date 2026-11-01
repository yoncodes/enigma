use std::collections::HashMap;

use sonettobuf::{Fight, FightEntityInfo, HeroAttribute};

use crate::engine::{
    entity::passive::{Passive, PassiveSourceKind},
    event::payload::BattleEvent,
    fight::defender::Defender,
    manager::hp::HpManager,
    skill::{condition::extra::ExtraSkillKind, rule::CommandOrigin},
};

const SPECIAL_POSITION: i32 = 5;
pub(crate) const MAX_COMBAT_POSITION: i32 = SPECIAL_POSITION - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RosterLane {
    Main,
    Reserve,
    Special,
    Replaced,
    Inactive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityOperation {
    SummonCombatant { model_id: i32, position: i32 },
    SummonSpecial { model_id: i32 },
    Transform { model_id: i32, parameters: [i32; 2] },
    ChangeCareer { career: i32 },
}

impl EntityOperation {
    pub(crate) fn resets_state(self) -> bool {
        matches!(
            self,
            Self::Transform {
                parameters: [_, 1],
                ..
            }
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityCommand {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub operation: EntityOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntitySkillCommand {
    pub origin: CommandOrigin,
    pub target_uid: i64,
    pub ultimate_kind: ExtraSkillKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntityChanges {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub entity: FightEntityInfo,
    pub operation: EntityOperation,
}

impl EntityChanges {
    pub fn events(&self) -> Vec<BattleEvent> {
        match self.operation {
            EntityOperation::SummonCombatant { .. } | EntityOperation::SummonSpecial { .. } => self
                .entity
                .uid
                .map(|target_uid| BattleEvent::EntityEntered { target_uid })
                .into_iter()
                .collect(),
            EntityOperation::Transform { .. } => {
                vec![BattleEvent::EntityTransformed {
                    target_uid: self.target_uid,
                }]
            }
            EntityOperation::ChangeCareer { .. } => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityCommandError {
    InvalidCommand,
    MissingSource,
    MissingModel,
}

#[derive(Debug, Clone, Default)]
pub struct EntityManager {
    teams: HashMap<i64, i32>,
    identities: HashMap<i64, FightEntityInfo>,
    roster_lanes: HashMap<i64, RosterLane>,
    reserve_order: HashMap<i32, Vec<i64>>,
    order: Vec<i64>,
    identity_order: Vec<i64>,
    replacements: HashMap<i64, FightEntityInfo>,
    passive_sources: HashMap<i64, HashMap<i32, PassiveSourceKind>>,
    ultimate_kinds: HashMap<i64, ExtraSkillKind>,
    next_special_uid: i64,
}

impl EntityManager {
    pub(crate) fn configured(catalog: crate::catalog::BattleCatalog, fight: &Fight) -> Self {
        let mut manager = Self::from_fight(
            catalog.game_data(),
            fight,
            catalog.defender_reservation_count(fight),
        );
        let mut rule_sources = Vec::new();
        for rule in crate::engine::fight::rules::configured_with_catalog(catalog, fight).into_iter().filter(|rule| {
            rule.rule_type != crate::engine::fight::rules::AdditionRuleType::FightSkill
        }) {
            for (&uid, entity) in &manager.identities {
                let owns_side = match rule.side {
                    crate::engine::fight::rules::BattleRuleSide::Attacker => {
                        manager.teams.get(&uid) == Some(&1)
                    }
                    crate::engine::fight::rules::BattleRuleSide::Defender => {
                        manager.teams.get(&uid) == Some(&2)
                    }
                    crate::engine::fight::rules::BattleRuleSide::Both => true,
                };
                if owns_side && entity.passive_skill.contains(&rule.skill_id) {
                    rule_sources.push((uid, rule.skill_id));
                }
            }
        }
        for (uid, skill_id) in rule_sources {
            manager
                .passive_sources
                .entry(uid)
                .or_default()
                .insert(skill_id, PassiveSourceKind::BattleRule);
        }
        manager
    }

    pub fn seed_with_game_data(game_data: &config::GameDB, fight: &Fight) -> Self {
        Self::from_fight(
            game_data,
            fight,
            crate::catalog::configured_defender_reservation_count(game_data, fight),
        )
    }

    fn from_fight(game_data: &config::GameDB, fight: &Fight, reserved_index: usize) -> Self {
        let entities = fight
            .attacker
            .iter()
            .flat_map(|team| {
                team.entitys
                    .iter()
                    .map(|entity| (1, RosterLane::Main, entity.clone()))
                    .chain(
                        team.sub_entitys
                            .iter()
                            .map(|entity| (1, RosterLane::Reserve, entity.clone())),
                    )
                    .chain(
                        team.sp_entitys
                            .iter()
                            .map(|entity| (1, RosterLane::Special, entity.clone())),
                    )
            })
            .chain(fight.defender.iter().flat_map(|team| {
                team.entitys
                    .iter()
                    .map(|entity| (2, RosterLane::Main, entity.clone()))
                    .chain(
                        team.sub_entitys
                            .iter()
                            .map(|entity| (2, RosterLane::Reserve, entity.clone())),
                    )
                    .chain(
                        team.sp_entitys
                            .iter()
                            .map(|entity| (2, RosterLane::Special, entity.clone())),
                    )
            }))
            .collect::<Vec<_>>();
        let teams = entities
            .iter()
            .filter_map(|(side, _, entity)| Some((entity.uid?, entity.team_type.unwrap_or(*side))))
            .collect();
        let identities = entities
            .iter()
            .filter_map(|(_, _, entity)| Some((entity.uid?, entity.clone())))
            .collect();
        let passive_sources = entities
            .iter()
            .filter_map(|(_, _, entity)| {
                let sources = psychube_passive_sources(game_data, entity);
                (!sources.is_empty()).then_some((entity.uid?, sources))
            })
            .collect();
        let roster_lanes = entities
            .iter()
            .filter_map(|(_, lane, entity)| Some((entity.uid?, *lane)))
            .collect();
        let order = entities
            .iter()
            .filter_map(|(_, _, entity)| entity.uid)
            .collect::<Vec<_>>();
        let mut reserve_order = HashMap::<i32, Vec<i64>>::new();
        for (side, lane, entity) in &entities {
            if *lane == RosterLane::Reserve
                && let Some(uid) = entity.uid
            {
                reserve_order
                    .entry(entity.team_type.unwrap_or(*side))
                    .or_default()
                    .push(uid);
            }
        }
        let largest_existing_index = entities
            .iter()
            .filter_map(|(_, _, entity)| entity.uid)
            .filter(|uid| *uid < 0)
            .map(i64::unsigned_abs)
            .max()
            .unwrap_or_default() as usize;
        Self {
            teams,
            identities,
            roster_lanes,
            reserve_order,
            identity_order: order.clone(),
            order,
            replacements: HashMap::new(),
            passive_sources,
            ultimate_kinds: HashMap::new(),
            next_special_uid: -(largest_existing_index.max(reserved_index) as i64 + 1),
        }
    }

    #[cfg(test)]
    pub fn seed(fight: &Fight) -> Self {
        Self::seed_with_game_data(crate::test_support::game_data(), fight)
    }

    #[cfg(test)]
    pub(crate) fn execute_command(
        &mut self,
        catalog: crate::catalog::BattleCatalog,
        command: EntityCommand,
        hp: &HpManager,
    ) -> Result<EntityChanges, EntityCommandError> {
        self.execute_command_with_capacity(catalog, command, hp, MAX_COMBAT_POSITION)
    }

    pub(crate) fn execute_command_with_capacity(
        &mut self,
        catalog: crate::catalog::BattleCatalog,
        command: EntityCommand,
        hp: &HpManager,
        combat_capacity: i32,
    ) -> Result<EntityChanges, EntityCommandError> {
        if command.source_uid == 0 || command.target_uid == 0 {
            return Err(EntityCommandError::InvalidCommand);
        }
        let entity = match command.operation {
            EntityOperation::SummonCombatant { model_id, position } => {
                if model_id <= 0
                    || !self.combat_position_available(
                        command.source_uid,
                        position,
                        hp,
                        combat_capacity,
                    )
                {
                    return Err(EntityCommandError::InvalidCommand);
                }
                let team_type = self
                    .teams
                    .get(&command.source_uid)
                    .copied()
                    .ok_or(EntityCommandError::MissingSource)?;
                let defeated_index = self.order.iter().position(|uid| {
                    self.teams.get(uid) == Some(&team_type)
                        && self.roster_lanes.get(uid) == Some(&RosterLane::Main)
                        && self
                            .identities
                            .get(uid)
                            .is_some_and(|entity| entity.position == Some(position))
                        && hp.current(*uid) <= 0
                });
                let uid = self.next_special_uid;
                let entity = Defender::build_monster(catalog, model_id, uid, position, team_type)
                    .map_err(|_| EntityCommandError::MissingModel)?;
                self.next_special_uid -= 1;
                self.teams.insert(uid, team_type);
                self.identities.insert(uid, entity.clone());
                self.roster_lanes.insert(uid, RosterLane::Main);
                self.order.push(uid);
                self.identity_order.push(uid);
                if let Some(defeated_index) = defeated_index {
                    let defeated_uid = self.order[defeated_index];
                    self.roster_lanes.insert(defeated_uid, RosterLane::Replaced);
                    let summoned_index = self.order.len() - 1;
                    self.order.swap(defeated_index, summoned_index);
                }
                entity
            }
            EntityOperation::SummonSpecial { model_id } => {
                if model_id <= 0 {
                    return Err(EntityCommandError::InvalidCommand);
                }
                let team_type = self
                    .teams
                    .get(&command.source_uid)
                    .copied()
                    .ok_or(EntityCommandError::MissingSource)?;
                let uid = self.next_special_uid;
                let entity =
                    Defender::build_monster(catalog, model_id, uid, SPECIAL_POSITION, team_type)
                        .map_err(|_| EntityCommandError::MissingModel)?;
                self.next_special_uid -= 1;
                self.teams.insert(uid, team_type);
                self.identities.insert(uid, entity.clone());
                self.roster_lanes.insert(uid, RosterLane::Special);
                self.order.push(uid);
                self.identity_order.push(uid);
                entity
            }
            EntityOperation::Transform {
                model_id,
                parameters,
            } => {
                if model_id <= 0 {
                    return Err(EntityCommandError::InvalidCommand);
                }
                let current = self
                    .identities
                    .get(&command.target_uid)
                    .ok_or(EntityCommandError::MissingSource)?
                    .clone();
                let mut entity = Defender::build_monster(
                    catalog,
                    model_id,
                    command.target_uid,
                    current.position.unwrap_or_default(),
                    current.team_type.unwrap_or_default(),
                )
                .map_err(|_| EntityCommandError::MissingModel)?;
                let intrinsic_identity = current.model_id.and_then(|current_model_id| {
                    Defender::build_monster(
                        catalog,
                        current_model_id,
                        command.target_uid,
                        current.position.unwrap_or_default(),
                        current.team_type.unwrap_or_default(),
                    )
                    .ok()
                });
                if let Some(intrinsic_identity) = intrinsic_identity.as_ref() {
                    apply_encounter_attribute_scale(&mut entity, &current, intrinsic_identity);
                }
                apply_transform_hp(&mut entity, hp.current(command.target_uid), parameters[0]);
                let intrinsic_passives = intrinsic_identity
                    .map(|identity| identity.passive_skill)
                    .unwrap_or_default();
                entity.passive_skill = replace_intrinsic_passives(
                    &current.passive_skill,
                    &intrinsic_passives,
                    &entity.passive_skill,
                );
                entity.user_id = current.user_id;
                entity.ex_point = current.ex_point;
                entity.expoint_max_add = current.expoint_max_add;
                entity.shield_value = current.shield_value;
                if !command.operation.resets_state() {
                    entity.buffs = current.buffs.clone();
                }
                entity.power_infos = current.power_infos.clone();
                self.identities.insert(command.target_uid, entity.clone());
                self.replacements.insert(command.target_uid, entity.clone());
                entity
            }
            EntityOperation::ChangeCareer { career } => {
                if !(1..=6).contains(&career) {
                    return Err(EntityCommandError::InvalidCommand);
                }
                let mut entity = self
                    .identities
                    .get(&command.target_uid)
                    .ok_or(EntityCommandError::MissingSource)?
                    .clone();
                entity.career = Some(career);
                self.identities.insert(command.target_uid, entity.clone());
                self.replacements.insert(command.target_uid, entity.clone());
                entity
            }
        };

        Ok(EntityChanges {
            origin: command.origin,
            source_uid: command.source_uid,
            target_uid: command.target_uid,
            entity,
            operation: command.operation,
        })
    }

    pub(crate) fn update(&mut self, entity: FightEntityInfo) {
        let Some(uid) = entity.uid else { return };
        if let Some(sources) = self.passive_sources.get_mut(&uid) {
            sources.retain(|skill_id, _| entity.passive_skill.contains(skill_id));
        }
        self.identities.insert(uid, entity.clone());
        self.replacements.insert(uid, entity);
    }

    pub(crate) fn register(
        &mut self,
        catalog: crate::catalog::BattleCatalog,
        entity: &FightEntityInfo,
    ) {
        let lane = entity
            .uid
            .and_then(|uid| self.roster_lanes.get(&uid).copied())
            .unwrap_or(if entity.position == Some(SPECIAL_POSITION) {
                RosterLane::Special
            } else {
                RosterLane::Main
            });
        self.register_in_lane(catalog, entity, lane);
    }

    pub(crate) fn register_in_lane(
        &mut self,
        catalog: crate::catalog::BattleCatalog,
        entity: &FightEntityInfo,
        lane: RosterLane,
    ) {
        let (Some(uid), Some(team_type)) = (entity.uid, entity.team_type) else {
            return;
        };
        self.teams.insert(uid, team_type);
        self.roster_lanes.insert(uid, lane);
        if lane == RosterLane::Reserve {
            let reserves = self.reserve_order.entry(team_type).or_default();
            if !reserves.contains(&uid) {
                reserves.push(uid);
            }
        }
        if !self.identities.contains_key(&uid) {
            self.order.push(uid);
            self.identity_order.push(uid);
        }
        self.refresh_passive_sources(catalog.game_data(), entity);
        self.identities.insert(uid, entity.clone());
    }

    pub(crate) fn promote_reserves(
        &mut self,
        hp: &HpManager,
    ) -> Vec<crate::engine::fight::reserve::Promotion> {
        self.promote_reserves_matching(hp, |_| true)
    }

    pub(crate) fn promote_immediate_reserves(
        &mut self,
        hp: &HpManager,
        promotes_immediately: impl Fn(i32) -> bool,
    ) -> Vec<crate::engine::fight::reserve::Promotion> {
        self.promote_reserves_matching(hp, promotes_immediately)
    }

    fn promote_reserves_matching(
        &mut self,
        hp: &HpManager,
        promotes: impl Fn(i32) -> bool,
    ) -> Vec<crate::engine::fight::reserve::Promotion> {
        let mut promotions = Vec::new();
        for (defeated_uid, entering_uid, team_type) in self
            .pending_reserve_promotions(hp)
            .into_iter()
            .filter(|(_, _, team_type)| promotes(*team_type))
        {
            let Some(mut defeated) = self.identities.get(&defeated_uid).cloned() else {
                continue;
            };
            let Some(mut entering) = self.identities.get(&entering_uid).cloned() else {
                continue;
            };
            std::mem::swap(&mut defeated.position, &mut entering.position);
            let main_position = entering.position;
            if let (Some(defeated_index), Some(entering_index)) = (
                self.order.iter().position(|uid| *uid == defeated_uid),
                self.order.iter().position(|uid| *uid == entering_uid),
            ) {
                self.order.swap(defeated_index, entering_index);
            }
            let reserves = self.reserve_order.entry(team_type).or_default();
            reserves.retain(|uid| *uid != entering_uid && *uid != defeated_uid);
            reserves.push(defeated_uid);
            self.identities.insert(defeated_uid, defeated.clone());
            self.identities.insert(entering_uid, entering.clone());
            self.roster_lanes.insert(defeated_uid, RosterLane::Reserve);
            self.roster_lanes.insert(entering_uid, RosterLane::Main);
            promotions.push(crate::engine::fight::reserve::Promotion {
                defeated_uid,
                entering_uid,
                position: main_position.unwrap_or_default(),
                team_type,
                entering,
            });
        }
        promotions
    }

    pub(crate) fn has_pending_reserve_promotion(&self, uid: i64, hp: &HpManager) -> bool {
        if hp.current(uid) > 0 || self.roster_lanes.get(&uid) != Some(&RosterLane::Main) {
            return false;
        }
        let Some(&team) = self.teams.get(&uid) else { return false; };
        if !matches!(team, 1 | 2) { return false; }
        let Some(index) = self.order.iter().copied()
            .filter(|candidate| self.teams.get(candidate) == Some(&team)
                && self.roster_lanes.get(candidate) == Some(&RosterLane::Main)
                && hp.current(*candidate) <= 0)
            .position(|candidate| candidate == uid)
        else { return false; };
        self.reserve_order.get(&team).into_iter().flatten().copied()
            .filter(|candidate| self.roster_lanes.get(candidate) == Some(&RosterLane::Reserve)
                && hp.current(*candidate) > 0)
            .nth(index).is_some()
    }

    fn pending_reserve_promotions(&self, hp: &HpManager) -> Vec<(i64, i64, i32)> {
        let mut promotions = Vec::new();
        for team in [1, 2] {
            let mut reserves = self.reserve_order.get(&team).into_iter().flatten().copied()
                .filter(|uid| self.roster_lanes.get(uid) == Some(&RosterLane::Reserve)
                    && hp.current(*uid) > 0);
            let Some(mut entering_uid) = reserves.next() else { continue; };
            for defeated_uid in self.order.iter().copied()
                .filter(|uid| self.teams.get(uid) == Some(&team)
                    && self.roster_lanes.get(uid) == Some(&RosterLane::Main)
                    && hp.current(*uid) <= 0)
            {
                promotions.push((defeated_uid, entering_uid, team));
                let Some(next) = reserves.next() else { break; };
                entering_uid = next;
            }
        }
        promotions
    }

    pub(crate) fn replace_team_roster(
        &mut self,
        catalog: crate::catalog::BattleCatalog,
        team_type: i32,
        mains: &[FightEntityInfo],
        reserves: &[FightEntityInfo],
    ) {
        for uid in self
            .order
            .iter()
            .copied()
            .filter(|uid| self.teams.get(uid) == Some(&team_type))
            .filter(|uid| {
                matches!(
                    self.roster_lanes.get(uid),
                    Some(RosterLane::Main | RosterLane::Reserve)
                )
            })
            .collect::<Vec<_>>()
        {
            self.roster_lanes.insert(uid, RosterLane::Inactive);
        }
        self.reserve_order.insert(team_type, Vec::new());
        for entity in mains {
            self.register_in_lane(catalog, entity, RosterLane::Main);
        }
        for entity in reserves {
            self.register_in_lane(catalog, entity, RosterLane::Reserve);
        }
    }

    fn roster_uids(&self, team_type: i32, lane: RosterLane) -> Vec<i64> {
        if lane == RosterLane::Reserve {
            return self
                .reserve_order
                .get(&team_type)
                .into_iter()
                .flatten()
                .copied()
                .filter(|uid| self.roster_lanes.get(uid) == Some(&lane))
                .collect();
        }
        self.order
            .iter()
            .copied()
            .filter(|uid| self.teams.get(uid) == Some(&team_type))
            .filter(|uid| self.roster_lanes.get(uid) == Some(&lane))
            .collect()
    }

    pub(crate) fn ordered_uids(&self) -> impl Iterator<Item = i64> + '_ {
        self.order.iter().copied()
    }

    pub(crate) fn roster_lane(&self, uid: i64) -> Option<RosterLane> {
        self.roster_lanes.get(&uid).copied()
    }

    pub(crate) fn registration_ordered_uids(&self) -> impl Iterator<Item = i64> + '_ {
        self.identity_order.iter().copied()
    }

    pub(crate) fn passive_override(&self, uid: i64) -> Option<&[i32]> {
        self.replacements
            .get(&uid)
            .map(|entity| entity.passive_skill.as_slice())
    }

    pub(crate) fn execute_skill_command(
        &mut self,
        command: EntitySkillCommand,
    ) -> Result<(), EntityCommandError> {
        let entity = self
            .identities
            .get(&command.target_uid)
            .ok_or(EntityCommandError::MissingSource)?;
        if entity.ex_skill.unwrap_or_default() <= 0 {
            return Err(EntityCommandError::InvalidCommand);
        }
        self.ultimate_kinds
            .insert(command.target_uid, command.ultimate_kind);
        Ok(())
    }

    pub(crate) fn passive_skills(&self, uid: i64) -> Option<&[i32]> {
        self.replacements
            .get(&uid)
            .or_else(|| self.identities.get(&uid))
            .map(|entity| entity.passive_skill.as_slice())
    }

    pub(crate) fn passive_source(&self, uid: i64, skill_id: i32) -> PassiveSourceKind {
        self.passive_sources
            .get(&uid)
            .and_then(|sources| sources.get(&skill_id))
            .copied()
            .unwrap_or_default()
    }

    fn refresh_passive_sources(&mut self, game_data: &config::GameDB, entity: &FightEntityInfo) {
        let Some(uid) = entity.uid else { return };
        let sources = self.passive_sources.entry(uid).or_default();
        sources.retain(|skill_id, _| entity.passive_skill.contains(skill_id));
        sources.extend(psychube_passive_sources(game_data, entity));
    }

    pub(crate) fn passive_overrides(&self) -> impl Iterator<Item = (i64, &[i32])> {
        // Roster order, not hash order, so subscribers resolve the same way on every run.
        let mut overrides = self
            .replacements
            .iter()
            .map(|(&uid, entity)| (uid, entity.passive_skill.as_slice()))
            .collect::<Vec<_>>();
        overrides.sort_by_key(|(uid, _)| {
            (
                self.order
                    .iter()
                    .position(|ordered| ordered == uid)
                    .unwrap_or(usize::MAX),
                *uid,
            )
        });
        overrides.into_iter()
    }

    pub(crate) fn model_id(&self, uid: i64) -> Option<i32> {
        self.identities.get(&uid)?.model_id
    }

    pub(crate) fn snapshot(&self, uid: i64) -> Option<FightEntityInfo> {
        self.identities.get(&uid).cloned()
    }

    pub(crate) fn skill_kind(&self, uid: i64, skill_id: i32) -> Option<ExtraSkillKind> {
        let entity = self.identities.get(&uid)?;
        (entity.ex_skill == Some(skill_id))
            .then(|| self.ultimate_kinds.get(&uid).copied())
            .flatten()
    }

    pub(crate) fn team_type(&self, uid: i64) -> Option<i32> {
        self.teams.get(&uid).copied()
    }

    pub(crate) fn defeated_combatant_count(&self, team_type: i32, hp: &HpManager) -> usize {
        self.order
            .iter()
            .copied()
            .filter(|uid| self.teams.get(uid) == Some(&team_type))
            .filter(|uid| {
                matches!(
                    self.roster_lanes.get(uid),
                    Some(
                        RosterLane::Main
                            | RosterLane::Reserve
                            | RosterLane::Replaced
                            | RosterLane::Inactive
                    )
                )
            })
            .filter(|uid| hp.current(*uid) <= 0)
            .count()
    }

    pub(crate) fn first_open_combat_position(
        &self,
        source_uid: i64,
        hp: &crate::engine::manager::hp::HpManager,
        combat_capacity: i32,
    ) -> Option<i32> {
        (1..=combat_capacity).find(|position| {
            self.combat_position_available(source_uid, *position, hp, combat_capacity)
        })
    }

    pub(crate) fn combat_position_available(
        &self,
        source_uid: i64,
        position: i32,
        hp: &crate::engine::manager::hp::HpManager,
        combat_capacity: i32,
    ) -> bool {
        let Some(team_type) = self.teams.get(&source_uid).copied() else {
            return false;
        };
        if !(1..=combat_capacity).contains(&position) {
            return false;
        }
        !self
            .roster_uids(team_type, RosterLane::Main)
            .into_iter()
            .filter_map(|uid| self.identities.get(&uid))
            .any(|entity| {
                entity.position == Some(position)
                    && entity.uid.is_some_and(|uid| hp.current(uid) > 0)
            })
    }

    pub(crate) fn alive_combatants(
        &self,
        team_type: i32,
        hp: &crate::engine::manager::hp::HpManager,
    ) -> Vec<i64> {
        self.order
            .iter()
            .copied()
            .filter(|uid| {
                self.teams.get(uid) == Some(&team_type)
                    && self.is_main_combatant(*uid)
                    && hp.current(*uid) > 0
            })
            .collect()
    }

    pub(crate) fn is_main_combatant(&self, uid: i64) -> bool {
        self.roster_lanes.get(&uid) == Some(&RosterLane::Main)
            && self.identities.get(&uid).is_some_and(|entity| {
                entity
                    .position
                    .is_some_and(|position| (1..SPECIAL_POSITION).contains(&position))
            })
    }

    pub fn sync_to_fight(&self, fight: &mut Fight) {
        if let Some(team) = fight.attacker.as_mut() {
            self.sync_team(team, 1);
        }
        if let Some(team) = fight.defender.as_mut() {
            self.sync_team(team, 2);
        }
    }

    fn sync_team(&self, team: &mut sonettobuf::FightTeam, team_type: i32) {
        let current = team
            .entitys
            .iter()
            .chain(&team.sub_entitys)
            .chain(&team.sp_entitys)
            .filter_map(|entity| Some((entity.uid?, entity.clone())))
            .collect::<HashMap<_, _>>();
        team.entitys = self.roster_entities(team_type, RosterLane::Main, &current);
        team.sub_entitys = self.roster_entities(team_type, RosterLane::Reserve, &current);
        team.sp_entitys = self.roster_entities(team_type, RosterLane::Special, &current);
    }

    fn roster_entities(
        &self,
        team_type: i32,
        lane: RosterLane,
        current: &HashMap<i64, FightEntityInfo>,
    ) -> Vec<FightEntityInfo> {
        self.roster_uids(team_type, lane)
            .into_iter()
            .filter_map(|uid| {
                let identity = self.identities.get(&uid)?;
                let mut entity = self
                    .replacements
                    .get(&uid)
                    .or_else(|| current.get(&uid))
                    .unwrap_or(identity)
                    .clone();
                entity.position = identity.position;
                Some(entity)
            })
            .collect()
    }
}

fn psychube_passive_sources(
    game_data: &config::GameDB,
    entity: &FightEntityInfo,
) -> HashMap<i32, PassiveSourceKind> {
    entity
        .equips
        .iter()
        .chain(entity.trial_equip.iter())
        .filter_map(|equip| Some((equip.equip_id?, equip.refine_lv.map(|level| level.max(1)))))
        .flat_map(|(equip_id, refine_lv)| Passive::psychube_from(game_data, equip_id, refine_lv))
        .filter(|passive| entity.passive_skill.contains(&passive.skill_id))
        .map(|passive| (passive.skill_id, passive.source.kind))
        .collect()
}

fn replace_intrinsic_passives(
    current: &[i32],
    old_intrinsic: &[i32],
    replacement_intrinsic: &[i32],
) -> Vec<i32> {
    let Some(first_intrinsic) = current
        .iter()
        .position(|skill_id| old_intrinsic.contains(skill_id))
    else {
        let mut passives = replacement_intrinsic.to_vec();
        for skill_id in current.iter().copied() {
            if !passives.contains(&skill_id) {
                passives.push(skill_id);
            }
        }
        return passives;
    };

    let mut passives = Vec::with_capacity(current.len() + replacement_intrinsic.len());
    for (index, skill_id) in current.iter().copied().enumerate() {
        if index == first_intrinsic {
            passives.extend_from_slice(replacement_intrinsic);
        }
        if !old_intrinsic.contains(&skill_id) && !passives.contains(&skill_id) {
            passives.push(skill_id);
        }
    }
    passives
}

fn apply_transform_hp(entity: &mut FightEntityInfo, current_hp: i32, restore_permille: i32) {
    let max_hp = entity
        .attr
        .as_ref()
        .and_then(|attr| attr.hp)
        .unwrap_or_default()
        .max(0);
    let restored_hp = (i64::from(max_hp) * i64::from(restore_permille.max(0)) / 1_000)
        .clamp(0, i64::from(i32::MAX)) as i32;
    entity.current_hp = Some(current_hp.saturating_add(restored_hp).clamp(0, max_hp));

    if restore_permille == 1_000 {
        for attr in entity.attr.iter_mut().chain(&mut entity.base_attr) {
            attr.multi_hp_idx = Some(-1);
        }
    }
}

fn apply_encounter_attribute_scale(
    replacement: &mut FightEntityInfo,
    current: &FightEntityInfo,
    intrinsic_current: &FightEntityInfo,
) {
    let Some(current_base) = current.base_attr.as_ref().or(current.attr.as_ref()) else {
        return;
    };
    let Some(intrinsic_base) = intrinsic_current
        .base_attr
        .as_ref()
        .or(intrinsic_current.attr.as_ref())
    else {
        return;
    };
    let Some(replacement_base) = replacement.base_attr.as_ref().or(replacement.attr.as_ref())
    else {
        return;
    };
    let scaled = HeroAttribute {
        hp: scale_attribute(replacement_base.hp, current_base.hp, intrinsic_base.hp),
        attack: scale_attribute(
            replacement_base.attack,
            current_base.attack,
            intrinsic_base.attack,
        ),
        defense: scale_attribute(
            replacement_base.defense,
            current_base.defense,
            intrinsic_base.defense,
        ),
        mdefense: scale_attribute(
            replacement_base.mdefense,
            current_base.mdefense,
            intrinsic_base.mdefense,
        ),
        technic: scale_attribute(
            replacement_base.technic,
            current_base.technic,
            intrinsic_base.technic,
        ),
        multi_hp_idx: replacement_base.multi_hp_idx,
        multi_hp_num: replacement_base.multi_hp_num,
    };
    replacement.current_hp = scaled.hp;
    replacement.attr = Some(scaled);
    replacement.base_attr = Some(scaled);
}

fn scale_attribute(
    replacement: Option<i32>,
    current: Option<i32>,
    intrinsic: Option<i32>,
) -> Option<i32> {
    let (Some(replacement), Some(current), Some(intrinsic)) = (replacement, current, intrinsic)
    else {
        return replacement;
    };
    if intrinsic <= 0 || replacement == intrinsic {
        return Some(current);
    }
    Some(
        (i64::from(replacement) * i64::from(current) / i64::from(intrinsic))
            .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
    )
}

#[cfg(test)]
mod tests;
