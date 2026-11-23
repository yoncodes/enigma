use super::*;
use crate::engine::{
    event::{kind::EventKind, payload::GaugeChangeEvent, subscription::SubscriptionKey},
    manager::gauge::GaugeKey,
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};
use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

#[test]
fn positive_bloodtithe_gain_advances_the_configured_skill_threshold() {
    let ops = rule_ops(
        &managers(true),
        &SkillEffectCatalog::default(),
        &subscriber(),
        &gauge_event(5, 6),
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::ThresholdSkill(ThresholdSkillCommand {
            threshold: 6000,
            delta: 1000,
            invocation,
            ..
        }))] if invocation.plan.skill_id == 31200192
    ));
}

#[test]
fn buff_caused_bloodtithe_gain_advances_the_configured_skill_threshold() {
    let mut event = gauge_event(5, 6);
    let BattleEvent::GaugeChanged(change) = &mut event else {
        unreachable!();
    };
    change.source_skill_id = 0;

    assert!(matches!(
        rule_ops(
            &managers(true),
            &SkillEffectCatalog::default(),
            &subscriber(),
            &event,
        )
        .unwrap()
        .as_slice(),
        [RuleOp::Command(BattleCommand::ThresholdSkill(_))]
    ));
}

#[test]
fn spending_bloodtithe_does_not_advance_gain_progress() {
    assert!(
        rule_ops(
            &managers(true),
            &SkillEffectCatalog::default(),
            &subscriber(),
            &gauge_event(6, 2),
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn gains_before_the_required_status_do_not_advance_progress() {
    assert!(
        rule_ops(
            &managers(false),
            &SkillEffectCatalog::default(),
            &subscriber(),
            &gauge_event(0, 3),
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn unrelated_extra_action_bloodtithe_gain_advances_the_threshold_skill() {
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert_extra_kind(123, 1);

    assert!(matches!(
        rule_ops(&managers(true), &catalog, &subscriber(), &gauge_event(5, 7))
            .unwrap()
            .as_slice(),
        [RuleOp::Command(BattleCommand::ThresholdSkill(_))]
    ));
}

fn subscriber() -> BuffActSubscriber {
    BuffActSubscriber {
        owner_uid: 20,
        source_uid: 20,
        buff_uid: 1,
        buff_id: 31200184,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::GaugeChanged,
            DefinitionKey::new(1009, "BloodValueUseSkill"),
        ),
        act_type: "BloodValueUseSkill".to_owned(),
        effect_time: 0,
        effect_condition: 0,
        args: vec![31200124, 6, 31200192],
        raw: "1009#31200124#6#31200192".to_owned(),
    }
}

fn gauge_event(before: i32, after: i32) -> BattleEvent {
    BattleEvent::GaugeChanged(GaugeChangeEvent {
        origin: CommandOrigin {
            domain: RuleDomain::BuffAct,
            key: DefinitionKey::new(953, "BloodPoolTag"),
        },
        key: GaugeKey {
            kind: GaugeKind::Bloodtithe,
            owner: GaugeOwner::Team(1),
        },
        source_uid: 20,
        source_skill_id: 123,
        config_effect: 0,
        progress_raw_delta: (after - before).max(0).saturating_mul(1000),
        kind: GaugeChangeKind::Value,
        before,
        requested_delta: after - before,
        applied_delta: after - before,
        after,
        overflow: 0,
        before_max: Some(56),
        after_max: Some(56),
        enabled_before: true,
        enabled_after: true,
    })
}

fn managers(with_required_buff: bool) -> BattleManagers {
    crate::test_support::init_config();
    BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(20),
                current_hp: Some(1),
                buffs: with_required_buff
                    .then_some(BuffInfo {
                        uid: Some(2),
                        buff_id: Some(31200124),
                        ..Default::default()
                    })
                    .into_iter()
                    .collect(),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    })
}
