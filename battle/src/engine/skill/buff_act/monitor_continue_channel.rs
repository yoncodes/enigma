use crate::engine::{
    event::payload::BattleEvent,
    manager::BattleManagers,
    manager::buff::{
        BuffCommand, BuffConsume, BuffGrant, BuffGrantUidReservation, BuffManager, BuffSelector,
        DepletedBuff,
    },
    skill::{
        action::{SkillInvocation, SkillTarget},
        buff_act::BuffActRuleOp,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
        target::TargetPool,
    },
};

pub fn rule_ops(
    managers: &BattleManagers,
    pool: &TargetPool,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !can_run(subscriber, &managers.buff) {
        return Some(Vec::new());
    }
    let action_target = match event {
        BattleEvent::AllyAction(action)
            if pool.source_is_attacker(action.source_uid)
                != pool.source_is_attacker(subscriber.owner_uid) =>
        {
            Some(action.source_uid)
        }
        BattleEvent::Kind(crate::engine::event::kind::EventKind::RoundEndFinalSettlement) => None,
        _ => return Some(Vec::new()),
    };
    let casts = if action_target.is_some() {
        1
    } else {
        managers
            .buff
            .buff_id_or_type_amount(subscriber.owner_uid, *subscriber.args.first()?)
    };
    let plan = super::use_skill::linked(subscriber)?;
    let mut ops = Vec::new();
    for _ in 0..casts {
        if let Some(reservation) = reward_uid_reservation(subscriber) {
            ops.push(reservation);
        }
        let mut invocation = SkillInvocation::from(plan);
        if let Some(target_uid) = action_target {
            invocation.target = SkillTarget::Explicit(target_uid);
        }
        invocation.extra_skill_kind =
            Some(crate::engine::skill::condition::extra::ExtraSkillKind::FollowUp);
        invocation.mode = crate::engine::skill::action::SkillExecutionMode::Active;
        invocation.rate_modifier = rate_modifier(subscriber, &managers.buff);
        ops.push(RuleOp::Skill(invocation));
        ops.extend(after_skill(subscriber));
    }
    Some(ops)
}

