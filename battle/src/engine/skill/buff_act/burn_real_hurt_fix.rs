use crate::engine::manager::BattleManagers;

use super::{is_kind, registry::BuffActKind};

pub fn adjusted_amount(managers: &BattleManagers, owner_uid: i64, amount: i32) -> i32 {
    let mut reduction = 0i32;
    let mut floor = 0i32;
    let mut configured = false;
    for feature in managers.buff.active_features(&managers.hp) {
        if feature.owner_uid != owner_uid || !is_kind(&feature, BuffActKind::BurnRealHurtFix) {
            continue;
        }
        let [_, feature_reduction, feature_floor] = feature.values.as_slice() else {
            continue;
        };
        configured = true;
        reduction = reduction.saturating_add(*feature_reduction);
        floor = floor.max(*feature_floor);
    }
    if !configured {
        return amount;
    }

    let scaled = (i64::from(amount) * i64::from(1000i32.saturating_sub(reduction).max(0)) / 1000)
        .clamp(0, i64::from(i32::MAX)) as i32;
    let hp = managers.hp.get(owner_uid);
    let minimum = i64::from(hp.max) * i64::from(floor) / 1000;
    scaled.min((i64::from(hp.current) - minimum).clamp(0, i64::from(i32::MAX)) as i32)
}

#[cfg(test)]
mod tests;
