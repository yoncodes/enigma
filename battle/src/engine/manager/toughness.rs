use std::collections::HashMap;

use sonettobuf::{Fight, FightEntityInfo};

use super::entities;

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

#[derive(Debug, Clone, Default)]
pub struct ToughnessManager {
    states: HashMap<i64, ToughnessState>,
    recovery_penalties: HashMap<i64, i32>,
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
            value_delta: before.value - state.value,
            point_delta: before.point - state.point,
            after: *state,
            broke: !before.broken && state.broken,
        })
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
        let after_total = (max_total
            - i64::from(
                self.recovery_penalties
                    .remove(&command.target_uid)
                    .unwrap_or_default(),
            ))
        .max(0);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        manager::hp::{DamageEffectKind, HpCommand, HpDamage, HurtDamageFromType, HurtInfoData},
        skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
    };
    use sonettobuf::FightTeam;

    #[test]
    fn damage_crosses_segments_and_reports_wire_deltas() {
        let mut manager = ToughnessManager::default();
        manager.states.insert(
            -1,
            ToughnessState {
                value: 100,
                point: 3,
                segment_value: 100,
                max_point: 3,
                team_type: 2,
                broken: false,
            },
        );

        let first = manager.reduce(-1, 80, true).unwrap();
        assert_eq!(
            (first.value_delta, first.point_delta, first.broke),
            (80, 0, false)
        );

        let crossed = manager.reduce(-1, 30, true).unwrap();
        assert_eq!(
            (crossed.value_delta, crossed.point_delta, crossed.broke),
            (-70, 1, false)
        );

        let broken = manager.reduce(-1, 190, true).unwrap();
        assert_eq!(
            (broken.value_delta, broken.point_delta, broken.broke),
            (90, 2, true)
        );
        assert_eq!(manager.get(-1).unwrap().point, 0);
    }

    #[test]
    fn ordinary_afflatus_reduces_one_fifth_of_damage() {
        let mut manager = ToughnessManager::default();
        manager.states.insert(
            -1,
            ToughnessState {
                value: 100,
                point: 1,
                segment_value: 100,
                max_point: 1,
                team_type: 2,
                broken: false,
            },
        );

        let change = manager.reduce(-1, 99, false).unwrap();
        assert_eq!(change.value_delta, 19);
    }

    #[test]
    fn configured_percent_builds_each_guard_segment_from_max_hp() {
        crate::test_support::init_config();
        let mut manager = ToughnessManager::default();
        manager.register_with_game_data(
            crate::test_support::game_data(),
            &FightEntityInfo {
                uid: Some(-1),
                model_id: Some(109_350_003),
                toughness_value: Some(101_500),
                toughness_point: Some(4),
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(1_015_000),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );

        assert_eq!(manager.get(-1).unwrap().segment_value, 101_500);
        assert_eq!(manager.get(-1).unwrap().max_point, 4);
    }

    #[test]
    fn recovery_only_resets_a_broken_guard() {
        let mut manager = ToughnessManager::default();
        manager.states.insert(
            -1,
            ToughnessState {
                value: 0,
                point: 0,
                segment_value: 100,
                max_point: 3,
                team_type: 2,
                broken: true,
            },
        );
        let recovery = manager
            .recover(ToughnessRecover {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(60287, "ToughnessRecover"),
                },
                target_uid: -1,
                config_effect: 60287,
            })
            .unwrap();

        assert_eq!(
            (
                recovery.point,
                recovery.value,
                recovery.config_effect,
                recovery.team_type,
            ),
            (3, 100, 60287, 2)
        );
        assert_eq!(
            manager.get(-1),
            Some(ToughnessState {
                value: 100,
                point: 3,
                segment_value: 100,
                max_point: 3,
                team_type: 2,
                broken: false,
            })
        );
    }

    #[test]
    fn recorded_break_damage_reduces_the_recovered_segment() {
        let mut manager = ToughnessManager::default();
        manager.states.insert(
            -1,
            ToughnessState {
                value: 0,
                point: 0,
                segment_value: 101_500,
                max_point: 3,
                team_type: 2,
                broken: true,
            },
        );
        manager.record_broken_damage(ToughnessRecord {
            target_uid: -1,
            damage: 203_000,
            rate_permille: STANDARD_DAMAGE_RATE_PERMILLE,
        });

        let recovery = manager
            .recover(ToughnessRecover {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(60_287, "ToughnessRecover"),
                },
                target_uid: -1,
                config_effect: 60_287,
            })
            .unwrap();

        assert_eq!((recovery.point, recovery.value), (3, 60_900));
        assert_eq!(manager.get(-1).unwrap().value, 60_900);
    }

    #[test]
    fn skill_damage_commits_guard_break_with_the_hp_transaction() {
        let fight = Fight {
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(1_000),
                    attr: Some(sonettobuf::HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    toughness_value: Some(100),
                    toughness_point: Some(1),
                    is_broken: Some(false),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = super::super::BattleManagers::seeded(&fight);
        let changes = managers
            .execute_hp(HpCommand::Damage(HpDamage {
                origin: CommandOrigin {
                    domain: RuleDomain::Skill,
                    key: DefinitionKey::new(1, "SkillDamage"),
                },
                source_uid: 1,
                target_uid: -1,
                amount: 100,
                config_effect: 0,
                effect_kind: DamageEffectKind::Normal,
                assassinate: false,
                ignore_riposte: false,
                hurt: HurtInfoData {
                    from_uid: 1,
                    is_crit: false,
                    career_restraint: true,
                    reduce_hp: 0,
                    effect_id: 0,
                    skill_id: 1,
                    damage_from: HurtDamageFromType::Skill,
                    buff_act_id: 0,
                    buff_uid: 0,
                    hurt_effect_type: 0,
                    display_amount: None,
                },
            }))
            .unwrap();

        assert!(changes.toughness.unwrap().broke);
        assert!(changes.events().iter().any(|event| matches!(
            event,
            crate::engine::event::payload::BattleEvent::ToughnessBroken {
                source_uid: 1,
                target_uid: -1,
                skill_id: 1,
            }
        )));
        assert!(managers.toughness.get(-1).unwrap().broken);
    }
}
