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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndicatorCommand {
    pub indicator_id: i32,
    pub amount: i32,
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
        self.activity_score = self
            .activity_score
            .saturating_add(i64::from(command.amount));
        Some(EffectMarker {
            target_uid: i64::from(command.indicator_id),
            effect_type: sonettobuf::effect_type_enum::EffectType::Indicatorchange as i32,
            effect_num: command.amount,
            effect_num1: Some(1),
            config_effect: command.config_effect,
            reserve_id: None,
            reserve_str: None,
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
        self.totals
            .get(&(indicator_id as i32))
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
    fn configured_indicator_change_updates_and_emits_the_named_indicator() {
        let mut manager = IndicatorManager::default();
        let marker = manager
            .add(IndicatorCommand {
                indicator_id: 6,
                amount: 500_000,
                config_effect: 60016,
            })
            .unwrap();

        assert_eq!(marker.target_uid, 6);
        assert_eq!(marker.effect_num, 500_000);
        assert_eq!(marker.effect_num1, Some(1));
        assert_eq!(marker.config_effect, 60016);
        assert_eq!(manager.totals.get(&6), Some(&500_000));
        assert_eq!(manager.activity_score(), 500_000);
    }
}
