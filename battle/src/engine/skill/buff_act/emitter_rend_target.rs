use crate::engine::{
    damage::DamageRateTerm,
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffCommand, BuffRemove, BuffRemoveSelector},
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        target::TargetPool,
    },
};

use super::{feature_command_origin, is_kind, registry::BuffActKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmitterRendPlan {
    feature: ActiveBuffFeature,
    target_buff_id: i32,
    base_rate: i32,
    rate_per_layer: i32,
    before_damage_buff_id: i32,
    targets: Vec<(i64, i32)>,
}

impl EmitterRendPlan {
    pub fn frame_owner(&self) -> Option<crate::engine::runtime::record::FrameOwner> {
        let origin = feature_command_origin(&self.feature)?;
        Some(crate::engine::runtime::record::FrameOwner::BuffAct {
            owner_uid: self.feature.owner_uid,
            source_uid: self.feature.source_uid,
            buff_uid: self.feature.buff_uid,
            buff_id: self.feature.buff_id,
            key: origin.key,
        })
    }

    pub fn targets(&self) -> Vec<i64> {
        self.targets.iter().map(|(uid, _)| *uid).collect()
    }

    pub fn rate_term(&self, target_uid: i64) -> Option<DamageRateTerm> {
        let layers = self
            .targets
            .iter()
            .find_map(|(uid, layers)| (*uid == target_uid).then_some(*layers))?;
        let rate = i64::from(self.base_rate)
            .saturating_add(i64::from(self.rate_per_layer).saturating_mul(i64::from(layers)))
            .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
        (rate != 0).then_some(DamageRateTerm {
            opcode: self.feature.act_id().unwrap_or_default(),
            rate,
            career_scaled: true,
            composition: crate::engine::damage::DamageRateComposition::Additive,
        })
    }

    pub fn before_damage_rule_ops(&self) -> Vec<RuleOp> {
        let Some(origin) = feature_command_origin(&self.feature) else {
            return Vec::new();
        };
        (self.before_damage_buff_id > 0)
            .then_some(RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(
                BuffRemove {
                    origin,
                    target_uid: self.feature.owner_uid,
                    selector: BuffRemoveSelector::ExactId(self.before_damage_buff_id),
                },
            ))))
            .into_iter()
            .collect()
    }

    pub fn after_damage_rule_ops(&self) -> Vec<RuleOp> {
        let Some(origin) = feature_command_origin(&self.feature) else {
            return Vec::new();
        };
        let target_removals = self
            .targets
            .iter()
            .map(|(target_uid, _)| {
                BuffCommand::Remove(BuffRemove {
                    origin,
                    target_uid: *target_uid,
                    selector: BuffRemoveSelector::ExactId(self.target_buff_id),
                })
            })
            .collect();
        vec![
            RuleOp::Command(BattleCommand::BuffBatch(target_removals)),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
                origin,
                target_uid: self.feature.owner_uid,
                selector: BuffRemoveSelector::Uid(self.feature.buff_uid),
            }))),
        ]
    }
}

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [target_buff_id, base_rate, rate_per_layer, before_damage_buff_id]
        if *target_buff_id > 0
            && *base_rate >= 0
            && *rate_per_layer > 0
            && *before_damage_buff_id > 0)
}

pub fn resolve(
    managers: &BattleManagers,
    pool: &TargetPool,
    source_uid: i64,
    attack_index: i32,
    attack_max: i32,
) -> Option<EmitterRendPlan> {
    if attack_index <= 0 || attack_index != attack_max {
        return None;
    }
    let feature = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| feature.owner_uid == source_uid)
        .filter(|feature| is_kind(feature, BuffActKind::EmitterRendTarget))
        .min_by_key(|feature| feature.buff_uid)?;
    let (target_buff_id, base_rate, rate_per_layer, before_damage_buff_id) =
        match feature.values.as_slice() {
            [
                _,
                target_buff_id,
                base_rate,
                rate_per_layer,
                before_damage_buff_id,
            ] => (
                *target_buff_id,
                *base_rate,
                *rate_per_layer,
                *before_damage_buff_id,
            ),
            _ => return None,
        };
    if !supports(&feature.values[1..]) {
        return None;
    }
    let targets = pool
        .enemies(source_uid, false)
        .iter()
        .filter(|target| managers.hp.current(target.uid) > 0)
        .filter_map(|target| {
            let layers = managers.buff.buff_id_amount(target.uid, target_buff_id);
            (layers > 0).then_some((target.uid, layers))
        })
        .collect::<Vec<_>>();
    if targets.is_empty() {
        return None;
    }
    Some(EmitterRendPlan {
        feature,
        target_buff_id,
        base_rate,
        rate_per_layer,
        before_damage_buff_id,
        targets,
    })
}

#[cfg(test)]
mod tests;
