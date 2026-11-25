use crate::engine::{
    event::subscription::SubscriptionKey,
    manager::hp::HurtDamageFromType,
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};

use super::*;

fn subscriber() -> BuffActSubscriber {
    BuffActSubscriber {
        owner_uid: -1,
        source_uid: -1,
        buff_uid: 100,
        buff_id: 119381118,
        team_type: 2,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            crate::engine::event::kind::EventKind::BeAttacked,
            DefinitionKey::new(1148, "HurtToProgress"),
        ),
        act_type: "HurtToProgress".to_owned(),
        effect_time: 0,
        effect_condition: 0,
        args: vec![8, 9],
        raw: "1148#8#9".to_owned(),
    }
}

fn hit(resolved_damage: i32, damage_from: HurtDamageFromType) -> BattleEvent {
    BattleEvent::Hit(crate::engine::event::payload::HitEvent {
        origin: CommandOrigin {
            domain: RuleDomain::Skill,
            key: DefinitionKey::new(1, "SkillDamage"),
        },
        source_uid: 10,
        target_uid: -1,
        skill_id: 1,
        amount: 1_000,
        resolved_damage,
        shield_absorbed: 0,
        career_restraint: false,
        damage_from,
        share_count: 0,
        assassinate: false,
        ignore_riposte: false,
    })
}

#[test]
fn converts_damage_using_configured_permille() {
    let event = hit(3_874, HurtDamageFromType::Skill);
    assert!(matches!(
        rule_ops(&subscriber(), &event).as_deref(),
        Some([RuleOp::Command(BattleCommand::FightProgressValue(
            FightProgressValueCommand {
                progress_id: 8,
                delta: 34,
                ..
            }
        ))])
    ));
}

#[test]
fn linked_additional_damage_does_not_fill_echo_volume() {
    let event = hit(4_955, HurtDamageFromType::Additional);
    assert!(matches!(
        rule_ops(&subscriber(), &event).as_deref(),
        Some([])
    ));
}
