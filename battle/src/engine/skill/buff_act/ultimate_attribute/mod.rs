use crate::engine::{entity::attr::AttrId, manager::BattleManagers};

use super::{attr_only_cal_damage_attack, is_kind, registry::BuffActKind};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [raw_attr, value]
        if matches!(AttrId::from_raw(*raw_attr), Some(AttrId::CriticalRate | AttrId::CriticalDmg))
            && *value != 0)
}

pub fn attributes(
    managers: &BattleManagers,
    source_uid: i64,
    ultimate: bool,
) -> Vec<(AttrId, i32)> {
    if !ultimate {
        return Vec::new();
    }
    managers
        .buff
        .active_features(&managers.hp)
        .iter()
        .filter(|feature| {
            feature.owner_uid == source_uid && is_kind(feature, BuffActKind::UltimateAttribute)
        })
        .filter_map(|feature| {
            let attr = AttrId::from_raw(*feature.values.get(1)?)?;
            Some((
                attr,
                attr_only_cal_damage_attack::attribute_delta(feature, attr),
            ))
        })
        .collect()
}
