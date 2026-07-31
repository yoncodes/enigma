use std::collections::HashMap;

use sonettobuf::{Fight, FightEntityInfo};

use super::entities;

pub const STANDARD_DAMAGE_RATE_PERMILLE: i32 = 200;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ToughnessState {
    pub value: i32,
    pub point: i32,
    pub segment: i32,
    pub max_point: i32,
    pub broken: bool,
    recovery_penalty: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToughnessChange {
    pub target_uid: i64,
    pub value_delta: i32,
    pub point_delta: i32,
    pub broken: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToughnessRecovery {
    pub target_uid: i64,
    pub value_delta: i32,
    pub point_delta: i32,
    pub config_effect: i32,
    pub team_type: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToughnessRecover {
    pub target_uid: i64,
    pub config_effect: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToughnessCommand {
    RecordBrokenDamage {
        target_uid: i64,
        damage: i32,
        rate_permille: i32,
    },
    Recover(ToughnessRecover),
}

#[derive(Debug, Clone, Default)]
pub struct ToughnessManager {
    states: HashMap<i64, ToughnessState>,
}

impl ToughnessManager {
    pub fn seed(&mut self, fight: &Fight) {
        self.states.clear();
        for entity in entities(fight).chain(
            fight
                .attacker
                .iter()
                .chain(fight.defender.iter())
                .filter_map(|team| team.assist_boss.as_ref()),
        ) {
            self.register(entity);
        }
    }

    pub fn register(&mut self, entity: &FightEntityInfo) {
        let (Some(uid), Some(value), Some(point)) =
            (entity.uid, entity.toughness_value, entity.toughness_point)
        else {
            return;
        };
        if value <= 0 || point <= 0 {
            return;
        }
        self.states.insert(
            uid,
            ToughnessState {
                value,
                point,
                segment: configured_segment(entity).unwrap_or(value).max(1),
                max_point: point,
                broken: entity.is_broken.unwrap_or(false),
                recovery_penalty: 0,
            },
        );
    }

    pub fn damage(
        &mut self,
        target_uid: i64,
        damage: i32,
        rate_permille: i32,
    ) -> Option<ToughnessChange> {
        let state = self.states.get_mut(&target_uid)?;
        if state.broken || damage <= 0 || rate_permille <= 0 {
            return None;
        }
        let reduction = (i64::from(damage) * i64::from(rate_permille) / 1000)
            .clamp(0, i64::from(i32::MAX)) as i32;
        if reduction == 0 {
            return None;
        }

        let before_total = i64::from(state.point.saturating_sub(1)) * i64::from(state.segment)
            + i64::from(state.value);
        let after_total = (before_total - i64::from(reduction)).max(0);
        let before_value = state.value;
        let before_point = state.point;
        if after_total == 0 {
            state.value = 0;
            state.point = 0;
            state.broken = true;
            state.recovery_penalty = 0;
        } else {
            state.point = ((after_total - 1) / i64::from(state.segment) + 1) as i32;
            state.value = (after_total
                - i64::from(state.point.saturating_sub(1)) * i64::from(state.segment))
                as i32;
        }

        Some(ToughnessChange {
            target_uid,
            value_delta: before_value - state.value,
            point_delta: before_point - state.point,
            broken: state.broken,
        })
    }

    pub fn is_broken(&self, target_uid: i64) -> bool {
        self.states
            .get(&target_uid)
            .is_some_and(|state| state.broken)
    }

    pub fn execute(
        &mut self,
        command: ToughnessCommand,
        team_type: i32,
    ) -> Option<ToughnessRecovery> {
        match command {
            ToughnessCommand::RecordBrokenDamage {
                target_uid,
                damage,
                rate_permille,
            } => {
                self.record_broken_damage(target_uid, damage, rate_permille);
                None
            }
            ToughnessCommand::Recover(command) => self.recover(command, team_type),
        }
    }

    fn record_broken_damage(&mut self, target_uid: i64, damage: i32, rate_permille: i32) {
        let Some(state) = self
            .states
            .get_mut(&target_uid)
            .filter(|state| state.broken)
        else {
            return;
        };
        let penalty = (i64::from(damage.max(0)) * i64::from(rate_permille.max(0)) / 1000)
            .clamp(0, i64::from(i32::MAX)) as i32;
        state.recovery_penalty = state.recovery_penalty.saturating_add(penalty);
    }

    fn recover(&mut self, command: ToughnessRecover, team_type: i32) -> Option<ToughnessRecovery> {
        let state = self.states.get_mut(&command.target_uid)?;
        if !state.broken {
            return None;
        }
        let max_total = i64::from(state.max_point.saturating_sub(1)) * i64::from(state.segment)
            + i64::from(state.segment);
        let after_total = (max_total - i64::from(state.recovery_penalty)).max(0);
        let before_value = state.value;
        let before_point = state.point;
        state.point = if after_total == 0 {
            0
        } else {
            ((after_total - 1) / i64::from(state.segment) + 1) as i32
        };
        state.value = (after_total
            - i64::from(state.point.saturating_sub(1)) * i64::from(state.segment))
            as i32;
        state.broken = false;
        state.recovery_penalty = 0;
        Some(ToughnessRecovery {
            target_uid: command.target_uid,
            value_delta: state.value - before_value,
            point_delta: state.point - before_point,
            config_effect: command.config_effect,
            team_type,
        })
    }

    pub fn sync_entity(&self, entity: &mut FightEntityInfo) {
        let Some(state) = entity.uid.and_then(|uid| self.states.get(&uid)) else {
            return;
        };
        entity.toughness_value = Some(state.value);
        entity.toughness_point = Some(state.point);
        entity.is_broken = Some(state.broken);
    }
}

fn configured_segment(entity: &FightEntityInfo) -> Option<i32> {
    configured_toughness(entity.model_id?, entity.attr.as_ref()?.hp?).map(|(segment, _)| segment)
}

pub(crate) fn configured_toughness(model_id: i32, max_hp: i32) -> Option<(i32, i32)> {
    let monster = config::configs::try_get()?.monster.get(model_id)?;
    let mut parts = monster.toughness.split('#');
    let value = parts.next()?.parse::<i32>().ok()?;
    let points = parts.next()?.parse::<i32>().ok()?;
    let segment = match parts.next()?.parse::<i32>().ok()? {
        0 => value,
        1 => (i64::from(max_hp) * i64::from(value) / 1000).clamp(0, i64::from(i32::MAX)) as i32,
        _ => return None,
    };
    (segment > 0 && points > 0).then_some((segment, points))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_guard_uses_the_monster_hp_segment_definition() {
        crate::test_support::init_config();

        assert_eq!(
            configured_toughness(1_163_857_112, 1_431_503),
            Some((143_150, 3))
        );
    }

    #[test]
    fn damage_crosses_segments_and_reports_wire_deltas() {
        let mut manager = ToughnessManager::default();
        manager.states.insert(
            -1,
            ToughnessState {
                value: 80_984,
                point: 3,
                segment: 143_150,
                max_point: 3,
                broken: false,
                recovery_penalty: 0,
            },
        );

        assert_eq!(
            manager.damage(-1, 234_354, 1000),
            Some(ToughnessChange {
                target_uid: -1,
                value_delta: -51_946,
                point_delta: 2,
                broken: false,
            })
        );
        assert_eq!(
            manager.damage(-1, 132_930, 1000),
            Some(ToughnessChange {
                target_uid: -1,
                value_delta: 132_930,
                point_delta: 1,
                broken: true,
            })
        );
    }

    #[test]
    fn recorded_break_damage_reduces_immediate_recovery() {
        let mut manager = ToughnessManager::default();
        manager.states.insert(
            -1,
            ToughnessState {
                value: 0,
                point: 0,
                segment: 101_500,
                max_point: 3,
                broken: true,
                recovery_penalty: 0,
            },
        );
        manager.execute(
            ToughnessCommand::RecordBrokenDamage {
                target_uid: -1,
                damage: 203_000,
                rate_permille: STANDARD_DAMAGE_RATE_PERMILLE,
            },
            2,
        );

        assert_eq!(
            manager.execute(
                ToughnessCommand::Recover(ToughnessRecover {
                    target_uid: -1,
                    config_effect: 60_287,
                }),
                2,
            ),
            Some(ToughnessRecovery {
                target_uid: -1,
                value_delta: 60_900,
                point_delta: 3,
                config_effect: 60_287,
                team_type: 2,
            })
        );
    }
}
