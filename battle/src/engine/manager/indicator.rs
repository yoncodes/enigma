use std::collections::HashMap;

use crate::engine::skill::rule::output::EffectMarker;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
enum IndicatorOperation {
    Add = 60016,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum IndicatorId {
    BossRushScore = 4,
    ZongMaoTechniqueScore = 6,
    Rouge2Boss = 168,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndicatorCommand {
    pub indicator_id: i32,
    pub amount: i32,
    pub config_effect: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndicatorClearCommand {
    pub indicator_id: i32,
    pub config_effect: i32,
}

#[derive(Debug, Clone, Default)]
pub struct IndicatorManager {
    damage_targets: HashMap<i64, i32>,
    totals: HashMap<i32, i64>,
    activity_score: i64,
}

impl IndicatorManager {
    pub fn add(&mut self, command: IndicatorCommand) -> Option<EffectMarker> {
        if command.indicator_id <= 0 || command.amount == 0 {
            return None;
        }
        let total = self.totals.entry(command.indicator_id).or_default();
        *total = total.saturating_add(i64::from(command.amount));
        let effect_num = match command.indicator_id {
            id if id == IndicatorId::BossRushScore as i32
                || id == IndicatorId::ZongMaoTechniqueScore as i32
                || id == IndicatorId::Rouge2Boss as i32 =>
            {
                command.amount
            }
            _ => (*total).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        };
        self.activity_score = self
            .activity_score
            .saturating_add(i64::from(command.amount));
        Some(EffectMarker {
            target_uid: i64::from(command.indicator_id),
            effect_type: sonettobuf::effect_type_enum::EffectType::Indicatorchange as i32,
            effect_num,
            effect_num1: Some(1),
            config_effect: command.config_effect,
            reserve_id: None,
            reserve_str: None,
        })
    }

    pub fn clear(&mut self, command: IndicatorClearCommand) -> Option<EffectMarker> {
        (command.indicator_id > 0).then(|| {
            self.totals.remove(&command.indicator_id);
            EffectMarker {
                target_uid: i64::from(command.indicator_id),
                effect_type: sonettobuf::effect_type_enum::EffectType::Indicatorchange as i32,
                effect_num: 0,
                effect_num1: None,
                config_effect: command.config_effect,
                reserve_id: None,
                reserve_str: None,
            }
        })
    }

    pub fn track_damage(&mut self, indicator_id: IndicatorId, target_uid: i64) {
        self.damage_targets.insert(target_uid, indicator_id as i32);
    }

    pub fn record_damage(&mut self, target_uid: i64, amount: i32) -> Option<EffectMarker> {
        let indicator_id = *self.damage_targets.get(&target_uid)?;
        let amount = amount.max(0);
        if amount == 0 {
            return None;
        }
        *self.totals.entry(indicator_id).or_default() += i64::from(amount);
        self.activity_score = self.activity_score.saturating_add(i64::from(amount));
        Some(EffectMarker {
            target_uid: i64::from(indicator_id),
            effect_type: sonettobuf::effect_type_enum::EffectType::Indicatorchange as i32,
            effect_num: amount,
            effect_num1: Some(0),
            config_effect: IndicatorOperation::Add as i32,
            reserve_id: None,
            reserve_str: None,
        })
    }

    pub fn total(&self, indicator_id: IndicatorId) -> i32 {
        self.value(indicator_id as i32)
    }

    pub fn value(&self, indicator_id: i32) -> i32 {
        self.totals
            .get(&indicator_id)
            .copied()
            .unwrap_or_default()
            .clamp(0, i64::from(i32::MAX)) as i32
    }

    pub fn activity_score(&self) -> i32 {
        self.activity_score.clamp(0, i64::from(i32::MAX)) as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracked_target_damage_updates_and_emits_the_indicator() {
        let mut manager = IndicatorManager::default();
        manager.track_damage(IndicatorId::BossRushScore, -1);

        assert!(manager.record_damage(-2, 50).is_none());
        let marker = manager.record_damage(-1, 75).unwrap();

        assert_eq!(marker.target_uid, 4);
        assert_eq!(marker.effect_num, 75);
        assert_eq!(marker.effect_num1, Some(0));
        assert_eq!(marker.config_effect, 60016);
        assert_eq!(manager.total(IndicatorId::BossRushScore), 75);
        assert_eq!(manager.activity_score(), 75);
    }

    #[test]
    fn configured_score_indicators_emit_the_delta() {
        for indicator_id in [
            IndicatorId::BossRushScore,
            IndicatorId::ZongMaoTechniqueScore,
            IndicatorId::Rouge2Boss,
        ] {
            let mut manager = IndicatorManager::default();
            for _ in 0..2 {
                let marker = manager
                    .add(IndicatorCommand {
                        indicator_id: indicator_id as i32,
                        amount: 500_000,
                        config_effect: 60016,
                    })
                    .unwrap();
                assert_eq!(marker.target_uid, indicator_id as i64);
                assert_eq!(marker.effect_num, 500_000);
                assert_eq!(marker.effect_num1, Some(1));
                assert_eq!(marker.config_effect, 60016);
            }
            assert_eq!(manager.totals.get(&(indicator_id as i32)), Some(&1_000_000));
            assert_eq!(manager.activity_score(), 1_000_000);
        }
    }

    #[test]
    fn configured_ordinary_indicator_emits_the_committed_total() {
        let mut manager = IndicatorManager::default();
        for amount in [3_000, 2_000] {
            let marker = manager
                .add(IndicatorCommand {
                    indicator_id: 3203,
                    amount,
                    config_effect: 60016,
                })
                .unwrap();
            assert_eq!(marker.effect_num, manager.totals[&3203] as i32);
        }

        assert_eq!(manager.totals.get(&3203), Some(&5_000));
    }
}
