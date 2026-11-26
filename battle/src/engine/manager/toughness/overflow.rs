use std::collections::HashMap;

use super::ToughnessChange;

#[derive(Debug, Clone, Default)]
pub(super) struct OverflowRecord {
    amounts: HashMap<i64, i64>,
}

impl OverflowRecord {
    pub(super) fn record(&mut self, change: ToughnessChange) {
        if change.point_delta <= 0 {
            return;
        }
        let before = change.before;
        let crossed = if change.broke {
            i64::from(before.point) * i64::from(before.segment_value)
        } else {
            0
        };
        let overflow = i64::from(change.reduction) - i64::from(before.value) - crossed;
        if overflow > 0 {
            let amount = self.amounts.entry(change.target_uid).or_default();
            *amount = amount.saturating_add(overflow);
        }
    }

    pub(super) fn settle(&mut self, uid: i64, segment: i32, loss: Option<i32>) -> i64 {
        let amount = self.amounts.remove(&uid).unwrap_or_default();
        let Some(loss) = loss else { return 0 };
        let restored_segment = segment.saturating_sub(loss).max(0);
        let remaining = amount - i64::from(restored_segment);
        if remaining > 0 {
            self.amounts.insert(uid, remaining);
        }
        amount
    }

    pub(super) fn clear(&mut self, uid: i64) {
        self.amounts.remove(&uid);
    }
}
