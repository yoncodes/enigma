use crate::engine::{
    entity::attr::AttrId, manager::BattleManagers, skill::buff_act::registry::BuffActKind,
};

pub fn supports(args: &[i32]) -> bool {
    matches!(
        args,
        [attr, -300] if AttrId::from_raw(*attr) == Some(AttrId::DmgBonus)
    )
}

pub fn owner_attribute_delta(
    managers: &BattleManagers,
    owner_uid: i64,
    target_count_kind: i32,
    attr_id: AttrId,
) -> i32 {
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| feature.owner_uid == owner_uid)
        .filter(|feature| {
            matches!(
                (super::feature_kind(feature), target_count_kind),
                (Some(BuffActKind::AttrSkillSingle), 1) | (Some(BuffActKind::AttrSkillMultiple), 2)
            )
        })
        .filter_map(|feature| {
            let [_, raw_attr, delta] = feature.values.as_slice() else {
                return None;
            };
            (AttrId::from_raw(*raw_attr) == Some(attr_id))
                .then_some(delta.saturating_mul(feature.amount))
        })
        .sum()
}
