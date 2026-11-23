use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;
use crate::engine::{
    event::{kind::EventKind, payload::HitEvent, subscription::SubscriptionKey},
    manager::hp::HurtDamageFromType,
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};

const VILA: i64 = 30;

#[test]
fn reaction_waits_for_attacked_skills() {
    assert_eq!(
        super::super::registry::reaction_timing(10010, "BeAttackedFromUseSkill"),
        crate::engine::event::subscription::ReactionTiming::HpThresholds
    );
}

fn ally(uid: i64, current_hp: i32) -> FightEntityInfo {
    FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(current_hp),
        attr: Some(HeroAttribute {
            hp: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn managers(eagle_hp: i32, bkornblume_hp: i32) -> BattleManagers {
    BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                ally(10, eagle_hp),
                ally(20, bkornblume_hp),
                ally(VILA, 1_000),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![ally(-1, 1_000)],
            ..Default::default()
        }),
        ..Default::default()
    })
}

fn subscriber(owner_uid: i64, buff_uid: i64) -> BuffActSubscriber {
    BuffActSubscriber {
        owner_uid,
        source_uid: VILA,
        buff_uid,
        buff_id: 30870301,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::BeAttacked,
            DefinitionKey::new(10010, "BeAttackedFromUseSkill"),
        ),
        act_type: "BeAttackedFromUseSkill".to_owned(),
        effect_time: 209,
        effect_condition: 0,
        args: vec![700, 30870121, 2],
        raw: "10010#700#30870121#2".to_owned(),
    }
}

fn hit(target_uid: i64) -> BattleEvent {
    BattleEvent::Hit(HitEvent {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(1, "Damage"),
        },
        source_uid: -1,
        target_uid,
        skill_id: 114200311,
        amount: 100,
        resolved_damage: 100,
        shield_absorbed: 0,
        career_restraint: false,
        damage_from: HurtDamageFromType::Skill,
        share_count: 0,
        assassinate: false,
        ignore_riposte: false,
    })
}

fn casts(ops: &[RuleOp]) -> Option<(i64, i32, i64)> {
    let [RuleOp::Skill(invocation)] = ops else {
        return None;
    };
    let SkillTarget::Explicit(target) = invocation.target else {
        return None;
    };
    let cap = invocation
        .round_cap
        .expect("the cast carries Vila's round cap");
    assert_eq!((cap.owner_uid, cap.buff_uid, cap.limit), (VILA, 0, 2));
    Some((invocation.plan.source_uid, invocation.plan.skill_id, target))
}

#[test]
fn source_casts_on_a_holder_left_below_the_threshold() {
    let managers = managers(600, 1_000);
    let ops = rule_ops(&managers, &subscriber(10, 1), &hit(10)).unwrap();
    assert_eq!(casts(&ops), Some((VILA, 30870121, 10)));

    let managers = managers_at(700);
    assert!(
        rule_ops(&managers, &subscriber(10, 1), &hit(10))
            .unwrap()
            .is_empty()
    );
    assert!(
        rule_ops(&managers, &subscriber(10, 1), &hit(20))
            .unwrap()
            .is_empty()
    );
}

fn managers_at(eagle_hp: i32) -> BattleManagers {
    managers(eagle_hp, 1_000)
}

#[test]
fn a_holder_killed_by_the_hit_is_not_healed() {
    let managers = managers(0, 1_000);
    assert!(
        rule_ops(&managers, &subscriber(10, 1), &hit(10))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn the_round_cap_is_shared_by_every_ally_copy() {
    let mut managers = managers(500, 500);
    let key = DefinitionKey::new(10010, "BeAttackedFromUseSkill");
    managers.mark_buff_act_fired(VILA, 0, key);
    assert!(
        rule_ops(&managers, &subscriber(10, 1), &hit(10)).is_some_and(|ops| casts(&ops).is_some())
    );
    managers.mark_buff_act_fired(VILA, 0, key);
    assert!(
        rule_ops(&managers, &subscriber(20, 2), &hit(20))
            .unwrap()
            .is_empty()
    );
    managers.begin_round();
    assert!(
        rule_ops(&managers, &subscriber(20, 2), &hit(20)).is_some_and(|ops| casts(&ops).is_some())
    );
}
