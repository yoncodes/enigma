use sonettobuf::{CardInfo, Fight, FightEntityInfo, FightTeam, PowerInfo};

use super::*;
use crate::engine::{
    manager::card::{CardPlay, CardSetup},
    runtime::determinism::RoundDeterminism,
    skill::{
        action::SkillModifiers,
        target::{TargetContext, TargetPool},
    },
};

fn card(uid: i64, hero_id: i32, skill_id: i32) -> CardInfo {
    CardInfo {
        uid: Some(uid),
        hero_id: Some(hero_id),
        skill_id: Some(skill_id),
        ..Default::default()
    }
}

fn emitted_ops(power: i32, cost: i32, hand: Vec<CardInfo>) -> Vec<RuleOp> {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3039),
                    entity_type: Some(1),
                    team_type: Some(1),
                    current_hp: Some(100),
                    skill_group1: vec![303901111, 303901121, 303901131],
                    skill_group2: vec![303901211, 303901221, 303901231],
                    power_infos: vec![PowerInfo {
                        power_id: Some(EUREKA_RESOURCE_ID),
                        num: Some(power),
                        max: Some(10),
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    model_id: Some(3053),
                    entity_type: Some(1),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = crate::engine::manager::BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            deck_num: hand.len() as i32,
            hand,
            draw_pile: Vec::new(),
        }))
        .unwrap();
    while !managers.card.hand().is_empty() {
        managers
            .execute_card(CardCommand::Play(CardPlay {
                origin: crate::engine::skill::rule::CommandOrigin {
                    domain: crate::engine::skill::rule::RuleDomain::Behavior,
                    key: crate::engine::skill::rule::DefinitionKey::new(
                        60265,
                        "ConsumePowerUpdateActorCard",
                    ),
                },
                hand_index: 0,
                target_uid: None,
                chosen_skill_id: None,
                choice: None,
                recorded_skill: None,
            }))
            .unwrap();
    }
    let event = BattleEvent::ActionQueueCommitted {
        team: 1,
        emitter_uid: 10,
        cards: Vec::new(),
    };
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();
    let behavior = ParsedBehavior::new(60265, "ConsumePowerUpdateActorCard", vec![cost]);

    Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 303901441,
            transfer_count: 1,
            event: Some(&event),
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap()
}

#[test]
fn spends_power_to_upgrade_owned_cards_in_queue_order() {
    let ops = emitted_ops(
        3,
        1,
        vec![
            card(10, 3039, 303901211),
            card(20, 3053, 30530161),
            card(10, 3039, 303901111),
        ],
    );

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::Eureka(EurekaCommand::Change(EurekaChange {
                delta: -3,
                ..
            }))),
            RuleOp::Command(BattleCommand::Card(CardCommand::ChangeQueuedRanks {
                changes: upgrades,
                ..
            }))
        ] if upgrades == &vec![
            QueuedCardRankChange { card_index: 1, levels: 2 },
            QueuedCardRankChange { card_index: 3, levels: 1 },
        ]
    ));
}

#[test]
fn cost_two_stops_after_the_first_affordable_rank() {
    let ops = emitted_ops(
        3,
        2,
        vec![card(10, 3039, 303901211), card(10, 3039, 303901111)],
    );

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::Eureka(EurekaCommand::Change(EurekaChange {
                delta: -2,
                ..
            }))),
            RuleOp::Command(BattleCommand::Card(CardCommand::ChangeQueuedRanks {
                changes,
                ..
            }))
        ] if changes == &vec![QueuedCardRankChange { card_index: 1, levels: 1 }]
    ));
}

#[test]
fn non_rankable_owned_card_spends_nothing() {
    assert!(emitted_ops(3, 1, vec![card(10, 3039, 999)]).is_empty());
}

#[test]
fn ignores_the_other_teams_queue() {
    let behavior = ParsedBehavior::new(60265, "ConsumePowerUpdateActorCard", vec![1]);
    let managers = crate::engine::manager::BattleManagers::default();
    let pool = TargetPool::default();
    let event = BattleEvent::ActionQueueCommitted {
        team: 2,
        emitter_uid: -1,
        cards: Vec::new(),
    };
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();

    assert_eq!(
        Handler::emit_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 303901441,
                transfer_count: 1,
                event: Some(&event),
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        ),
        Some(Vec::new())
    );
}

#[test]
fn keeps_queue_mutations_in_the_action_queue_lifecycle() {
    let definition =
        crate::engine::skill::behavior::registry::find_key(60265, "ConsumePowerUpdateActorCard")
            .unwrap();

    assert_eq!(
        definition.output_owner,
        crate::engine::skill::behavior::registry::OutputOwner::Parent
    );
}

#[test]
fn accepts_only_configured_costs() {
    for (args, expected) in [(vec![1], true), (vec![2], true), (vec![3], false)] {
        assert_eq!(
            supports_arguments(&ParsedBehavior::new(
                60265,
                "ConsumePowerUpdateActorCard",
                args,
            )),
            expected
        );
    }
}
