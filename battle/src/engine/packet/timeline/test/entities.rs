use super::*;

#[test]
fn summon_projection_uses_the_committed_operation() {
    let mut summons = SummonManager::default();
    let changes = summons
        .execute_command(SummonCommand {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(40009, "AddSummoned"),
            },
            owner_uid: 10,
            summoned_id: 60008,
            operation: SummonOperation::Add {
                target_uid: -1,
                count: 1,
                level: 2,
            },
        })
        .unwrap();

    let effects = project_change_for_test(&BattleChange::Summon(changes)).unwrap();

    let summoned = effects[0].summoned.as_ref().unwrap();
    assert_eq!(effects[0].target_id, Some(-1));
    assert_eq!(summoned.summoned_id, Some(60008));
    assert_eq!(summoned.level, Some(2));
    assert_eq!(summoned.from_uid, Some(10));
}

#[test]
fn special_entity_projection_keeps_source_target_and_spawned_identity_distinct() {
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60056, "SummonSp2"),
    };
    let changes = EntityChanges {
        origin,
        source_uid: -2,
        target_uid: -2,
        entity: FightEntityInfo {
            uid: Some(-8),
            model_id: Some(151416),
            position: Some(5),
            ..Default::default()
        },
        operation: EntityOperation::SummonSpecial { model_id: 151416 },
    };

    let effects = project_change_for_test(&BattleChange::Entity(Box::new(changes))).unwrap();

    assert_eq!(effects[0].target_id, Some(-2));
    assert_eq!(effects[0].effect_type, Some(EffectType::Summon as i32));
    assert_eq!(effects[0].effect_num, Some(151416));
    assert_eq!(effects[0].config_effect, Some(60056));
    assert_eq!(effects[0].entity.as_ref().unwrap().uid, Some(-8));
}

#[test]
fn transformed_entity_projects_the_committed_replacement() {
    let changes = EntityChanges {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(40006, "MonsterChange"),
        },
        source_uid: -7,
        target_uid: -7,
        entity: FightEntityInfo {
            uid: Some(-7),
            model_id: Some(251407),
            position: Some(1),
            ..Default::default()
        },
        operation: EntityOperation::Transform {
            model_id: 251407,
            parameters: [1000, 1],
        },
    };

    let effects = project_change_for_test(&BattleChange::Entity(Box::new(changes))).unwrap();

    assert_eq!(effects[0].target_id, Some(-7));
    assert_eq!(
        effects[0].effect_type,
        Some(EffectType::Monsterchange as i32)
    );
    assert_eq!(effects[0].effect_num, Some(251407));
    assert_eq!(effects[0].config_effect, Some(40006));
}

#[test]
fn upgrade_offer_projects_without_loading_the_selected_entity() {
    let mut upgrades = UpgradeManager::default();
    let change = upgrades
        .execute_command(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            UpgradeCommand {
                owner_uid: 10,
                operation: UpgradeOperation::Offer {
                    origin: CommandOrigin {
                        domain: RuleDomain::Behavior,
                        key: DefinitionKey::new(60037, "NotifyUpgradeHero"),
                    },
                    upgrade_id: 308665,
                },
            },
        )
        .unwrap();

    let effects = project_change_for_test(&BattleChange::Upgrade(change)).unwrap();

    assert_eq!(effects[0].target_id, Some(10));
    assert_eq!(effects[0].effect_num, Some(308665));
    assert_eq!(effects[0].config_effect, Some(60037));
}
