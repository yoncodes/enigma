use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::ActiveBuffFeature,
        ex_point::{ExPointChange, ExPointChanges, ExPointCommand, ExPointCommandError},
        gauge::GaugeKey,
    },
    skill::{
        action::SkillPhase,
        buff_act::{self, is_kind, registry::BuffActKind},
        rule::{
            CommandOrigin,
            output::{BattleCommand, RuleOp},
        },
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BloodPoolCountAddExPointFeature {
    pub owner_uid: i64,
    pub buff_uid: i64,
    pub buff_id: i32,
    pub team_type: i32,
    pub act_id: i32,
    pub threshold: i32,
    pub amount: i32,
}

impl BloodPoolCountAddExPointFeature {
    pub fn from_feature(feature: &ActiveBuffFeature) -> Option<Self> {
        if !is_kind(feature, BuffActKind::BloodPoolCountAddExPoint) {
            return None;
        }
        let [act_id, threshold, amount] = feature.values.as_slice() else {
            return None;
        };
        (*threshold > 0 && *amount > 0).then_some(Self {
            owner_uid: feature.owner_uid,
            buff_uid: feature.buff_uid,
            buff_id: feature.buff_id,
            team_type: feature.team_type,
            act_id: *act_id,
            threshold: *threshold,
            amount: *amount,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BloodPoolCountAddExPointCommand {
    pub origin: CommandOrigin,
    pub key: GaugeKey,
    pub listener_uid: i64,
    pub listener_opcode: i32,
    pub source_uid: i64,
    pub target_uid: i64,
    pub threshold: i32,
    pub amount: i32,
}

fn command(
    active: &ActiveBuffFeature,
    feature: BloodPoolCountAddExPointFeature,
    key: GaugeKey,
) -> Option<RuleOp> {
    Some(RuleOp::Command(BattleCommand::BloodPoolCountAddExPoint(
        BloodPoolCountAddExPointCommand {
            origin: buff_act::feature_command_origin(active)?,
            key,
            listener_uid: feature.buff_uid,
            listener_opcode: feature.act_id,
            source_uid: feature.owner_uid,
            target_uid: feature.owner_uid,
            threshold: feature.threshold,
            amount: feature.amount,
        },
    )))
}

pub fn event_rule_ops(
    managers: &BattleManagers,
    event: &BattleEvent,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    let BattleEvent::SkillAction(action) = event else {
        return Vec::new();
    };
    if action.phase != SkillPhase::AfterHit {
        return Vec::new();
    }

    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter_map(|active| {
            let feature = BloodPoolCountAddExPointFeature::from_feature(&active)?;
            let key = crate::engine::mechanic::bloodtithe::rule::key(feature.team_type);
            (managers.gauge.preview_positive_threshold(
                key,
                feature.buff_uid,
                feature.act_id,
                feature.threshold,
                feature.amount,
            ) > 0)
                .then(|| command(&active, feature, key).map(|op| (active, op)))?
        })
        .collect()
}

pub fn setup_rule_ops(
    managers: &BattleManagers,
    active: &ActiveBuffFeature,
) -> Option<Vec<RuleOp>> {
    let feature = BloodPoolCountAddExPointFeature::from_feature(active)?;
    let key = crate::engine::mechanic::bloodtithe::rule::key(feature.team_type);
    if managers.gauge.preview_positive_threshold(
        key,
        feature.buff_uid,
        feature.act_id,
        feature.threshold,
        feature.amount,
    ) <= 0
    {
        return Some(Vec::new());
    }
    Some(
        super::super::with_feature_runtime_markers(vec![(
            active.clone(),
            command(active, feature, key)?,
        )])
        .into_iter()
        .map(|(_, op)| op)
        .collect(),
    )
}

pub fn execute(
    managers: &mut BattleManagers,
    command: BloodPoolCountAddExPointCommand,
) -> Result<Option<ExPointChanges>, ExPointCommandError> {
    let delta = managers.gauge.settle_positive_threshold(
        command.key,
        command.listener_uid,
        command.listener_opcode,
        command.threshold,
        command.amount,
    );
    if delta <= 0 {
        return Ok(None);
    }
    managers
        .execute_ex_point(ExPointCommand::Change(ExPointChange {
            origin: command.origin,
            source_uid: command.source_uid,
            target_uid: command.target_uid,
            delta,
            config_effect: 0,
            effect_type: 0,
        }))
        .map(Some)
}

#[cfg(test)]
mod tests;
