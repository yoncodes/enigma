use std::collections::HashMap;

use sonettobuf::{Fight, FightEntityInfo};

use super::entities;

mod overflow;

use overflow::OverflowRecord;

pub const STANDARD_DAMAGE_RATE_PERMILLE: i32 = 200;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ToughnessState {
    pub value: i32,
    pub point: i32,
    pub segment_value: i32,
    pub max_point: i32,
    pub team_type: i32,
    pub broken: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToughnessChange {
    pub target_uid: i64,
    pub before: ToughnessState,
    pub reduction: i32,
    pub value_delta: i32,
    pub point_delta: i32,
    pub after: ToughnessState,
    pub broke: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToughnessRecover {
    pub origin: crate::engine::skill::rule::CommandOrigin,
    pub target_uid: i64,
    pub config_effect: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToughnessRecord {
    pub target_uid: i64,
    pub damage: i32,
    pub rate_permille: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToughnessRecovery {
    pub target_uid: i64,
    pub point: i32,
    pub value: i32,
    pub config_effect: i32,
    pub team_type: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToughnessReset {
    pub target_uid: i64,
    pub before: Option<ToughnessState>,
    pub after: Option<ToughnessState>,
}

#[derive(Debug, Clone, Default)]
pub struct ToughnessManager {
    states: HashMap<i64, ToughnessState>,
    recovery_penalties: HashMap<i64, i32>,
    overflow: OverflowRecord,
}

impl ToughnessManager {
    pub fn seed_with_game_data(&mut self, game_data: &config::GameDB, fight: &Fight) {
        self.seed_from(fight, |entity| {
            entity
                .model_id
                .zip(entity.attr.as_ref().and_then(|attr| attr.hp))
                .and_then(|(model_id, max_hp)| {
                    crate::catalog::configured_monster_toughness(game_data, model_id, max_hp)
                })
        });
    }

    pub(crate) fn seed_configured(
        &mut self,
        catalog: crate::catalog::BattleCatalog,
        fight: &Fight,
    ) {
        self.seed_from(fight, |entity| {
            entity
                .model_id
                .zip(entity.attr.as_ref().and_then(|attr| attr.hp))
                .and_then(|(model_id, max_hp)| catalog.monster_toughness(model_id, max_hp))
        });
    }

    fn seed_from(
        &mut self,
        fight: &Fight,
        mut configured: impl FnMut(&FightEntityInfo) -> Option<(i32, i32)>,
    ) {
        self.states.clear();
        self.recovery_penalties.clear();
        self.overflow = OverflowRecord::default();
        for entity in entities(fight) {
            let values = configured(entity);
            self.register_from(entity, values);
        }
    }

    #[cfg(test)]
    pub fn seed(&mut self, fight: &Fight) {
        self.seed_with_game_data(crate::test_support::game_data(), fight);
    }

    pub fn register_with_game_data(
        &mut self,
        game_data: &config::GameDB,
        entity: &FightEntityInfo,
    ) {
        let configured = entity
            .model_id
            .zip(entity.attr.as_ref().and_then(|attr| attr.hp))
            .and_then(|(model_id, max_hp)| {
                crate::catalog::configured_monster_toughness(game_data, model_id, max_hp)
            });
        self.register_from(entity, configured);
    }

    pub(crate) fn register_configured(
        &mut self,
        catalog: crate::catalog::BattleCatalog,
        entity: &FightEntityInfo,
    ) {
        let configured = entity
            .model_id
            .zip(entity.attr.as_ref().and_then(|attr| attr.hp))
            .and_then(|(model_id, max_hp)| catalog.monster_toughness(model_id, max_hp));
        self.register_from(entity, configured);
    }

    fn register_from(&mut self, entity: &FightEntityInfo, configured: Option<(i32, i32)>) {
        let Some(uid) = entity.uid else { return };
        let value = entity.toughness_value.unwrap_or_default().max(0);
        let point = entity.toughness_point.unwrap_or_default().max(0);
        let segment_value = configured.map_or(value, |values| values.0);
        let max_point = configured.map_or(point, |values| values.1);
        if segment_value <= 0 {
            self.states.remove(&uid);
            return;
        }
        self.states.insert(
            uid,
            ToughnessState {
                value,
                point,
                segment_value,
                max_point,
                team_type: entity.team_type.unwrap_or_default(),
                broken: entity.is_broken.unwrap_or_default(),
            },
        );
    }

    pub fn get(&self, uid: i64) -> Option<ToughnessState> {
        self.states.get(&uid).copied()
    }

    pub fn reduce(
        &mut self,
        target_uid: i64,
        damage: i32,
        stronger_afflatus: bool,
    ) -> Option<ToughnessChange> {
        let state = self.states.get_mut(&target_uid)?;
        if state.broken || state.point <= 0 || damage <= 0 {
            return None;
        }
        let reduction = if stronger_afflatus {
            damage
        } else {
            damage / 5
        };
        if reduction <= 0 {
            return None;
        }

        let before = *state;
        let total =
            i64::from(before.point - 1) * i64::from(before.segment_value) + i64::from(before.value);
        let after_total = (total - i64::from(reduction)).max(0);
        if after_total == 0 {
            state.value = 0;
            state.point = 0;
            state.broken = true;
        } else {
            state.point = ((after_total + i64::from(state.segment_value) - 1)
                / i64::from(state.segment_value)) as i32;
            state.value =
                (after_total - i64::from(state.point - 1) * i64::from(state.segment_value)) as i32;
        }

        Some(ToughnessChange {
            target_uid,
            before,
            reduction,
            value_delta: before.value - state.value,
            point_delta: before.point - state.point,
            after: *state,
            broke: !before.broken && state.broken,
        })
    }

    pub(crate) fn record_crossing(&mut self, change: ToughnessChange) {
        self.overflow.record(change);
    }

    pub fn is_broken(&self, target_uid: i64) -> bool {
        self.states
            .get(&target_uid)
            .is_some_and(|state| state.broken)
    }

    pub fn record_broken_damage(&mut self, command: ToughnessRecord) {
        if !self.is_broken(command.target_uid) {
            return;
        }
        let penalty = (i64::from(command.damage.max(0)) * i64::from(command.rate_permille.max(0))
            / 1000)
            .clamp(0, i64::from(i32::MAX)) as i32;
        let recorded = self
            .recovery_penalties
            .entry(command.target_uid)
            .or_default();
        *recorded = recorded.saturating_add(penalty);
    }

    pub fn recover(&mut self, command: ToughnessRecover) -> Option<ToughnessRecovery> {
        let state = self.states.get_mut(&command.target_uid)?;
        if !state.broken {
            return None;
        }
        let max_total = i64::from(state.max_point) * i64::from(state.segment_value);
        let penalty = self.recovery_penalties.remove(&command.target_uid);
        let overflow = self
            .overflow
            .settle(command.target_uid, state.segment_value, penalty);
        let minimum = i64::from(overflow > 0 && max_total > 0);
        let after_total = max_total
            .saturating_sub(i64::from(penalty.unwrap_or_default()))
            .saturating_sub(overflow)
            .max(minimum);
        state.point = if after_total == 0 {
            0
        } else {
            ((after_total - 1) / i64::from(state.segment_value) + 1) as i32
        };
        state.value = if state.point == 0 {
            0
        } else {
            (after_total - i64::from(state.point - 1) * i64::from(state.segment_value)) as i32
        };
        state.broken = false;
        Some(ToughnessRecovery {
            target_uid: command.target_uid,
            point: state.point,
            value: state.value,
            config_effect: command.config_effect,
            team_type: state.team_type,
        })
    }

    pub fn sync_entity(&self, entity: &mut FightEntityInfo) {
        let Some(uid) = entity.uid else { return };
        let Some(state) = self.get(uid) else { return };
        entity.toughness_value = Some(state.value);
        entity.toughness_point = Some(state.point);
        entity.is_broken = Some(state.broken);
    }

    pub(crate) fn settle_transform_reset(
        &mut self,
        target_uid: i64,
        before: Option<ToughnessState>,
    ) -> ToughnessReset {
        self.recovery_penalties.remove(&target_uid);
        self.overflow.clear(target_uid);
        ToughnessReset {
            target_uid,
            before,
            after: self.get(target_uid),
        }
    }
}

#[cfg(test)]
mod tests;
