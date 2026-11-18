use crate::engine::{
    entity::attr::AttrId,
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

pub fn max_hp_rule_op(
    managers: &BattleManagers,
    feature: &ActiveBuffFeature,
    amount_delta: i32,
) -> Option<RuleOp> {
    hp_permille(feature)?;
    if amount_delta == 0 {
        return None;
    }
    let after_permille = managers
        .buff
        .active_features(&managers.hp)
        .iter()
        .filter(|active| active.owner_uid == feature.owner_uid)
        .filter_map(hp_permille)
        .sum::<i32>();
    max_hp_adjustment(managers, feature, amount_delta, after_permille)
}

fn removed_max_hp_rule_op(
    managers: &BattleManagers,
    feature: &ActiveBuffFeature,
    amount_delta: i32,
) -> Option<RuleOp> {
    hp_permille(feature)?;
    if amount_delta == 0 {
        return None;
    }
    let replacement_uid = managers.buff.replacement_uid(
        feature.source_uid,
        feature.owner_uid,
        feature.buff_id,
        feature.buff_uid,
    );
    let after_permille = managers
        .buff
        .active_features(&managers.hp)
        .iter()
        .filter(|active| {
            active.owner_uid == feature.owner_uid && Some(active.buff_uid) != replacement_uid
        })
        .filter_map(hp_permille)
        .sum::<i32>();
    max_hp_adjustment(managers, feature, amount_delta, after_permille)
}

fn max_hp_adjustment(
    managers: &BattleManagers,
    feature: &ActiveBuffFeature,
    amount_delta: i32,
    after_permille: i32,
) -> Option<RuleOp> {
    if !buff_act::is_kind(feature, BuffActKind::Attr) {
        return None;
    }
    let [_, raw_attr_id, permille] = feature.values.as_slice() else {
        return None;
    };
    if AttrId::from_raw(*raw_attr_id) != Some(AttrId::Hp) || amount_delta == 0 {
        return None;
    }
    let origin = buff_act::feature_command_origin(feature)?;
    let before_permille = after_permille.saturating_sub(permille.saturating_mul(amount_delta));
    let base_max = managers.hp.base_max(feature.owner_uid);
    let delta = scaled_hp(base_max, after_permille) - scaled_hp(base_max, before_permille);
    (delta != 0).then_some(RuleOp::Command(BattleCommand::Hp(HpCommand::AdjustMax(
        MaxHpAdjust {
            origin,
            source_uid: feature.source_uid,
            target_uid: feature.owner_uid,
            delta,
        },
    ))))
}

pub fn transaction_rule_ops(
    managers: &BattleManagers,
    event: &crate::engine::event::payload::BattleEvent,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    let owns_refresh_markers = match event {
        crate::engine::event::payload::BattleEvent::BuffChanged(change) => {
            change.after_amount > change.before_amount
                && !managers
                    .buff
                    .command_owns_attribute_refresh_markers(change.buff_id)
        }
        _ => true,
    };
    super::attribute_transaction_rule_ops_with_refresh_marker_ownership(
        managers,
        event,
        BuffActKind::Attr,
        if matches!(
            event,
            crate::engine::event::payload::BattleEvent::BuffRemoved(_)
        ) {
            removed_max_hp_rule_op
        } else {
            max_hp_rule_op
        },
        owns_refresh_markers,
    )
}

fn hp_permille(feature: &ActiveBuffFeature) -> Option<i32> {
    if !buff_act::is_kind(feature, BuffActKind::Attr) {
        return None;
    }
    let [_, raw_attr_id, permille] = feature.values.as_slice() else {
        return None;
    };
    (AttrId::from_raw(*raw_attr_id) == Some(AttrId::Hp))
        .then_some(permille.saturating_mul(feature.amount))
}

fn scaled_hp(base_max: i32, permille: i32) -> i32 {
    ((i64::from(base_max) * i64::from(permille)) / 1000) as i32
}

#[cfg(test)]
mod tests;
