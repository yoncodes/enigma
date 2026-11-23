use super::*;
use crate::engine::skill::rule::{CommandOrigin, DefinitionKey, RuleDomain};

const ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::Behavior,
    key: DefinitionKey::new(40009, "AddSummoned"),
};

#[test]
fn commands_commit_add_level_and_remove_through_one_owner() {
    let mut summons = SummonManager::default();
    let added = summons
        .execute_command(SummonCommand {
            origin: ORIGIN,
            owner_uid: 10,
            summoned_id: 150011,
            operation: SummonOperation::Add {
                target_uid: 10,
                count: 1,
                level: 1,
            },
        })
        .unwrap();
    assert_eq!(added.after.unwrap().level, 1);
    assert_eq!(added.events().len(), 1);

    summons
        .execute_command(SummonCommand {
            origin: ORIGIN,
            owner_uid: 10,
            summoned_id: 150011,
            operation: SummonOperation::AddLevel { delta: 1 },
        })
        .unwrap();
    assert_eq!(summons.get(10, 150011).unwrap().level, 2);

    summons
        .execute_command(SummonCommand {
            origin: ORIGIN,
            owner_uid: 10,
            summoned_id: 150011,
            operation: SummonOperation::Remove { count: 0 },
        })
        .unwrap();
    assert!(summons.get(10, 150011).is_none());
}

#[test]
fn summon_keeps_level_until_removed() {
    let mut summons = SummonManager::default();
    assert_eq!(summons.add(10, 10, 150011, 1, 1)[0].uid, 2);
    assert_eq!(summons.count(10, 150011, 1), 1);
    assert_eq!(summons.add_level(10, 150011, 1).unwrap().level, 2);
    assert_eq!(summons.change_level(10, 150011, 4).unwrap().level, 4);
    assert!(summons.remove(10, 150011, 0));
    assert!(summons.add_level(10, 150011, 1).is_none());
}

#[test]
fn adding_an_existing_summon_updates_its_slot_without_stacking() {
    let mut summons = SummonManager::default();
    summons.add(10, 10, 15003, 1, 1);

    assert!(summons.add(10, 10, 15003, 1, 2).is_empty());
    assert_eq!(summons.count(10, 15003, 0), 1);
    assert_eq!(summons.count(10, 15003, 2), 1);
    assert_eq!(summons.total(10, 0), 1);
}

#[test]
fn active_unique_skills_use_the_selected_catalog_and_summon_order() {
    crate::test_support::init_config();
    let game_data = crate::test_support::game_data();
    let catalog = crate::catalog::BattleCatalog::new(game_data);
    let mut summons = SummonManager::default();
    summons.add(20, 20, 15_001, 1, 1);
    summons.add(10, 10, 15_001, 1, 1);
    summons.add(10, 10, 150_011, 1, 1);
    let expected = vec![(10, 307_401_711), (10, 30_740_171), (20, 30_740_171)];

    assert_eq!(summons.active_unique_skills(game_data), expected);
    assert_eq!(
        summons.skill_owners(|summoned_id| catalog.summoned_unique_skills(summoned_id)),
        expected
    );
}
