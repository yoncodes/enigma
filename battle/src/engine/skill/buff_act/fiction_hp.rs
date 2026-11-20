use sonettobuf::{BuffActInfo, effect_type_enum::EffectType};

use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffActInfoMarkerResult, BuffCommand, BuffSetState},
        hp::{CurrentHpSet, HpCommand},
    },
    skill::{
        buff_act::BuffActRuleOp,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    args.is_empty()
}

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<BuffActRuleOp>> {
    if !super::subscriber_is_kind(subscriber, super::registry::BuffActKind::FictionHp) {
        return Some(Vec::new());
    }
    match event {
        BattleEvent::RoundStart | BattleEvent::Kind(EventKind::RoundStart) => {
            trim_rule_ops(managers, subscriber)
        }
        _ => Some(Vec::new()),
    }
}

pub fn hp_loss_state_update(
    managers: &BattleManagers,
    feature: &ActiveBuffFeature,
    amount: i32,
) -> Option<Vec<super::HpLossStateUpdate>> {
    let act_id = feature.act_id()?;
    let current = managers
        .hp
        .current(feature.owner_uid)
        .saturating_sub(amount)
        .max(0);
    let mut buff = managers
        .buff
        .snapshot(feature.owner_uid, feature.buff_uid)?;
    buff.act_info.retain(|info| info.act_id != Some(act_id));
    buff.act_info.push(BuffActInfo {
        act_id: Some(act_id),
        param: vec![current],
        str_param: Some(String::new()),
    });
    Some(vec![super::HpLossStateUpdate {
        command: BuffCommand::SetInternalState(BuffSetState {
            origin: super::feature_command_origin(feature)?,
            target_uid: feature.owner_uid,
            buff_uid: feature.buff_uid,
            ex_info: None,
            params: None,
            act_info: Some(buff.act_info),
        }),
        marker: BuffActInfoMarkerResult {
            target_uid: feature.owner_uid,
            buff_uid: feature.buff_uid,
            act_id,
            params: vec![current],
            str_param: Some(String::new()),
            team_type: 0,
        },
        after_hp: true,
    }])
}

fn trim_rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
) -> Option<Vec<BuffActRuleOp>> {
    let buff = managers
        .buff
        .snapshot(subscriber.owner_uid, subscriber.buff_uid)?;
    let Some(real) = stored(&buff, subscriber.key.definition.opcode) else {
        return Some(Vec::new());
    };
    // A value stored by a lethal hit must not end a later revival.
    if real <= 0 || !subscriber.owner_alive || managers.hp.current(subscriber.owner_uid) <= real {
        return Some(Vec::new());
    }
    Some(vec![BuffActRuleOp::subscriber_from_applier(
        RuleOp::Command(BattleCommand::Hp(HpCommand::SetCurrent(CurrentHpSet {
            origin: super::command_origin(subscriber)?,
            source_uid: subscriber.source_uid,
            target_uid: subscriber.owner_uid,
            value: real,
            config_effect: 0,
            effect_type: EffectType::Currenthpchange as i32,
        }))),
    )])
}

fn stored(buff: &sonettobuf::BuffInfo, act_id: i32) -> Option<i32> {
    buff.act_info
        .iter()
        .find(|info| info.act_id == Some(act_id))
        .and_then(|info| info.param.first().copied())
}

#[cfg(test)]
mod tests;