pub fn scoped_rule_ops(
    managers: &BattleManagers,
    pool: &TargetPool,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<BuffActRuleOp>> {
    Some(scope_rule_ops(rule_ops(managers, pool, subscriber, event)?))
}

fn scope_rule_ops(ops: Vec<RuleOp>) -> Vec<BuffActRuleOp> {
    ops.into_iter()
        .map(|op| {
            if matches!(op, RuleOp::Skill(_)) {
                BuffActRuleOp::subscriber_from_owner(op)
            } else {
                BuffActRuleOp::causing(op)
            }
        })
        .collect()
}

fn reward_uid_reservation(subscriber: &BuffActSubscriber) -> Option<RuleOp> {
    let origin = super::command_origin(subscriber)?;
    let reward_buff_id = *subscriber.args.get(3)?;
    Some(RuleOp::Command(BattleCommand::Buff(
        BuffCommand::ReserveGrantUid(BuffGrantUidReservation {
            origin,
            target_uid: subscriber.owner_uid,
            buff_id: reward_buff_id,
        }),
    )))
}

pub fn can_run(subscriber: &BuffActSubscriber, buffs: &BuffManager) -> bool {
    !super::subscriber_is_kind(
        subscriber,
        super::registry::BuffActKind::MonitorContinueChannel,
    ) || subscriber
        .args
        .first()
        .is_some_and(|lock_buff_id| buffs.has_buff_id_or_type(subscriber.owner_uid, *lock_buff_id))
}

pub fn rate_bonus(subscriber: &BuffActSubscriber, buffs: &BuffManager) -> i32 {
    rate_modifier(subscriber, buffs)
        .and_then(|modifier| modifier.fixed_value())
        .unwrap_or_default()
}

fn rate_modifier(
    subscriber: &BuffActSubscriber,
    buffs: &BuffManager,
) -> Option<crate::engine::skill::action::SkillRateModifier> {
    if !super::subscriber_is_kind(
        subscriber,
        super::registry::BuffActKind::MonitorContinueChannel,
    ) {
        return None;
    }
    let buff_id = subscriber.args.get(3).copied()?;
    if buffs.has_buff_id(subscriber.owner_uid, buff_id) {
        return None;
    }
    buffs
        .definition_features(buff_id)
        .into_iter()
        .find_map(|feature| {
            let delta = super::skill_rate_bonus(&feature);
            (delta != 0).then(|| {
                crate::engine::skill::action::SkillRateModifier::retribution_lane(
                    feature.values[0],
                    delta,
                )
            })
        })
}

pub fn after_skill(subscriber: &BuffActSubscriber) -> Vec<RuleOp> {
    if !super::subscriber_is_kind(
        subscriber,
        super::registry::BuffActKind::MonitorContinueChannel,
    ) {
        return Vec::new();
    }
    let (Some(origin), Some(&lock_buff_id), Some(&reward_buff_id)) = (
        super::command_origin(subscriber),
        subscriber.args.first(),
        subscriber.args.get(3),
    ) else {
        return Vec::new();
    };
    vec![
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
            origin,
            target_uid: subscriber.owner_uid,
            selector: BuffSelector::IdOrType(lock_buff_id),
            amount: 1,
            depleted: DepletedBuff::Remove,
        }))),
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: subscriber.owner_uid,
            target_uid: subscriber.owner_uid,
            buff_id: reward_buff_id,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        event::{kind::EventKind, subscription::SubscriptionKey},
        skill::action::SkillRequest,
    };

    #[test]
    fn after_skill_keeps_the_configured_consume_then_grant_order() {
        let subscriber = BuffActSubscriber {
            owner_uid: 10,
            source_uid: 10,
            buff_uid: 2,
            buff_id: 100,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::RoundEnd,
                crate::engine::skill::rule::DefinitionKey::new(1024, "MonitorContinueChannel"),
            ),
            act_type: "MonitorContinueChannel".to_owned(),
            effect_time: 302,
            effect_condition: 0,
            args: vec![200, 0, 0, 300],
            raw: String::new(),
        };

        assert!(matches!(
            after_skill(&subscriber).as_slice(),
            [
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                    selector: BuffSelector::IdOrType(200),
                    ..
                }))),
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                    buff_id: 300,
                    ..
                })))
            ]
        ));
    }

    #[test]
    fn reward_uid_reservation_uses_the_configured_output_buff() {
        let subscriber = BuffActSubscriber {
            owner_uid: 10,
            source_uid: 10,
            buff_uid: 2,
            buff_id: 100,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::RoundEnd,
                crate::engine::skill::rule::DefinitionKey::new(1024, "MonitorContinueChannel"),
            ),
            act_type: "MonitorContinueChannel".to_owned(),
            effect_time: 302,
            effect_condition: 0,
            args: vec![200, 0, 0, 300],
            raw: String::new(),
        };

        assert!(matches!(
            reward_uid_reservation(&subscriber),
            Some(RuleOp::Command(BattleCommand::Buff(
                BuffCommand::ReserveGrantUid(BuffGrantUidReservation {
                    target_uid: 10,
                    buff_id: 300,
                    ..
                })
            )))
        ));
    }

    #[test]
    fn channel_cast_owns_only_the_linked_skill_frame() {
        crate::test_support::init_config();
        let invocation = SkillInvocation::from(SkillRequest {
            source_uid: 10,
            skill_id: 20,
        });
        let command = RuleOp::Command(BattleCommand::Buff(BuffCommand::ReserveGrantUid(
            BuffGrantUidReservation {
                origin: super::super::configured_command_origin(
                    1024,
                    super::super::registry::BuffActKind::MonitorContinueChannel,
                )
                .unwrap(),
                target_uid: 10,
                buff_id: 30,
            },
        )));

        let scoped = scope_rule_ops(vec![command, RuleOp::Skill(invocation)]);

        assert_eq!(
            scoped[0].scope,
            super::super::BuffActFrameScope::CausingFrame
        );
        assert_eq!(
            scoped[1].scope,
            super::super::BuffActFrameScope::SubscriberFrame
        );
        assert_eq!(scoped[1].source, super::super::BuffActFrameSource::Owner);
    }
}
