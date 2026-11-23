use crate::engine::{
    manager::{
        BattleManagers,
        buff::{BuffActInfoMarkerResult, BuffCommand, BuffSetState},
    },
    skill::buff_act::{feature_command_origin, is_kind, registry::BuffActKind},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrystalChoice {
    pub owner_uid: i64,
    pub packed: i32,
    pub team_type: i32,
}

pub fn select(
    managers: &mut BattleManagers,
    owner_uid: i64,
    packed: i32,
) -> Option<(CrystalChoice, BuffActInfoMarkerResult)> {
    let feature = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find(|feature| {
            feature.owner_uid == owner_uid && is_kind(feature, BuffActKind::CrystalNotifySelect)
        })?;
    let total = feature.values.get(1).copied()?;
    let per_crystal = feature.values.get(2).copied()?;
    let counts = [packed / 100, packed / 10 % 10, packed % 10];
    if owner_uid == 0
        || counts.iter().sum::<i32>() != total
        || counts
            .iter()
            .any(|count| *count < 0 || *count > per_crystal)
    {
        return None;
    }
    let act_id = feature.act_id()?;
    let params = vec![total, per_crystal, 1];
    let state = managers
        .execute_buff(BuffCommand::SetInternalState(BuffSetState {
            ex_info: None,
            origin: feature_command_origin(&feature)?,
            target_uid: owner_uid,
            buff_uid: feature.buff_uid,
            params: None,
            act_info: Some(vec![sonettobuf::BuffActInfo {
                act_id: Some(act_id),
                param: params.clone(),
                str_param: Some(String::new()),
            }]),
        }))
        .ok()?;
    if state.change.refreshed.is_empty() || !managers.emanation.select(owner_uid, packed) {
        return None;
    }
    Some((
        CrystalChoice {
            owner_uid,
            packed,
            team_type: feature.team_type,
        },
        BuffActInfoMarkerResult {
            target_uid: owner_uid,
            buff_uid: feature.buff_uid,
            act_id,
            params,
            str_param: Some(String::new()),
            team_type: feature.team_type,
        },
    ))
}
