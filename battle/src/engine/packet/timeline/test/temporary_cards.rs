use super::*;

#[test]
fn temporary_card_projection_uses_the_committed_operation() {
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60175, "DirectUseBigSkill"),
    };
    let mut cards = CardManager::default();
    let changes = cards
        .execute_command(CardCommand::AddTemporary(CardAddTemporary {
            origin,
            target_uid: 10,
            skill_id: 999,
            reserve_id: 12,
            team_type: 1,
            kind: crate::engine::manager::card::TemporaryCardKind::ConfiguredSkill,
        }))
        .unwrap();

    let effects = project_change_for_test(&BattleChange::Card(Box::new(changes))).unwrap();

    assert_eq!(effects.len(), 2);
    assert_eq!(effects[0].target_id, Some(10));
    assert_eq!(effects[0].effect_num, Some(999));
    assert_eq!(effects[0].reserve_id, Some(12));
    assert_eq!(effects[0].team_type, Some(1));
    assert_eq!(
        effects[1].effect_type,
        Some(EffectType::Changetotempcard as i32)
    );
}

#[test]
fn card_setup_is_state_only() {
    let mut cards = CardManager::default();
    let changes = cards
        .execute_command(CardCommand::Setup(
            crate::engine::manager::card::CardSetup {
                hand: Vec::new(),
                draw_pile: Vec::new(),
                deck_num: 16,
            },
        ))
        .unwrap();

    assert!(
        project_change_for_test(&BattleChange::Card(Box::new(changes)))
            .unwrap()
            .is_empty()
    );
}
