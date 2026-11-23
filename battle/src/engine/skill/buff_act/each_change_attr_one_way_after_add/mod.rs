use sonettobuf::BuffActInfo;

use crate::engine::{
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::ActiveBuffFeature,
        hp::{HpCommand, MaxHpAdjust},
    },
    skill::{
        buff_act::{self, registry::BuffActKind},
        rule::output::{BattleCommand, RuleOp},
    },
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
            let source_value = managers.origin_attribute(source_uid, source_attr);
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

pub fn transaction_rule_ops(
    managers: &BattleManagers,
    event: &BattleEvent,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    let BattleEvent::BuffRemoved(change) = event else {
        return Vec::new();
    };
    super::changed_features(managers, event, BuffActKind::EachChangeAttrOneWayAfterAdd)
        .into_iter()
        .filter(|(feature, _)| {
            managers
                .buff
                .replacement_uid(
                    feature.source_uid,
                    feature.owner_uid,
                    feature.buff_id,
                    feature.buff_uid,
                )
                .and_then(|uid| managers.buff.snapshot(feature.owner_uid, uid))
                .is_some_and(|replacement| replacement.from_uid == Some(feature.source_uid))
        })
        .filter_map(|(feature, _)| {
            let origin = buff_act::feature_command_origin(&feature)?;
            Some((
                feature,
                RuleOp::Command(BattleCommand::Hp(HpCommand::RemoveReplacedMax(
                    MaxHpAdjust {
                        origin,
                        source_uid: change.source_uid,
                        target_uid: change.target_uid,
                        delta: 0,
                    },
                ))),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests;
