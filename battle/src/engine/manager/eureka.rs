use std::collections::{HashMap, HashSet};

use sonettobuf::{Fight, FightEntityInfo, PowerInfo};

use crate::engine::{
    event::payload::{BattleEvent, EurekaChangeEvent},
    skill::rule::{CommandOrigin, DefinitionKey},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum PowerType {
    Eureka = 1,
    AssistBoss = 4,
    ZongMaoBossEnergy = 9,
}

impl PowerType {
    pub const fn id(self) -> i32 {
        self as i32
    }
}

pub const EUREKA_RESOURCE_ID: i32 = PowerType::Eureka.id();

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EurekaState {
    pub current: i32,
    pub max: i32,
}

impl EurekaState {
    pub fn is_full(self) -> bool {
        self.max > 0 && self.current >= self.max
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EurekaApplyResult {
    pub source_uid: i64,
    pub target_uid: i64,
    pub power_id: i32,
    pub before: i32,
    pub requested_delta: i32,
    pub applied_delta: i32,
    pub after: i32,
    pub overflow: i32,
    pub max: i32,
    pub effect_type: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EurekaChange {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub power_id: i32,
    pub delta: i32,
    pub effect_type: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EurekaProgress {
    pub owner_uid: i64,
    pub key: DefinitionKey,
    pub threshold: i32,
    pub amount: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EurekaCommand {
    Change(EurekaChange),
    ChangeMax {
        origin: CommandOrigin,
        source_uid: i64,
        target_uid: i64,
        power_id: i32,
        delta: i32,
    },
    ChangeByProgress {
        change: EurekaChange,
        progress: EurekaProgress,
    },
    ResetRound {
        owner_uids: Vec<i64>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EurekaChanges {
    Changed {
        origin: CommandOrigin,
        change: EurekaApplyResult,
    },
    RoundReset {
        owner_uids: Vec<i64>,
    },
    Max {
        origin: CommandOrigin,
        change: EurekaMaxApplyResult,
    },
    Unchanged {
        origin: CommandOrigin,
    },
}

impl EurekaChanges {
    pub fn events(&self) -> Vec<BattleEvent> {
        let Self::Changed { origin, change } = self else {
            return Vec::new();
        };
        if change.applied_delta == 0 && change.overflow == 0 {
            return Vec::new();
        }
        vec![BattleEvent::EurekaChanged(EurekaChangeEvent {
            origin: *origin,
            source_uid: change.source_uid,
            target_uid: change.target_uid,
            power_id: change.power_id,
            before: change.before,
            requested_delta: change.requested_delta,
            applied_delta: change.applied_delta,
            after: change.after,
            overflow: change.overflow,
        })]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EurekaCommandError {
    InvalidCommand,
    MissingTarget(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EurekaMaxApplyResult {
    pub source_uid: i64,
    pub target_uid: i64,
    pub power_id: i32,
    pub before: i32,
    pub delta: i32,
    pub after: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EurekaRoundChange {
    pub consumed: i32,
    pub overflow: i32,
}

#[derive(Debug, Clone, Default)]
pub struct EurekaManager {
    states: HashMap<(i64, i32), EurekaState>,
    round_changes: HashMap<(i64, i32), EurekaRoundChange>,
    owners: HashSet<i64>,
}

impl EurekaManager {
    pub(crate) fn execute_command(
        &mut self,
        command: EurekaCommand,
    ) -> Result<EurekaChanges, EurekaCommandError> {
        match command {
            EurekaCommand::Change(value) => {
                if value.target_uid == 0 || value.power_id <= 0 || value.delta == 0 {
                    return Err(EurekaCommandError::InvalidCommand);
                }
                if !self.owners.contains(&value.target_uid) {
                    return Err(EurekaCommandError::MissingTarget(value.target_uid));
                }
                Ok(EurekaChanges::Changed {
                    origin: value.origin,
                    change: self.add(
                        value.source_uid,
                        value.target_uid,
                        value.power_id,
                        value.delta,
                        value.effect_type,
                    ),
                })
            }
            EurekaCommand::ChangeMax {
                origin,
                source_uid,
                target_uid,
                power_id,
                delta,
            } => {
                if target_uid == 0 || power_id <= 0 || delta == 0 {
                    return Err(EurekaCommandError::InvalidCommand);
                }
                if !self.owners.contains(&target_uid) {
                    return Err(EurekaCommandError::MissingTarget(target_uid));
                }
                let before = self.get(target_uid, power_id).max;
                self.add_max(target_uid, power_id, delta);
                Ok(EurekaChanges::Max {
                    origin,
                    change: EurekaMaxApplyResult {
                        source_uid,
                        target_uid,
                        power_id,
                        before,
                        delta,
                        after: self.get(target_uid, power_id).max,
                    },
                })
            }
            EurekaCommand::ResetRound { owner_uids } => {
                self.clear_round_changes_for(owner_uids.iter().copied());
                Ok(EurekaChanges::RoundReset { owner_uids })
            }
            EurekaCommand::ChangeByProgress { .. } => {
                unreachable!("progress-gated Eureka commands are coordinated by BattleManagers")
            }
        }
    }

    pub fn seed(&mut self, fight: &Fight) {
        self.states.clear();
        self.round_changes.clear();
        self.owners.clear();
        for entity in resource_entities(fight) {
            self.register(entity);
        }
    }

    pub fn sync_fight(&self, fight: &mut Fight) {
        for entity in resource_entities_mut(fight) {
            let Some(uid) = entity.uid else { continue };
            self.sync_entity(uid, entity);
        }
    }

    pub fn register(&mut self, entity: &FightEntityInfo) {
        let Some(uid) = entity.uid else { return };
        self.owners.insert(uid);
        self.states.retain(|(owner_uid, _), _| *owner_uid != uid);
        for power in &entity.power_infos {
            let power_id = power.power_id.unwrap_or_default();
            self.states.insert(
                (uid, power_id),
                EurekaState {
                    current: power.num.unwrap_or_default(),
                    max: power.max.unwrap_or_default(),
                },
            );
        }
    }

    pub fn get(&self, uid: i64, power_id: i32) -> EurekaState {
        self.states
            .get(&(uid, power_id))
            .copied()
            .unwrap_or_default()
    }

    pub fn clear_round_changes_for(&mut self, uids: impl IntoIterator<Item = i64>) {
        for uid in uids {
            self.round_changes
                .retain(|(target_uid, _), _| *target_uid != uid);
        }
    }

    pub fn round_change(&self, uid: i64, power_id: i32) -> EurekaRoundChange {
        self.round_changes
            .get(&(uid, power_id))
            .copied()
            .unwrap_or_default()
    }

    pub fn set(&mut self, uid: i64, power_id: i32, value: i32) {
        let state = self.states.entry((uid, power_id)).or_default();
        state.current = value.clamp(0, state.max.max(0));
    }

    pub fn add_max(&mut self, uid: i64, power_id: i32, delta: i32) -> EurekaMaxApplyResult {
        let state = self.states.entry((uid, power_id)).or_default();
        let before = state.max;
        state.max = (state.max + delta).max(0);
        state.current = state.current.min(state.max);

        EurekaMaxApplyResult {
            source_uid: uid,
            target_uid: uid,
            power_id,
            before,
            delta,
            after: state.max,
        }
    }

    pub fn add(
        &mut self,
        source_uid: i64,
        target_uid: i64,
        power_id: i32,
        delta: i32,
        effect_type: i32,
    ) -> EurekaApplyResult {
        let state = self.states.entry((target_uid, power_id)).or_default();
        let before = state.current;
        let after = (before + delta).clamp(0, state.max.max(0));
        let applied_delta = after - before;
        let overflow = if delta > 0 {
            (delta - applied_delta).max(0)
        } else {
            0
        };
        state.current = after;
        let round = self
            .round_changes
            .entry((target_uid, power_id))
            .or_default();
        round.consumed += (-applied_delta).max(0);
        round.overflow += overflow;
        EurekaApplyResult {
            source_uid,
            target_uid,
            power_id,
            before,
            requested_delta: delta,
            applied_delta,
            after,
            overflow,
            max: state.max,
            effect_type,
        }
    }

    pub fn power_infos(&self, uid: i64) -> Vec<PowerInfo> {
        let mut values: Vec<_> = self
            .states
            .iter()
            .filter_map(|(&(state_uid, power_id), state)| {
                (state_uid == uid).then_some(PowerInfo {
                    power_id: Some(power_id),
                    num: Some(state.current),
                    max: Some(state.max),
                })
            })
            .collect();
        values.sort_by_key(|power| power.power_id.unwrap_or_default());
        values
    }

    pub fn sync_entity(&self, uid: i64, entity: &mut FightEntityInfo) {
        for power in &mut entity.power_infos {
            let power_id = power.power_id.unwrap_or_default();
            let state = self.get(uid, power_id);
            power.num = Some(state.current);
            power.max = Some(state.max);
        }
    }
}

fn resource_entities(fight: &Fight) -> impl Iterator<Item = &FightEntityInfo> {
    fight
        .attacker
        .iter()
        .chain(fight.defender.iter())
        .flat_map(|team| {
            team.entitys
                .iter()
                .chain(&team.sub_entitys)
                .chain(&team.sp_entitys)
                .chain(&team.sp_fight_entities)
                .chain(team.assist_boss.iter())
                .chain(team.emitter.iter())
                .chain(team.player_entity.iter())
                .chain(team.vorpalith.iter())
        })
}

fn resource_entities_mut(fight: &mut Fight) -> impl Iterator<Item = &mut FightEntityInfo> {
    fight
        .attacker
        .iter_mut()
        .chain(fight.defender.iter_mut())
        .flat_map(|team| {
            team.entitys
                .iter_mut()
                .chain(&mut team.sub_entitys)
                .chain(&mut team.sp_entitys)
                .chain(&mut team.sp_fight_entities)
                .chain(team.assist_boss.iter_mut())
                .chain(team.emitter.iter_mut())
                .chain(team.player_entity.iter_mut())
                .chain(team.vorpalith.iter_mut())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_and_resets_round_consumption_and_overflow() {
        let mut manager = EurekaManager::default();
        manager
            .states
            .insert((10, EUREKA_RESOURCE_ID), EurekaState { current: 4, max: 5 });

        manager.add(10, 10, EUREKA_RESOURCE_ID, -3, 0);
        manager.add(10, 10, EUREKA_RESOURCE_ID, 8, 0);

        assert_eq!(
            manager.round_change(10, EUREKA_RESOURCE_ID),
            EurekaRoundChange {
                consumed: 3,
                overflow: 4,
            }
        );
        manager.clear_round_changes_for([10]);
        assert_eq!(
            manager.round_change(10, EUREKA_RESOURCE_ID),
            EurekaRoundChange::default()
        );
    }

    #[test]
    fn assist_boss_power_is_seeded_and_synced_by_the_resource_owner() {
        let mut fight = Fight {
            attacker: Some(sonettobuf::FightTeam {
                assist_boss: Some(FightEntityInfo {
                    uid: Some(-1),
                    power_infos: vec![PowerInfo {
                        power_id: Some(PowerType::AssistBoss.id()),
                        num: Some(6),
                        max: Some(6),
                    }],
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut manager = EurekaManager::default();

        manager.seed(&fight);
        assert_eq!(
            manager.get(-1, PowerType::AssistBoss.id()),
            EurekaState { current: 6, max: 6 }
        );

        manager.set(-1, PowerType::AssistBoss.id(), 0);
        manager.sync_fight(&mut fight);
        assert_eq!(
            fight.attacker.unwrap().assist_boss.unwrap().power_infos[0].num,
            Some(0)
        );
    }

    #[test]
    fn command_publishes_overflow_even_when_the_value_is_capped() {
        use crate::engine::skill::rule::{CommandOrigin, DefinitionKey, RuleDomain};

        let mut manager = EurekaManager::default();
        manager.owners.insert(10);
        manager
            .states
            .insert((10, EUREKA_RESOURCE_ID), EurekaState { current: 5, max: 5 });

        let changes = manager
            .execute_command(EurekaCommand::Change(EurekaChange {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(20010, "AddPower"),
                },
                source_uid: 10,
                target_uid: 10,
                power_id: EUREKA_RESOURCE_ID,
                delta: 3,
                effect_type: 0,
            }))
            .unwrap();

        let EurekaChanges::Changed { change, .. } = changes.clone() else {
            unreachable!()
        };
        assert_eq!(change.applied_delta, 0);
        assert_eq!(change.overflow, 3);
        assert!(matches!(
            changes.events().as_slice(),
            [BattleEvent::EurekaChanged(event)] if event.overflow == 3
        ));
    }
}
