use crate::engine::{
    event::payload::{BattleEvent, BuffFeatureTriggeredEvent},
    skill::{rule::output::RuleOp, subscriber::BuffActSubscriber},
};

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
mod tests {
    use crate::engine::event::{kind::EventKind, subscription::SubscriptionKey};

    use super::*;

    #[test]
    fn skill_cast_publishes_the_feature_without_owning_carrier_consumption() {
        let mut subscriber = BuffActSubscriber {
            owner_uid: 10,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 30,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::SkillCast,
                crate::engine::skill::rule::DefinitionKey::new(827, "Bullet"),
            ),
            act_type: "Bullet".to_owned(),
            effect_time: 208,
            effect_condition: 3,
            args: Vec::new(),
            raw: "827".to_owned(),
        };

        let definition = super::super::registry::find(827, "Bullet").unwrap();
        assert_eq!(
            definition.runtime.frame_scope,
            super::super::registry::RuntimeFrameScope::SubscriberFrame
        );
        assert_eq!(
            definition.runtime.frame_source,
            super::super::registry::RuntimeFrameSource::Applier
        );
        let event = BattleEvent::SkillAction(crate::engine::skill::action::SkillActionEvent {
            source_uid: 10,
            target_uid: -1,
            skill_id: 100,
            is_attack: true,
            target_uids: vec![-1],
            attacked_target_uids: vec![-1],
            phase: crate::engine::skill::action::SkillPhase::HitPassives,
            skill_slot: 0,
            rank: 1,
            skill_type: 0,
            effect_tag: 1,
            assassinate: false,
            ignore_riposte: false,
            damage_amount: 1,
            kill_count: 0,
            crit_count: 0,
            guard_break_count: 0,
            additional_moxie: 0,
            extra_skill_kind: 0,
            mode: crate::engine::skill::action::SkillExecutionMode::Active,
            teammate_injury_count: 0,
            teammate_injury_count_not_reset: 0,
            team_injury_count_round: 0,
            card_enchants: Vec::new(),
            buff_additions: Vec::new(),
        });
        assert!(matches!(
            rule_ops(&subscriber, &event).as_deref(),
            Some([RuleOp::Publish(BattleEvent::BuffFeatureTriggered(trigger))])
                if trigger.owner_uid == 10
                    && trigger.target_uid == -1
                    && trigger.buff_uid == 20
                    && trigger.act_id == 827
        ));
        subscriber.owner_alive = false;
        assert_eq!(rule_ops(&subscriber, &event), Some(Vec::new()));
    }
}
