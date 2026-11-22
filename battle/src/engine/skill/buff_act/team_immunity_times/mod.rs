use sonettobuf::BuffActInfo;

use crate::engine::{
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffActInfoMarkerResult, BuffCommand, BuffSetState, BuffStatus},
    },
    skill::{
        buff_act::registry::BuffActKind,
        rule::{
            SetupStage,
            output::{BattleCommand, RuleOp},
        },
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(
        args,
        [status, count]
            if BuffStatus::from_id(*status) == BuffStatus::Control && *count > 0
    )
}

pub fn setup_rule_ops(
    managers: &BattleManagers,
    feature: &ActiveBuffFeature,
    stage: SetupStage,
) -> Option<Vec<RuleOp>> {
    if stage != SetupStage::RoundStart || !super::is_kind(feature, BuffActKind::TeamImmunityTimes) {
        return Some(Vec::new());
    }
    let [act_id, status, count] = feature.values.as_slice() else {
        return None;
    };
    if !supports(&[*status, *count]) {
        return None;
    }

    let mut buff = managers
        .buff
        .snapshot(feature.owner_uid, feature.buff_uid)?;
    if let Some(info) = buff
        .act_info
        .iter_mut()
        .find(|info| info.act_id == Some(*act_id))
    {
        info.param = vec![*count];
        info.str_param = Some(String::new());
    } else {
        buff.act_info.push(BuffActInfo {
            act_id: Some(*act_id),
            param: vec![*count],
            str_param: Some(String::new()),
        });
    }
    let origin = super::feature_command_origin(feature)?;
    Some(vec![
        RuleOp::Command(BattleCommand::Buff(BuffCommand::SetInternalState(
            BuffSetState {
                origin,
                target_uid: feature.owner_uid,
                buff_uid: feature.buff_uid,
                ex_info: None,
                params: None,
                act_info: Some(buff.act_info),
            },
        ))),
        RuleOp::BuffActInfoMarker(BuffActInfoMarkerResult {
            target_uid: feature.owner_uid,
            buff_uid: feature.buff_uid,
            act_id: *act_id,
            params: vec![*count],
            str_param: Some(String::new()),
            team_type: 0,
        }),
    ])
}

#[cfg(test)]
mod tests;
