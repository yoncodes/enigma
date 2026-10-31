use crate::engine::{damage::scale_permille, entity::attr::AttrId, manager::BattleManagers};

use super::{is_kind, registry::BuffActKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoundationError {
    InvalidState,
    MultipleCarriers,
}

pub fn supports_shield(args: &[i32]) -> bool {
    matches!(args, [attr, rate, enhanced_buff, enhanced_rate]
        if AttrId::from_raw(*attr) == Some(AttrId::Attack)
            && *rate > 0
            && *enhanced_buff > 0
            && *enhanced_rate > 0)
}

pub fn mitigate(
    managers: &BattleManagers,
    target_uid: i64,
    damage: i32,
) -> Result<Option<i32>, FoundationError> {
    let mut features = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| {
            feature.owner_uid == target_uid && is_kind(feature, BuffActKind::FoundationShield)
        });
    let Some(feature) = features.next() else {
        return Ok(None);
    };
    if features.next().is_some() {
        return Err(FoundationError::MultipleCarriers);
    }
    let [act_id, _, rate, enhanced_buff, enhanced_rate] = feature.values.as_slice() else {
        return Err(FoundationError::InvalidState);
    };
    let rate = if managers
        .buff
        .has_active_buff_id_or_type(feature.source_uid, *enhanced_buff)
    {
        *enhanced_rate
    } else {
        *rate
    };
    let attack = managers
        .buff
        .snapshot(feature.owner_uid, feature.buff_uid)
        .and_then(|buff| {
            buff.act_info
                .iter()
                .find(|info| info.act_id == Some(*act_id))
                .and_then(|info| info.param.first())
                .copied()
        })
        .ok_or(FoundationError::InvalidState)?;
    let reduction = scale_permille(attack, rate);
    Ok(Some(damage.saturating_sub(reduction).max(0)))
}

#[cfg(test)]
mod tests;
