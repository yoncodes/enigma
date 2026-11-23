use sonettobuf::CardInfo;

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

#[test]
fn generated_crystal_is_an_owner_bound_precast_card() {
    let card = generated_card(&crate::engine::manager::BattleManagers::default(), 10, 101);

    assert_eq!(card.uid, Some(10));
    assert_eq!(card.temp_card, Some(true));
    assert_eq!(
        card.card_type,
        Some(sonettobuf::card_info::CardType::Skill3 as i32)
    );
}

#[test]
fn crystal_card_adds_the_captured_owner_bound_card() {
    let managers = BattleManagers::default();
    let pool = TargetPool::default();
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_crystal_cards([CardInfo {
        uid: Some(999),
        skill_id: Some(102),
        temp_card: Some(false),
        card_type: Some(123),
        ..Default::default()
    }]);
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60245, "CrystalAddCard"),
        Vec::new(),
        vec![
            "400,-100".into(),
            "500,0".into(),
            "100,100".into(),
            "101,201".into(),
            "102,202".into(),
            "103,203".into(),
        ],
    );

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 1,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    assert_eq!(ops.len(), 1);
    assert!(matches!(
        &ops[0],
        RuleOp::Command(BattleCommand::Card(CardCommand::AddCrystal(add)))
            if add.rank_group == vec![101, 102, 103]
                && add.card.uid == Some(10)
                && add.card.skill_id == Some(102)
                && add.card.temp_card == Some(true)
                && add.card.card_type
                    == Some(sonettobuf::card_info::CardType::Skill3 as i32)
    ));
}
