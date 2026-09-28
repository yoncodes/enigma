use crate::engine::{
    manager::{
        BattleManagers,
        buff::{BuffActInfoMarkerResult, BuffChanges, BuffCommand, BuffSetState},
    },
    skill::buff_act::{self, registry::BuffActKind},
};

#[derive(Debug, Clone, PartialEq)]
pub struct ExtraRoundGranted {
    pub buff: BuffChanges,
    pub marker: BuffActInfoMarkerResult,
    pub cards: Vec<sonettobuf::CardInfo>,
    pub deck_count: i32,
}

pub fn grant(managers: &mut BattleManagers, owner_uid: i64) -> Option<ExtraRoundGranted> {
    let feature = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find(|feature| {
            feature.owner_uid == owner_uid
                && buff_act::is_kind(feature, BuffActKind::BuffOwnedCharge)
        })?;
    let [trigger, limit, _linked_skill] = feature.values.get(1..)? else {
        return None;
    };
    let act_id = feature.act_id()?;
    let mut snapshot = managers.buff.snapshot(owner_uid, feature.buff_uid)?;
    let info = snapshot
        .act_info
        .iter_mut()
        .find(|info| info.act_id == Some(act_id))?;
    let [current] = info.param.as_slice() else {
        return None;
    };
    if info.str_param.as_deref() != Some("") || *current < *trigger || *current > *limit {
        return None;
    }
    let next = current - trigger;
    info.param = vec![next];
    let buff = managers
        .execute_buff(BuffCommand::SetInternalState(BuffSetState {
            origin: buff_act::feature_command_origin(&feature)?,
            target_uid: owner_uid,
            buff_uid: feature.buff_uid,
            ex_info: None,
            params: None,
            act_info: Some(snapshot.act_info),
        }))
        .ok()?;
    Some(ExtraRoundGranted {
        buff,
        marker: BuffActInfoMarkerResult {
            target_uid: owner_uid,
            buff_uid: feature.buff_uid,
            act_id,
            params: vec![next],
            str_param: Some(String::new()),
            team_type: feature.team_type,
        },
        cards: managers.card.hand().to_vec(),
        deck_count: managers.card.deck_num(),
    })
}
