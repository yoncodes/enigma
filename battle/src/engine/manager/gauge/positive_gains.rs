use super::{GaugeKey, GaugeManager};

impl GaugeManager {
    pub(crate) fn unclaimed_positive_gains(&self, key: GaugeKey, listener_id: i32) -> i32 {
        let total = self.positive_gains.get(&key).copied().unwrap_or_default();
        let claimed = self
            .positive_gains_claimed
            .get(&(key, listener_id))
            .copied()
            .unwrap_or_default();
        total.saturating_sub(claimed)
    }

    pub(crate) fn claim_positive_gains(&mut self, key: GaugeKey, listener_id: i32) {
        if let Some(&total) = self.positive_gains.get(&key) {
            self.positive_gains_claimed
                .insert((key, listener_id), total);
        }
    }
}
