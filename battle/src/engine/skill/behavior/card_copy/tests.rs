use sonettobuf::{CardInfo, Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::{
    manager::BattleManagers,
    runtime::determinism::RoundDeterminism,
    skill::{
        action::SkillModifiers,
        behavior::classify::BehaviorSpec,
        target::{TargetContext, TargetPool},
    },
};

const OWNER: i64 = 10;
const BUFF_CARD: i32 = 31_460_223;
const ATTACK_CARD: i32 = 31_460_213;
const ULTIMATE_BUFF_CARD: i32 = 31_460_131;
const ENERGY_CARD: i32 = 31_446_011;

fn behavior(first: &str, count: &str) -> ParsedBehavior {
    ParsedBehavior::from_spec(
        BehaviorSpec::new(100024, "UseCardCopy"),
        vec![1, 2, 4, 1],
        vec![first.into(), "4".into(), count.into()],
    )
}

fn card(uid: i64, skill_id: i32) -> CardInfo {
    CardInfo {
        uid: Some(uid),
        skill_id: Some(skill_id),
        ..Default::default()
    }
}

fn copy_ops(cards: Vec<CardInfo>) -> Vec<RuleOp> {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(OWNER),
                model_id: Some(3146),
                team_type: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();
    let event = BattleEvent::ActionQueueCommitted {
        team: 1,
        emitter_uid: 0,
        action_count: cards.len(),
        cards,
    };
    Handler::emit_ops(
        BehaviorOpContext {
            source_uid: OWNER,
            source_team: 1,
            target_uid: OWNER,
            active_skill_id: 0,
            transfer_count: 1,
            event: Some(&event),
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior("1,2", "1"),
    )
    .expect("a supported copy rule handles every queue commit")
}

#[test]
fn one_matching_card_is_queued_again_after_the_committed_actions() {
    let ops = copy_ops(vec![
        card(OWNER, ATTACK_CARD),
        card(OWNER, BUFF_CARD),
        card(OWNER, BUFF_CARD),
    ]);

    let [RuleOp::Command(BattleCommand::Card(CardCommand::QueueUseCard(queue)))] = ops.as_slice()
    else {
        panic!("expected one queued copy, got {ops:?}")
    };
    assert_eq!(queue.card.skill_id, Some(BUFF_CARD));
    assert_eq!(queue.card.uid, Some(OWNER));
    assert_eq!(queue.card_index, 4);
    assert_eq!(queue.source_skill_id, BUFF_CARD);
    assert_eq!(queue.reserve_id, 100024);
}

#[test]
fn ultimates_other_tags_and_other_owners_cards_are_not_copied() {
    let ops = copy_ops(vec![
        card(OWNER, ULTIMATE_BUFF_CARD),
        card(OWNER, ATTACK_CARD),
        card(OWNER, ENERGY_CARD),
        card(OWNER + 1, BUFF_CARD),
    ]);

    assert!(ops.is_empty());
}

#[test]
fn only_described_argument_shapes_are_supported() {
    assert!(supports_arguments(&behavior("1,2", "1")));
    assert!(supports_arguments(&behavior("1,2", "2")));
    assert!(!supports_arguments(&behavior("3", "1")));
    assert!(!supports_arguments(&behavior("1,2", "9")));
}
