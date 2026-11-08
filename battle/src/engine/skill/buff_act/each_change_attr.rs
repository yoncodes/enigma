use crate::engine::{
    entity::attr::AttrId,
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffManager},
        hp::{HpCommand, MaxHpAdjust},
    },
    skill::{
        buff_act::{self, registry::BuffActKind},
        rule::output::{BattleCommand, RuleOp},
    },
};

pub(crate) fn definition_snapshot(
    managers: &BattleManagers,
    buff_id: i32,
    source_uid: i64,
    target_uid: i64,
) -> Vec<(i64, AttrId, i32)> {
    let mut snapshots = Vec::<(i64, AttrId, i32)>::new();
    for feature in managers
        .buff
        .definition_features(buff_id)
        .into_iter()
        .filter(|feature| buff_act::is_kind(feature, BuffActKind::EachChangeAttr))
    {
        let [_, source_attr, source_rate, target_attr, target_rate] = feature.values.as_slice()
        else {
            continue;
        };
        for (owner_uid, basis_uid, raw_attr, rate) in [
            (source_uid, target_uid, *source_attr, *source_rate),
            (target_uid, source_uid, *target_attr, *target_rate),
        ] {
            let Some(attr) = AttrId::from_raw(raw_attr) else {
                continue;
            };
            if rate == 0 || attr == AttrId::Hp {
                continue;
            }
            let value = (i64::from(managers.origin_attribute(basis_uid, attr)) * i64::from(rate)
                / 1000)
                .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
            match snapshots.iter_mut().find(|(known_owner, known_attr, _)| {
                *known_owner == owner_uid && *known_attr == attr
            }) {
                Some((_, _, known)) => *known = known.saturating_add(value),
                None => snapshots.push((owner_uid, attr, value)),
            }
        }
    }
    snapshots
}

pub(crate) fn seed(managers: &mut BattleManagers) {
    for (buff_uid, buff_id, source_uid, target_uid) in managers.buff.each_change_attr_sources() {
        let snapshots = definition_snapshot(managers, buff_id, source_uid, target_uid);
        managers.buff.commit_attr_snapshots(buff_uid, &snapshots);
    }
}

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [source_attr, source_rate, target_attr, target_rate]
        if AttrId::from_raw(*source_attr).is_some_and(|attr| *source_rate == 0 || (supported_attr(attr) && attr != AttrId::Hp))
            && *source_rate >= 0
            && AttrId::from_raw(*target_attr).is_some_and(supported_attr)
            && *target_rate >= 0)
}

fn supported_attr(attr: AttrId) -> bool {
    matches!(
        attr,
        AttrId::Hp
            | AttrId::Attack
            | AttrId::RealityDef
            | AttrId::MentalDef
            | AttrId::CriticalRate
            | AttrId::CriticalDmg
    )
}

pub(crate) fn attribute_delta(
    buff_uid: i64,
    owner_uid: i64,
    amount: i32,
    attr_id: AttrId,
    buffs: &BuffManager,
) -> i32 {
    buffs
        .attr_snapshot(buff_uid, owner_uid, attr_id)
        .unwrap_or_default()
        .saturating_mul(amount)
}

pub fn max_hp_rule_op(
    managers: &BattleManagers,
    feature: &ActiveBuffFeature,
    amount_delta: i32,
) -> Option<RuleOp> {
    if !buff_act::is_kind(feature, BuffActKind::EachChangeAttr) || amount_delta == 0 {
        return None;
    }
    let [_, _, _, raw_attr, permille] = feature.values.as_slice() else {
        return None;
    };
    if AttrId::from_raw(*raw_attr) != Some(AttrId::Hp) {
        return None;
    }
    let source_uid = if feature.source_uid != 0 {
        feature.source_uid
    } else {
        feature.owner_uid
    };
    let delta = (i128::from(managers.hp.max(source_uid)) * i128::from(*permille) / 1000
        * i128::from(amount_delta))
    .clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32;
    (delta != 0).then_some(RuleOp::Command(BattleCommand::Hp(HpCommand::AdjustMax(
        MaxHpAdjust {
            origin: buff_act::feature_command_origin(feature)?,
            source_uid,
            target_uid: feature.owner_uid,
            delta,
        },
    ))))
}

pub fn transaction_rule_ops(
    managers: &BattleManagers,
    event: &crate::engine::event::payload::BattleEvent,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    super::attribute_transaction_rule_ops(
        managers,
        event,
        BuffActKind::EachChangeAttr,
        max_hp_rule_op,
    )
}
