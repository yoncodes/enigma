use crate::engine::{
    event::payload::{BattleEvent, BuffFeatureTriggeredEvent},
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffConsume, BuffManager, BuffSelector, DepletedBuff},
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

use super::{
    BuffActRuleOp,
    add_to_target::RecipientScope,
    registry::{BuffActKind, RuntimeContext},
};

pub(crate) fn add_to_target(context: &mut RuntimeContext<'_>) -> Option<Vec<BuffActRuleOp>> {
    let subscriber = context.subscriber;
    let event = context.event?;
    let counted = context
        .managers
        .catalog()
        .buff_has_effect_count(subscriber.buff_id)
        && context.managers.buff.buff_has_act(
            subscriber.owner_uid,
            subscriber.buff_uid,
            BuffActKind::Bullet,
        );
    let recipients = recipients(context.managers, subscriber, counted);
    let mut ops = super::add_to_target::scoped_rule_ops(
        context.managers,
        subscriber,
        event,
        context.catalog,
        context.pool,
        recipients,
    )?;
    if let BattleEvent::SkillAction(action) = event
        && action.source_uid == subscriber.owner_uid
        && action.is_attack
        && counted
    {
        ops.push(BuffActRuleOp::causing(RuleOp::Publish(
            BattleEvent::BuffFeatureTriggered(BuffFeatureTriggeredEvent {
                owner_uid: subscriber.owner_uid,
                source_uid: subscriber.source_uid,
                target_uid: action.target_uid,
                buff_uid: subscriber.buff_uid,
                buff_id: subscriber.buff_id,
                act_id: subscriber.key.definition.opcode,
            }),
        )));
    }
    Some(ops)
}

fn recipients(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    counted: bool,
) -> RecipientScope {
    if counted
        && matches!(subscriber.args.as_slice(), [1, _])
        && managers
            .catalog()
            .buff_feature_rows(subscriber.buff_id)
            .len()
            == 2
        && matches!(managers.buff.definition_features(subscriber.buff_id).as_slice(), [grant, charge]
            if grant.act_id() == Some(subscriber.key.definition.opcode)
                && grant.act_type == subscriber.act_type
                && super::is_kind(charge, BuffActKind::Bullet))
    {
        RecipientScope::Primary
    } else {
        RecipientScope::Action
    }
}

pub(crate) fn reaction(context: &mut RuntimeContext<'_>) -> Option<Vec<BuffActRuleOp>> {
    let subscriber = context.subscriber;
    let event = context.event?;
    if let BattleEvent::BuffFeatureTriggered(trigger) = event {
        if trigger.buff_uid != subscriber.buff_uid {
            return Some(Vec::new());
        }
        let Some(definition) = super::registry::find(trigger.act_id, "AddToTarget") else {
            return Some(Vec::new());
        };
        let origin = crate::engine::skill::rule::CommandOrigin {
            domain: crate::engine::skill::rule::RuleDomain::BuffAct,
            key: definition.key,
        };
        let consume = BuffCommand::ConsumeAttackCharge(BuffConsume {
            origin,
            target_uid: subscriber.owner_uid,
            selector: BuffSelector::Uid(subscriber.buff_uid),
            amount: 1,
            depleted: DepletedBuff::Remove,
        });
        return Some(vec![BuffActRuleOp::separate_subscriber_from_owner(
            RuleOp::Command(BattleCommand::Buff(consume)),
        )]);
    }
    rule_ops(subscriber, event).map(|ops| {
        ops.into_iter()
            .map(BuffActRuleOp::subscriber_from_applier)
            .collect()
    })
}

pub(crate) fn retains_final_charge(buffs: &BuffManager, owner_uid: i64, buff_uid: i64) -> bool {
    buffs.has_buff_act_kind(owner_uid, BuffActKind::LockBulletCountDecr)
        && buffs.buff_has_act(owner_uid, buff_uid, BuffActKind::Bullet)
        && buffs
            .snapshot(owner_uid, buff_uid)
            .is_some_and(|buff| buff.count == Some(1))
}

pub fn rule_ops(subscriber: &BuffActSubscriber, event: &BattleEvent) -> Option<Vec<RuleOp>> {
    if !subscriber.owner_alive {
        return Some(Vec::new());
    }
    match event {
        BattleEvent::SkillAction(action)
            if action.source_uid == subscriber.owner_uid && action.is_attack =>
        {
            Some(vec![RuleOp::Publish(BattleEvent::BuffFeatureTriggered(
                BuffFeatureTriggeredEvent {
                    owner_uid: subscriber.owner_uid,
                    source_uid: subscriber.source_uid,
                    target_uid: action.target_uid,
                    buff_uid: subscriber.buff_uid,
                    buff_id: subscriber.buff_id,
                    act_id: subscriber.key.definition.opcode,
                },
            ))])
        }
        _ => Some(Vec::new()),
    }
}

#[cfg(test)]
mod tests;
