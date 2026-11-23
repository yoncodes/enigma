use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;
use crate::engine::{
    event::{kind::EventKind, subscription::SubscriptionKey},
    skill::{
        action::SkillActionEvent,
        rule::{DefinitionKey, output::BattleCommand},
    },
};

fn managers() -> BattleManagers {
    crate::test_support::init_config();
    BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(30880121),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    })
}

fn subscriber() -> BuffActSubscriber {
    BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 30880121,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::SkillAction,
            DefinitionKey::new(757, "UseSkillLoseHpNotFixed"),
        ),
        act_type: "UseSkillLoseHpNotFixed".to_owned(),
        effect_time: 201,
        effect_condition: 0,
        args: vec![50],
        raw: "757#50".to_owned(),
    }
}

fn event(mode: SkillExecutionMode) -> BattleEvent {
    BattleEvent::SkillAction(SkillActionEvent {
        source_uid: 10,
        card_index: 0,
        skill_id: 1,
        target_uid: -1,
        target_uids: vec![-1],
        attacked_target_uids: vec![-1],
        phase: SkillPhase::Immediate,
        skill_slot: 1,
        is_attack: true,
        rank: 1,
        skill_type: 0,
        effect_tag: 0,
        assassinate: false,
        ignore_riposte: false,
        damage_amount: 0,
        kill_count: 0,
        crit_count: 0,
        guard_break_count: 0,
        additional_moxie: 0,
        extra_skill_kind: 0,
        mode,
        teammate_injury_count: 0,
        teammate_injury_count_not_reset: 0,
        team_injury_count_round: 0,
        card_enchants: Vec::new(),
        buff_additions: Vec::new(),
    })
}

#[test]
fn active_cast_loses_current_hp_but_nested_skill_does_not() {
    let managers = managers();
    let subscriber = subscriber();

    assert!(matches!(
        loss_rule_ops(&managers, &subscriber, &event(SkillExecutionMode::Active))
            .unwrap()
            .as_slice(),
        [RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(
            HpLoss { amount: 50, .. }
        )))]
    ));
    assert!(
        loss_rule_ops(&managers, &subscriber, &event(SkillExecutionMode::Nested))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn active_cast_reads_all_configured_action_attributes() {
    let managers = managers();

    assert_eq!(
        attribute_deltas(&managers, 10, SkillExecutionMode::Active),
        vec![(AttrId::CriticalRate, 150)]
    );
    assert!(attribute_deltas(&managers, 10, SkillExecutionMode::Nested).is_empty());
}

#[test]
fn synchronization_reads_action_attributes_without_paying_active_cast_hp_cost() {
    let managers = managers();
    let subscriber = subscriber();

    assert_eq!(
        attribute_deltas(&managers, 10, SkillExecutionMode::Synchronization),
        vec![(AttrId::CriticalRate, 150)]
    );
    assert!(
        loss_rule_ops(
            &managers,
            &subscriber,
            &event(SkillExecutionMode::Synchronization)
        )
        .unwrap()
        .is_empty()
    );
}
