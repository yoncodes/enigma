use super::*;

#[test]
fn applied_upgrade_projects_from_one_semantic_change() {
    let mut cards = CardManager::new(vec![CardInfo {
        uid: Some(30),
        skill_id: Some(100),
        ..Default::default()
    }]);
    let card_changes = cards
        .execute_command(CardCommand::ReplaceOwnerSkills(
            crate::engine::manager::card::CardReplaceOwnerSkills {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(60037, "NotifyUpgradeHero"),
                },
                owner_uid: 30,
                base_group1: vec![100],
                base_group2: Vec::new(),
                replacement_group1: vec![200],
                replacement_group2: Vec::new(),
            },
        ))
        .unwrap();
    let effects =
        project_change_for_test(&BattleChange::UpgradeApplied(Box::new(UpgradeApplied {
            change: crate::engine::manager::upgrade::UpgradeChange {
                operation: UpgradeOperation::Select {
                    upgrade_id: 15,
                    option_id: 20,
                },
                offer_origin: None,
                owner_uid: 10,
                offered_before: Some(15),
                offered_after: None,
                selected_before: None,
                selected_after: Some(20),
                selection: None,
            },
            entity: FightEntityInfo {
                uid: Some(10),
                ..Default::default()
            },
            buff_changes: Vec::new(),
            card_changes,
        })))
        .unwrap();

    assert_eq!(effects.len(), 2);
    assert_eq!(effects[0].effect_type, Some(EffectType::Heroupgrade as i32));
    assert_eq!(effects[1].effect_type, Some(EffectType::Cardspush as i32));
}
