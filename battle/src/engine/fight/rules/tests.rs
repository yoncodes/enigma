use super::*;

#[test]
fn explicit_catalog_preserves_rule_side_and_order() {
    crate::test_support::init_config();
    let fight = Fight {
        battle_id: Some(9_000_303),
        ..Default::default()
    };
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let rules = catalog.battle_rules(&fight);

    assert_eq!(configured(&fight), rules);
    assert_eq!(
        rules
            .iter()
            .map(|rule| (rule.side, rule.rule_id, rule.skill_id, rule.rule_type))
            .collect::<Vec<_>>(),
        vec![
            (
                BattleRuleSide::Defender,
                370_003_003,
                370_003_003,
                AdditionRuleType::Skill
            ),
            (
                BattleRuleSide::Attacker,
                22_301_961,
                22_301_961,
                AdditionRuleType::Skill
            ),
            (
                BattleRuleSide::Attacker,
                90_120_002,
                90_120_002,
                AdditionRuleType::Skill
            ),
            (
                BattleRuleSide::Defender,
                22_301_962,
                22_301_962,
                AdditionRuleType::Skill
            ),
            (
                BattleRuleSide::Defender,
                370_003_013,
                370_003_013,
                AdditionRuleType::Skill
            ),
        ]
    );
}
