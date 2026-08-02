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

#[derive(Debug, Clone, Default)]
pub struct IndicatorManager {
    damage_targets: HashMap<i64, i32>,
    totals: HashMap<i32, i64>,
}

impl IndicatorManager {
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
        Some(EffectMarker {
            target_uid: i64::from(indicator_id),
            effect_type: sonettobuf::effect_type_enum::EffectType::Indicatorchange as i32,
            effect_num: amount,
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
        assert_eq!(marker.config_effect, 60016);
        assert_eq!(manager.total(IndicatorId::BossRushScore), 75);
    }
}
