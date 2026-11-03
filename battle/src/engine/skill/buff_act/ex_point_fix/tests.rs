use crate::engine::{
    event::{kind::EventKind, payload::HitEvent, subscription::SubscriptionKey},
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};

use super::*;

fn subscriber(opcode: i32, args: Vec<i32>) -> BuffActSubscriber {
    BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 500503,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::BeAttacked,
            DefinitionKey::new(opcode, "ExPointFix"),
        ),
        act_type: "ExPointFix".to_owned(),
        effect_time: 211,
        effect_condition: 0,
        args,
        raw: String::new(),
    }
}

fn hit() -> BattleEvent {
    BattleEvent::Hit(HitEvent {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(1, "Damage"),
        },
        source_uid: -1,
        target_uid: 10,
        skill_id: 1,
        amount: 100,
        resolved_damage: 100,
        shield_absorbed: 0,
        career_restraint: false,
        damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
        share_count: 0,
        assassinate: false,
        ignore_riposte: false,
    })
}

#[test]
fn attacked_counter_changes_the_configured_participant_moxie() {
    assert!(supports_holder(&[0, 1]));
    assert!(supports_attacker(&[1, -2]));
    assert!(!supports_holder(&[1, 1]));
    assert!(!supports_attacker(&[0, -1]));

    let holder = rule_ops(&subscriber(306, vec![0, 1]), &hit()).unwrap();
    let attacker = rule_ops(&subscriber(307, vec![1, -2]), &hit()).unwrap();

    assert!(matches!(
        holder.as_slice(),
        [RuleOp::Command(BattleCommand::ExPoint(
            ExPointCommand::Change(ExPointChange {
                source_uid: 10,
                target_uid: 10,
                delta: 1,
                ..
            })
        ))]
    ));
    assert!(matches!(
        attacker.as_slice(),
        [RuleOp::Command(BattleCommand::ExPoint(
            ExPointCommand::Reduce(ExPointChange {
                source_uid: 10,
                target_uid: -1,
                delta: -2,
                ..
            })
        ))]
    ));
}
