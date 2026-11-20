use sonettobuf::BuffActInfo;

use crate::engine::{
    damage::scale_permille,
    entity::attr::AttrId,
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffActInfoMarkerResult, BuffCommand, BuffSetState},
        hp::FakeHpConsumption,
    },
    skill::buff_act::registry::BuffActKind,
};

#[derive(Clone, Copy)]
pub(crate) struct FakeHpState {
    pub buff_uid: i64,
    pub act_id: i32,
    pub current: i32,
    pub maximum: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FakeHpStateError {
    Multiple,
    Invalid,
}

pub(crate) struct DamagePlan {
    pub consumption: FakeHpConsumption,
    pub loss_limit: Option<i32>,
    pub update: Option<super::HpLossStateUpdate>,
}

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [raw_attr, bonus] if AttrId::from_raw(*raw_attr) == Some(AttrId::Attack) && *bonus > 0)
}

pub fn initial_act_info(source_attack: i32, values: &[i32]) -> Option<BuffActInfo> {
    let [act_id, raw_attr, bonus] = values else {
        return None;
    };
    if !supports(&[*raw_attr, *bonus]) {
        return None;
    }
    Some(BuffActInfo {
        act_id: Some(*act_id),
        param: vec![0, scale_permille(source_attack.max(0), *bonus)],
        str_param: Some(String::new()),
    })
}

pub(crate) fn state(
    managers: &BattleManagers,
    owner_uid: i64,
) -> Result<Option<FakeHpState>, FakeHpStateError> {
    feature(managers, owner_uid)?
        .map(|feature| state_for_feature(managers, &feature).ok_or(FakeHpStateError::Invalid))
        .transpose()
}

fn feature(
    managers: &BattleManagers,
    owner_uid: i64,
) -> Result<Option<ActiveBuffFeature>, FakeHpStateError> {
    let mut features = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| {
            feature.owner_uid == owner_uid && super::is_kind(feature, BuffActKind::LostHpToFakeHp)
        });
    let Some(feature) = features.next() else {
        return Ok(None);
    };
    if features.next().is_some() {
        return Err(FakeHpStateError::Multiple);
    }
    Ok(Some(feature))
}

pub(crate) fn plan_damage(
    managers: &BattleManagers,
    owner_uid: i64,
    remaining: i32,
) -> Result<Option<DamagePlan>, FakeHpStateError> {
    let Some(feature) = feature(managers, owner_uid)? else {
        return Ok(None);
    };
    let state = state_for_feature(managers, &feature).ok_or(FakeHpStateError::Invalid)?;
    let mut plan = DamagePlan {
        consumption: FakeHpConsumption {
            buff_uid: state.buff_uid,
            consumed: 0,
        },
        loss_limit: None,
        update: None,
    };
    let hp = managers.hp.get(owner_uid);
    if hp.current > 0 && remaining >= hp.current {
        if hp.max <= 0 {
            return Err(FakeHpStateError::Invalid);
        }
        let loss = hp.current - 1;
        let overflow = remaining - loss;
        let next = if overflow <= state.current {
            plan.loss_limit = Some(loss);
            (state.current - overflow)
                .saturating_add(credit(state, loss, hp.max))
                .min(state.maximum)
        } else {
            0
        };
        plan.consumption.consumed = state.current - next;
        plan.update =
            Some(state_update(managers, &feature, state, next).ok_or(FakeHpStateError::Invalid)?);
    }
    Ok(Some(plan))
}

fn state_for_feature(
    managers: &BattleManagers,
    feature: &ActiveBuffFeature,
) -> Option<FakeHpState> {
    if !super::is_kind(feature, BuffActKind::LostHpToFakeHp) {
        return None;
    }
    let act_id = feature.act_id()?;
    let buff = managers
        .buff
        .snapshot(feature.owner_uid, feature.buff_uid)?;
    let mut matching = buff
        .act_info
        .iter()
        .filter(|info| info.act_id == Some(act_id));
    let info = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    let [current, maximum] = info.param.as_slice() else {
        return None;
    };
    (info.str_param.as_deref() == Some("") && (0..=*maximum).contains(current) && *maximum > 0)
        .then_some(FakeHpState {
            buff_uid: feature.buff_uid,
            act_id,
            current: *current,
            maximum: *maximum,
        })
}

pub fn hp_loss_state_update(
    managers: &BattleManagers,
    feature: &ActiveBuffFeature,
    amount: i32,
) -> Option<Vec<super::HpLossStateUpdate>> {
    if amount <= 0 {
        return Some(Vec::new());
    }
    let state = state(managers, feature.owner_uid).ok()??;
    if state.buff_uid != feature.buff_uid {
        return None;
    }
    let maximum_hp = managers.hp.max(feature.owner_uid);
    if maximum_hp <= 0 {
        return Some(Vec::new());
    }
    let added = credit(state, amount, maximum_hp);
    let next = state.current.saturating_add(added).min(state.maximum);
    if next == state.current {
        return Some(Vec::new());
    }
    Some(vec![state_update(managers, feature, state, next)?])
}

fn credit(state: FakeHpState, amount: i32, maximum_hp: i32) -> i32 {
    (i64::from(amount) * i64::from(state.maximum) / i64::from(maximum_hp))
        .clamp(0, i64::from(i32::MAX)) as i32
}

fn state_update(
    managers: &BattleManagers,
    feature: &ActiveBuffFeature,
    state: FakeHpState,
    next: i32,
) -> Option<super::HpLossStateUpdate> {
    let mut buff = managers
        .buff
        .snapshot(feature.owner_uid, feature.buff_uid)?;
    let info = buff
        .act_info
        .iter_mut()
        .find(|info| info.act_id == Some(state.act_id))?;
    info.param = vec![next, state.maximum];
    info.str_param = Some(String::new());
    let marker = BuffActInfoMarkerResult {
        target_uid: feature.owner_uid,
        buff_uid: feature.buff_uid,
        act_id: state.act_id,
        params: info.param.clone(),
        str_param: info.str_param.clone(),
        team_type: 0,
    };
    Some(super::HpLossStateUpdate {
        command: BuffCommand::SetInternalState(BuffSetState {
            origin: super::feature_command_origin(feature)?,
            target_uid: feature.owner_uid,
            buff_uid: feature.buff_uid,
            ex_info: None,
            params: None,
            act_info: Some(buff.act_info),
        }),
        marker,
        after_hp: false,
    })
}

#[cfg(test)]
mod tests;
