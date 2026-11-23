use sonettobuf::{Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::{
    event::subscription::SubscriptionKey,
    skill::{
        buff_act::{
            BuffActFrameOwner, BuffActFrameScope, BuffActFrameSource, registry::BuffActKind,
        },
        rule::DefinitionKey,
    },
};

#[test]
fn channel_spends_bloodtithe_stores_points_and_uses_configured_attack() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                team_type: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let origin = super::super::configured_command_origin(953, BuffActKind::BloodPoolTag).unwrap();
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            bloodtithe::rule::key(1),
            GaugeOperation::Enable { max: Some(56) },
        ))
        .unwrap();
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            bloodtithe::rule::key(1),
            GaugeOperation::ChangeValue { delta: 21 },
        ))
        .unwrap();

    let ops = rule_ops(
        &managers,
        &crate::engine::skill::effect::SkillEffectCatalog::default(),
        &subscriber(),
        &BattleEvent::Kind(EventKind::RoundEndAfterSettlement),
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::Gauge(GaugeCommand {
                operation: GaugeOperation::ChangeValue { delta: -18 },
                raw_delta: Some(-18_000),
                ..
            })),
            RuleOp::Command(BattleCommand::NuoDiKa(NuoDiKaCommand::Set {
                points: 9,
                bloodtithe_consumed: 18,
                max_points: 30,
                ..
            })),
            RuleOp::Skill(invocation),
            RuleOp::Command(BattleCommand::NuoDiKa(NuoDiKaCommand::Clear {
                owner_uid: 10,
            })),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
                target_uid: 10,
                selector: BuffRemoveSelector::Uid(43),
                ..
            }))),
        ] if invocation.plan.skill_id == 31200173
    ));
}

#[test]
fn channel_uses_configured_heal_without_state_when_cost_is_unavailable() {
    let mut managers = BattleManagers::default();
    let origin = super::super::configured_command_origin(953, BuffActKind::BloodPoolTag).unwrap();
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            bloodtithe::rule::key(1),
            GaugeOperation::Enable { max: Some(56) },
        ))
        .unwrap();

    let ops = rule_ops(
        &managers,
        &crate::engine::skill::effect::SkillEffectCatalog::default(),
        &subscriber(),
        &BattleEvent::Kind(EventKind::RoundEndAfterSettlement),
    )
    .unwrap();

    assert!(matches!(ops.as_slice(), [RuleOp::Skill(invocation)]
        if invocation.plan.skill_id == 31200182));
}

#[test]
fn channel_clear_is_an_owner_emitted_untargeted_event() {
    let mut managers = BattleManagers::default();
    let origin = super::super::configured_command_origin(953, BuffActKind::BloodPoolTag).unwrap();
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            bloodtithe::rule::key(1),
            GaugeOperation::Enable { max: Some(56) },
        ))
        .unwrap();
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            bloodtithe::rule::key(1),
            GaugeOperation::ChangeValue { delta: 6 },
        ))
        .unwrap();

    let ops = scoped_rule_ops(
        &managers,
        &crate::engine::skill::effect::SkillEffectCatalog::default(),
        &subscriber(),
        &BattleEvent::Kind(EventKind::RoundEndAfterSettlement),
    )
    .unwrap();

    assert_eq!(ops[3].scope, BuffActFrameScope::ActionFrame);
    assert_eq!(ops[3].source, BuffActFrameSource::Owner);
    assert_eq!(ops[3].frame_owner, BuffActFrameOwner::UntargetedEvent);
    assert!(!ops[3].group_with_siblings);
}

fn subscriber() -> BuffActSubscriber {
    BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 43,
        buff_id: 31200193,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::RoundEndAfterSettlement,
            DefinitionKey::new(1006, "NuoDiKaCastChannel"),
        ),
        act_type: "NuoDiKaCastChannel".to_owned(),
        effect_time: 304,
        effect_condition: 0,
        args: vec![1, 6, 10, 3, 31200173, 31200182],
        raw: "1006#1#6#10#3#31200173#31200182".to_owned(),
    }
}
