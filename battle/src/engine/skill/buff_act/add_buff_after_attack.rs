use crate::engine::{
    event::payload::BattleEvent,
    manager::buff::{BuffCommand, BuffGrant},
    skill::{
        action::SkillPhase,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

use super::registry::BuffActKind;

pub fn rule_ops(subscriber: &BuffActSubscriber, event: &BattleEvent) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::AddBuffAfterAttack) {
        return None;
    }
    let BattleEvent::SkillAction(action) = event else {
        return None;
    };
    if action.phase != SkillPhase::AfterDamage
        || !action.is_attack
        || action.source_uid != subscriber.owner_uid
    {
        return Some(Vec::new());
    }
    let [buff_id, amount] = subscriber.args.as_slice() else {
        return None;
    };
    Some(vec![RuleOp::Command(BattleCommand::Buff(
        BuffCommand::GrantIndependent(BuffGrant {
            origin: super::command_origin(subscriber)?,
            source_uid: action.source_uid,
            target_uid: subscriber.owner_uid,
            buff_id: *buff_id,
            amount: Some(*amount),
            occurrences: 1,
            child_uid_reservations: 0,
        }),
    ))])
}

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [buff_id, amount] if *buff_id > 0 && *amount > 0)
}

#[cfg(test)]
mod tests {
    use crate::engine::{
        event::{kind::EventKind, subscription::SubscriptionKey},
        skill::{
            action::{SkillActionEvent, SkillExecutionMode},
            rule::DefinitionKey,
        },
    };

    use super::*;

    fn subscriber() -> BuffActSubscriber {
        BuffActSubscriber {
            owner_uid: 99_998,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 31_080_144,
            team_type: 1,
            owner_alive: true,
            amount: 0,
            key: SubscriptionKey::new(
                EventKind::SkillAction,
                DefinitionKey::new(884, "AddToBuffEntity3"),
            ),
            act_type: "AddToBuffEntity3".to_owned(),
            effect_time: 908,
            effect_condition: 0,
            args: vec![31_080_145, 1],
            raw: "884#31080145#1".to_owned(),
        }
    }

    fn event(source_uid: i64, phase: SkillPhase) -> BattleEvent {
        BattleEvent::SkillAction(SkillActionEvent {
            source_uid,
            skill_id: 2_240_001,
            target_uid: -1,
            target_uids: vec![-1],
            attacked_target_uids: vec![-1],
            phase,
            skill_slot: 0,
            is_attack: true,
            rank: 0,
            skill_type: 0,
            effect_tag: 2,
            assassinate: false,
            damage_amount: 100,
            kill_count: 0,
            crit_count: 1,
            guard_break_count: 0,
            additional_moxie: 0,
            extra_skill_kind: 0,
            mode: SkillExecutionMode::Nested,
            teammate_injury_count: 0,
            teammate_injury_count_not_reset: 0,
            team_injury_count_round: 0,
            card_enchants: Vec::new(),
        })
    }

    #[test]
    fn grants_configured_buff_to_the_attacking_carrier_after_damage() {
        assert!(matches!(
            rule_ops(&subscriber(), &event(99_998, SkillPhase::AfterDamage)).as_deref(),
            Some([RuleOp::Command(BattleCommand::Buff(
                BuffCommand::GrantIndependent(BuffGrant {
                    source_uid: 99_998,
                    target_uid: 99_998,
                    buff_id: 31_080_145,
                    amount: Some(1),
                    occurrences: 1,
                    child_uid_reservations: 0,
                    ..
                })
            ))])
        ));
    }

    #[test]
    fn ignores_other_attackers_and_earlier_phases() {
        assert_eq!(
            rule_ops(&subscriber(), &event(10, SkillPhase::AfterDamage)),
            Some(Vec::new())
        );
        assert_eq!(
            rule_ops(&subscriber(), &event(99_998, SkillPhase::Damage)),
            Some(Vec::new())
        );
    }
}
