use std::collections::BTreeMap;

use crate::engine::{
    entity::attr::AttrId,
    manager::{
        buff::{ActiveBuffFeature, BuffManager, CommandOrigin},
        hp::HpManager,
    },
    skill::buff_act::{feature_command_origin, is_kind, registry::BuffActKind},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtraTransfer {
    pub buff_uid: i64,
    pub consumed_layers: i32,
    pub progress: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TeamAttributeRule {
    pub dynamic_attr: AttrId,
    pub per_recorded_stack: i32,
    pub fixed_attr: AttrId,
    pub fixed_value: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransferRecord {
    pub origin: CommandOrigin,
    pub owner_uid: i64,
    pub buff_uid: i64,
    pub act_id: i32,
    pub delta: i32,
}

pub fn supports_extra_value(args: &[i32]) -> bool {
    matches!(args, [multiplier] if *multiplier > 0)
}

pub fn supports_team_attribute(args: &[i32]) -> bool {
    team_attribute_rule(args).is_some()
}

pub fn supports_record(args: &[i32]) -> bool {
    matches!(args, [cap] if *cap > 0)
}

pub fn team_attribute_rule(args: &[i32]) -> Option<TeamAttributeRule> {
    let [dynamic_attr, per_recorded_stack, fixed_attr, fixed_value] = args else {
        return None;
    };
    (*per_recorded_stack > 0 && *fixed_value >= 0).then_some(TeamAttributeRule {
        dynamic_attr: AttrId::from_raw(*dynamic_attr)?,
        per_recorded_stack: *per_recorded_stack,
        fixed_attr: AttrId::from_raw(*fixed_attr)?,
        fixed_value: *fixed_value,
    })
}

pub fn transfer_records(
    buffs: &BuffManager,
    hp: &HpManager,
    team_type: i32,
    contributor_uid: i64,
    amount: i32,
) -> Vec<TransferRecord> {
    buffs
        .active_features(hp)
        .into_iter()
        .filter(|feature| {
            feature.team_type == team_type
                && feature.owner_uid != contributor_uid
                && feature.owner_alive
                && is_kind(feature, BuffActKind::RecordTeamExElectricTransConsumeValue)
        })
        .filter_map(|feature| {
            let [act_id, cap] = feature.values.as_slice() else {
                return None;
            };
            let delta = amount
                .max(0)
                .min(cap.saturating_sub(buffs.act_value(feature.buff_uid, *act_id)));
            (delta > 0).then_some(TransferRecord {
                origin: feature_command_origin(&feature)?,
                owner_uid: feature.owner_uid,
                buff_uid: feature.buff_uid,
                act_id: *act_id,
                delta,
            })
        })
        .collect()
}

pub fn team_damage_multiplier_delta(buffs: &BuffManager, hp: &HpManager, owner_uid: i64) -> i32 {
    let team_type = buffs.team_type(owner_uid);
    buffs
        .active_features(hp)
        .into_iter()
        .filter(|feature| {
            Some(feature.team_type) == team_type
                && feature.owner_alive
                && is_kind(feature, BuffActKind::TeamExElectricTransConsumeValueAttr)
        })
        .filter_map(|feature| {
            let [act_id, args @ ..] = feature.values.as_slice() else {
                return None;
            };
            let rule = team_attribute_rule(args)?;
            (rule.dynamic_attr == AttrId::DmgBonus)
                .then(|| buffs.grant_value(feature.buff_uid, *act_id))
                .flatten()
        })
        .sum()
}

pub fn extra_transfers(buffs: &BuffManager, hp: &HpManager, owner_uid: i64) -> Vec<ExtraTransfer> {
    let mut by_buff = BTreeMap::<i64, ExtraTransfer>::new();
    for transfer in buffs
        .active_features(hp)
        .into_iter()
        .filter(|feature| feature.owner_uid == owner_uid && feature.owner_alive)
        .filter_map(extra_transfer)
    {
        by_buff
            .entry(transfer.buff_uid)
            .and_modify(|existing| {
                existing.progress = existing.progress.saturating_add(transfer.progress);
            })
            .or_insert(transfer);
    }
    by_buff.into_values().collect()
}

fn extra_transfer(feature: ActiveBuffFeature) -> Option<ExtraTransfer> {
    if !is_kind(&feature, BuffActKind::ExtraValueElectricTransform) {
        return None;
    }
    let multiplier = *feature.values.get(1)?;
    let consumed_layers = feature.amount.max(0);
    let progress = consumed_layers.saturating_mul(multiplier);
    (progress > 0).then_some(ExtraTransfer {
        buff_uid: feature.buff_uid,
        consumed_layers,
        progress,
    })
}

#[cfg(test)]
mod tests;
