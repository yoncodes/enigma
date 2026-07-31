use crate::engine::{
    event::payload::BattleEvent,
    manager::buff::{BuffCommand, BuffConsume, BuffSelector, DepletedBuff},
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [amount, tags @ ..] if *amount > 0 && !tags.is_empty())
}

pub fn rule_ops(subscriber: &BuffActSubscriber, event: &BattleEvent) -> Option<Vec<RuleOp>> {
    let BattleEvent::SkillAction(action) = event else {
        return Some(Vec::new());
    };
    let (&amount, tags) = subscriber.args.split_first()?;
    if !action.target_uids.contains(&subscriber.owner_uid) || !tags.contains(&action.effect_tag) {
        return Some(Vec::new());
    }
    Some(vec![RuleOp::Command(BattleCommand::Buff(
        BuffCommand::Consume(BuffConsume {
            origin: super::command_origin(subscriber)?,
            target_uid: subscriber.owner_uid,
            selector: BuffSelector::Uid(subscriber.buff_uid),
            amount,
            depleted: DepletedBuff::Remove,
        }),
    ))])
}

#[cfg(test)]
mod tests {
    use crate::engine::{
        event::{kind::EventKind, subscription::SubscriptionKey},
        skill::{
            action::{SkillActionEvent, SkillExecutionMode, SkillPhase},
            rule::DefinitionKey,
        },
    };

    use super::*;

    fn subscriber() -> BuffActSubscriber {
        BuffActSubscriber {
            owner_uid: 20,
            source_uid: -3,
            buff_uid: 7,
            buff_id: 12110011,
            team_type: 1,
            owner_alive: true,
            amount: 3,
            key: SubscriptionKey::new(
                EventKind::SkillCast,
                DefinitionKey::new(804, "DisperseByTag"),
            ),
            act_type: "DisperseByTag".to_owned(),
            effect_time: 208,
            effect_condition: 0,
            args: vec![1, 4, 5, 6, 9],
            raw: "804#1#4,5,6,9".to_owned(),
        }
    }

    fn cast(effect_tag: i32, target_uids: Vec<i64>) -> BattleEvent {
        BattleEvent::SkillAction(SkillActionEvent {
            source_uid: 10,
            skill_id: 1,
            target_uid: target_uids.first().copied().unwrap_or_default(),
            target_uids,
            attacked_target_uids: Vec::new(),
            phase: SkillPhase::HitPassives,
            skill_slot: 0,
            is_attack: false,
            rank: 1,
            skill_type: 0,
            effect_tag,
            assassinate: false,
            damage_amount: 0,
            kill_count: 0,
            crit_count: 0,
            additional_moxie: 0,
            extra_skill_kind: 0,
            mode: SkillExecutionMode::Active,
            teammate_injury_count: 0,
            teammate_injury_count_not_reset: 0,
            team_injury_count_round: 0,
            card_enchants: Vec::new(),
            buff_additions: Vec::new(),
        })
    }

    #[test]
    fn configured_support_skill_consumes_one_holder_stack() {
        assert!(matches!(
            rule_ops(&subscriber(), &cast(6, vec![20])).as_deref(),
            Some([RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(
                BuffConsume {
                    target_uid: 20,
                    selector: BuffSelector::Uid(7),
                    amount: 1,
                    depleted: DepletedBuff::Remove,
                    ..
                }
            )))])
        ));
        assert_eq!(
            rule_ops(&subscriber(), &cast(3, vec![20])),
            Some(Vec::new())
        );
        assert_eq!(
            rule_ops(&subscriber(), &cast(6, vec![21])),
            Some(Vec::new())
        );
    }
}
