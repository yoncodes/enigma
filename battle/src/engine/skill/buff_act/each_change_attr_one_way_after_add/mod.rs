use sonettobuf::BuffActInfo;

use crate::engine::{
    entity::attr::AttrId,
    manager::BattleManagers,
    skill::buff_act::{self, registry::BuffActKind},
};

pub fn supports(args: &[i32]) -> bool {
    matches!(
        args,
        [source_attr, target_attr, rate, cap]
            if AttrId::from_raw(*source_attr) == Some(AttrId::ConduitMight)
                && AttrId::from_raw(*target_attr) == Some(AttrId::DmgBonus)
                && *rate >= 0
                && *cap >= 0
    )
}

pub(crate) fn definition_snapshot(
    managers: &BattleManagers,
    buff_id: i32,
    source_uid: i64,
    target_uid: i64,
    removed_buff_uids: &[i64],
) -> Option<Vec<BuffActInfo>> {
    let act_info = managers
        .buff
        .definition_features(buff_id)
        .into_iter()
        .filter(|feature| buff_act::is_kind(feature, BuffActKind::EachChangeAttrOneWayAfterAdd))
        .filter_map(|feature| {
            let [act_id, raw_source_attr, raw_target_attr, rate, cap] = feature.values.as_slice()
            else {
                return None;
            };
            let source_attr = AttrId::from_raw(*raw_source_attr)?;
            let target_attr = AttrId::from_raw(*raw_target_attr)?;
            let mut source_value = managers.origin_attribute(source_uid, source_attr);
            if source_uid == target_uid {
                source_value = source_value
                    .saturating_sub(managers.buff.configured_attribute_delta_for_uids(
                        source_uid,
                        source_attr,
                        removed_buff_uids,
                    ))
                    .saturating_add(
                        managers
                            .buff
                            .definition_features(buff_id)
                            .into_iter()
                            .filter(|candidate| buff_act::is_kind(candidate, BuffActKind::Attr))
                            .filter_map(|candidate| match candidate.values.as_slice() {
                                [_, raw_attr, value]
                                    if AttrId::from_raw(*raw_attr) == Some(source_attr) =>
                                {
                                    Some(*value)
                                }
                                _ => None,
                            })
                            .fold(0, i32::saturating_add),
                    );
            }
            let value = (i64::from(source_value) * i64::from(*rate) / 1000)
                .clamp(0, i64::from(*cap)) as i32;
            Some(BuffActInfo {
                act_id: Some(*act_id),
                param: Vec::new(),
                str_param: Some(format!("{}#{value}", target_attr.id())),
            })
        })
        .collect::<Vec<_>>();
    (!act_info.is_empty()).then_some(act_info)
}

#[cfg(test)]
mod tests;
