use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffConsume, BuffMarkerResult, BuffSelector, DepletedBuff},
        ex_point::{ExPointChange, ExPointChanges, ExPointCommand, ExPointCommandError},
    },
    skill::{
        rule::{
            CommandOrigin,
            output::{BattleCommand, RuleOp},
        },
        subscriber::BuffActSubscriber,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitMoxieAward {
    origin: CommandOrigin,
    buff_id: i32,
    recipient_uid: i64,
    delta: i32,
}

pub(crate) struct Settlement {
    pub change: ExPointChanges,
    pub marker: Option<BuffMarkerResult>,
}

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, super::registry::BuffActKind::ExPointAddByHit) {
        return None;
    }
    let BattleEvent::Hit(hit) = event else {
        return Some(Vec::new());
    };
    if !subscriber.owner_alive || hit.target_uid != subscriber.owner_uid {
        return Some(Vec::new());
    }
    let layer = managers
        .buff
        .snapshot(subscriber.owner_uid, subscriber.buff_uid)?
        .layer
        .unwrap_or_default();
    if layer <= 0 {
        return Some(Vec::new());
    }
    let origin = super::command_origin(subscriber)?;
    let delta = subscriber.args.first().copied().unwrap_or(1);
    Some(vec![
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
            origin,
            target_uid: subscriber.owner_uid,
            selector: BuffSelector::Uid(subscriber.buff_uid),
            amount: 1,
            depleted: DepletedBuff::Remove,
        }))),
        RuleOp::Command(BattleCommand::HitMoxieAward(HitMoxieAward {
            origin,
            buff_id: subscriber.buff_id,
            recipient_uid: hit.source_uid,
            delta,
        })),
    ])
}

pub(crate) fn execute(
    managers: &mut BattleManagers,
    award: HitMoxieAward,
) -> Result<Settlement, ExPointCommandError> {
    let change = managers.execute_ex_point(ExPointCommand::Change(ExPointChange {
        origin: award.origin,
        source_uid: award.recipient_uid,
        target_uid: award.recipient_uid,
        delta: award.delta,
        config_effect: 0,
        effect_type: sonettobuf::effect_type_enum::EffectType::Expointchange as i32,
    }))?;
    let marker = if matches!(change, ExPointChanges::Value { change, .. } if change.applied_delta > 0)
    {
        let key = award.origin.key;
        super::wire::find(key.opcode, key.type_name)
            .and_then(|definition| definition.markers(super::wire::WirePhase::Static).first())
            .map(|&effect_type| BuffMarkerResult {
                target_uid: award.recipient_uid,
                effect_type,
                effect_num: award.buff_id,
                buff_act_id: key.opcode,
            })
    } else {
        None
    };
    Ok(Settlement { change, marker })
}

#[cfg(test)]
mod tests;
