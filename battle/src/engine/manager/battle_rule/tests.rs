use super::*;

#[test]
fn extending_rules_preserves_configured_order_and_deduplicates() {
    let mut manager = BattleRuleManager::default();
    manager.extend_owned_skills([
        OwnedBattleSkill {
            owner_uid: 0,
            skill_id: 20,
        },
        OwnedBattleSkill {
            owner_uid: 0,
            skill_id: 10,
        },
        OwnedBattleSkill {
            owner_uid: 0,
            skill_id: 20,
        },
    ]);

    assert_eq!(
        manager.owned_skills().collect::<Vec<_>>(),
        vec![(0, 20), (0, 10)]
    );
}

#[test]
fn max_round_increase_uses_the_battle_base_and_configured_cap() {
    crate::test_support::init_config();
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let fight = Fight {
        battle_id: Some(1_211),
        ..Default::default()
    };
    let mut manager = BattleRuleManager::seed_with_catalog(catalog, &fight);

    let marker = manager
        .add_max_round(MaxRoundCommand {
            increment: 1,
            cap: 30,
        })
        .unwrap();
    assert_eq!(manager.max_round(), Some(21));
    assert_eq!(marker.effect_num, 1);
    assert_eq!(marker.config_effect, 0);

    manager.add_max_round(MaxRoundCommand {
        increment: 20,
        cap: 30,
    });
    assert_eq!(manager.max_round(), Some(30));
}

#[test]
fn player_reserves_are_immediate_and_monsters_require_the_rule() {
    let mut manager = BattleRuleManager::default();

    assert!(manager.promotes_reserves_immediately(1));
    assert!(!manager.promotes_reserves_immediately(2));

    manager.enable_immediate_reserve_promotion(ImmediateReservePromotionCommand { team_type: 2 });
    assert!(manager.promotes_reserves_immediately(2));
}
