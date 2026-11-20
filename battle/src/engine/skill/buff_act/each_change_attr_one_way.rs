use crate::engine::{
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffActInfoMarkerResult},
        hp::{HpCommand, MaxHpAdjust},
    },
    skill::{
        buff_act::{self, registry::BuffActKind},
        rule::output::{BattleCommand, RuleOp},
    },
};
use sonettobuf::BuffActInfo;

pub fn supports(args: &[i32]) -> bool {
    matches!(
        args,
        [raw_attr, rate, cap]
            if matches!(
                AttrId::from_raw(*raw_attr),
                Some(
                    AttrId::Hp
                        | AttrId::Attack
                        | AttrId::RealityDef
                        | AttrId::MentalDef
                        | AttrId::CriticalTechnique
                        | AttrId::CriticalDmg
                )
            ) && *rate >= 0 && *cap >= 0
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
        .filter(|feature| buff_act::is_kind(feature, BuffActKind::EachChangeAttrOneWay))
        .filter_map(|feature| {
            let [act_id, raw_attr, rate, cap] = feature.values.as_slice() else {
                return None;
            };
            let attr = AttrId::from_raw(*raw_attr)?;
            let basis = match attr {
                AttrId::Hp => managers.hp.max_without_capacity(source_uid),
                _ => managers.origin_attribute(source_uid, attr),
            };
            let value =
                (i64::from(basis) * i64::from(*rate) / 1000).clamp(0, i64::from(*cap)) as i32;
            Some(BuffActInfo {
                act_id: Some(*act_id),
                param: Vec::new(),
                str_param: Some(format!("{raw_attr}#{value}")),
            })
        })
        .collect::<Vec<_>>();
    (!act_info.is_empty()).then_some(act_info)
}

fn snapshot_value(managers: &BattleManagers, feature: &ActiveBuffFeature) -> Option<(AttrId, i32)> {
    if !buff_act::is_kind(feature, BuffActKind::EachChangeAttrOneWay) {
        return None;
    }
    let [_, raw_attr, _, _] = feature.values.as_slice() else {
        return None;
    };
    let attr = AttrId::from_raw(*raw_attr)?;
    let value = managers
        .buff
        .fixed_attribute_value(feature.buff_uid, attr)?;
    Some((attr, value))
}

pub fn transaction_rule_ops(
    managers: &BattleManagers,
    event: &BattleEvent,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    let change = match event {
        BattleEvent::BuffAdded(change) | BattleEvent::BuffRemoved(change) => change,
        _ => return Vec::new(),
    };
    let features = super::changed_features(managers, event, BuffActKind::EachChangeAttrOneWay);
    if features.is_empty() {
        return Vec::new();
    }

    if matches!(event, BattleEvent::BuffRemoved(_)) {
        let Some((feature, _)) = features
            .into_iter()
            .find(|(feature, _)| feature.values.get(1).copied() == Some(AttrId::Hp.id()))
        else {
            return Vec::new();
        };
        if change.act_value == 0 {
            return Vec::new();
        }
        let Some(origin) = buff_act::feature_command_origin(&feature) else {
            return Vec::new();
        };
        return vec![(
            feature,
            RuleOp::Command(BattleCommand::Hp(HpCommand::AdjustMax(MaxHpAdjust {
                origin,
                source_uid: change.source_uid,
                target_uid: change.target_uid,
                delta: -change.act_value,
            }))),
        )];
    }

    let snapshots = features
        .into_iter()
        .filter_map(|(feature, _)| {
            let (attr, value) = snapshot_value(managers, &feature)?;
            Some((feature, attr, value))
        })
        .collect::<Vec<_>>();
    if snapshots.is_empty() {
        return Vec::new();
    }
    let mut ops = Vec::new();
    for (feature, attr, value) in snapshots {
        if attr == AttrId::Hp && value != 0 {
            let Some(origin) = buff_act::feature_command_origin(&feature) else {
                continue;
            };
            let adjustment = MaxHpAdjust {
                origin,
                source_uid: change.source_uid,
                target_uid: change.target_uid,
                delta: value,
            };
            let command = if change.replaced_fixed_max_hp == Some(value) {
                HpCommand::RestoreMax(adjustment)
            } else {
                HpCommand::AdjustMax(adjustment)
            };
            ops.push((feature, RuleOp::Command(BattleCommand::Hp(command))));
        }
    }
    ops
}

#[cfg(test)]
#[path = "each_change_attr_one_way/tests.rs"]
mod tests;
