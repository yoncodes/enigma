use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

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
fn sequential_slots_consume_the_committed_amount_for_each_precast_rank() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                team_type: Some(1),
                current_hp: Some(100),
                skill_group1: vec![101, 102, 103],
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(30650201),
                    layer: Some(3),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(100018, "ConsumeBuffCreateTempCardOrder"),
        vec![30650201, 1],
        vec!["30650201".into(), "2,1".into(), "2,1".into(), "1".into()],
    );
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();

    let mut emit = || {
        Handler::emit_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 30650201,
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
        .unwrap()
        .pop()
        .unwrap()
    };

    let RuleOp::Command(BattleCommand::BuffPrecast(command)) = emit() else {
        panic!("buff-backed precast must emit its owned mechanic command")
    };
    let changes = crate::engine::mechanic::buff_precast::execute(&mut managers, command.clone())
        .unwrap()
        .unwrap();
    assert_eq!(
        changes.card.kind,
        crate::engine::manager::card::CardChangeKind::GeneratedAdded
    );
    let card = changes.card.added.as_ref().unwrap();
    assert_eq!(card.uid, Some(10));
    assert_eq!(card.hero_id, Some(3149));
    assert_eq!(card.temp_card, Some(true));
    assert_eq!(
        card.card_type,
        Some(sonettobuf::card_info::CardType::None as i32)
    );
    assert!(matches!(
        changes.card.operation,
        Some(crate::engine::manager::card::CardChange::AddHand { target_uid: 10, .. })
    ));
    crate::engine::mechanic::buff_precast::execute(&mut managers, command)
        .unwrap()
        .unwrap();

    assert_eq!(managers.buff.buff_id_amount(10, 30650201), 0);
    assert_eq!(
        managers
            .card
            .hand()
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![102, 101]
    );
}
