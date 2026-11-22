use sonettobuf::BuffActInfo;

use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        buff::{
            BuffActInfoMarkerResult, BuffCommand, BuffConsume, BuffRemove, BuffRemoveSelector,
            BuffSelector, BuffSetState, DepletedBuff,
        },
    },
    skill::{
        action::SkillInvocation,
        buff_act::registry::BuffActKind,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [skill_id, resource_buff_id, 0, 50]
        if *skill_id > 0 && *resource_buff_id > 0)
}

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::ConsumeBuffAddBuffContinueChannel)
        || event.kind() != EventKind::RoundEnd
        || !supports(&subscriber.args)
    {
        return None;
    }
    let [_, resource_buff_id, initial_cost, cost_step] = subscriber.args.as_slice() else {
        return None;
    };
    let origin = super::command_origin(subscriber)?;
    let channel = managers
        .buff
        .snapshot(subscriber.owner_uid, subscriber.buff_uid)?;
    let current_cost = channel
        .act_info
        .iter()
        .find(|info| info.act_id == Some(subscriber.key.definition.opcode))
        .and_then(|info| info.str_param.as_deref())
        .map(str::parse::<i32>)
        .transpose()
        .ok()?
        .unwrap_or(*initial_cost);
    if current_cost < 0 {
        return None;
    }
    let available = managers
        .buff
        .buff_id_or_type_amount(subscriber.owner_uid, *resource_buff_id);
    if available < current_cost {
        return Some(vec![RuleOp::Command(BattleCommand::Buff(
            BuffCommand::Remove(BuffRemove {
                origin,
                target_uid: subscriber.owner_uid,
                selector: BuffRemoveSelector::Uid(subscriber.buff_uid),
            }),
        ))]);
    }

    let next_cost = current_cost.saturating_add(*cost_step);
    let mut act_info = channel.act_info;
    if let Some(info) = act_info
        .iter_mut()
        .find(|info| info.act_id == Some(subscriber.key.definition.opcode))
    {
        info.str_param = Some(next_cost.to_string());
    } else {
        act_info.push(BuffActInfo {
            act_id: Some(subscriber.key.definition.opcode),
            param: Vec::new(),
            str_param: Some(next_cost.to_string()),
        });
    }

    Some(vec![
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
            origin,
            target_uid: subscriber.owner_uid,
            selector: BuffSelector::IdOrType(*resource_buff_id),
            amount: current_cost,
            depleted: DepletedBuff::Remove,
        }))),
        RuleOp::Skill(SkillInvocation::from(super::use_skill::linked(subscriber)?)),
        RuleOp::Command(BattleCommand::Buff(BuffCommand::SetInternalState(
            BuffSetState {
                origin,
                target_uid: subscriber.owner_uid,
                buff_uid: subscriber.buff_uid,
                ex_info: None,
                params: None,
                act_info: Some(act_info),
            },
        ))),
        RuleOp::BuffActInfoMarker(BuffActInfoMarkerResult {
            target_uid: subscriber.owner_uid,
            buff_uid: subscriber.buff_uid,
            act_id: subscriber.key.definition.opcode,
            params: Vec::new(),
            str_param: Some(next_cost.to_string()),
            team_type: 0,
        }),
    ])
}

#[cfg(test)]
mod tests;
