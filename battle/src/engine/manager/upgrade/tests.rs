use super::*;
use crate::engine::skill::rule::{DefinitionKey, RuleDomain};

const ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::Behavior,
    key: DefinitionKey::new(60037, "NotifyUpgradeHero"),
};

#[test]
fn explicit_selection_must_be_offered_and_is_consumed_once() {
    crate::test_support::init_config();
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let mut upgrades = UpgradeManager::default();
    upgrades.offer(10, 308664);

    assert_eq!(
        upgrades
            .select(catalog, 10, 308664, 3086524)
            .unwrap()
            .option_id,
        3086524
    );
    assert!(upgrades.select(catalog, 10, 308664, 3086525).is_none());
}

#[test]
fn commands_keep_offer_and_selection_in_one_owner() {
    crate::test_support::init_config();
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let mut upgrades = UpgradeManager::default();

    let offered = upgrades
        .execute_command(
            catalog,
            UpgradeCommand {
                owner_uid: 10,
                operation: UpgradeOperation::Offer {
                    origin: ORIGIN,
                    upgrade_id: 308664,
                },
            },
        )
        .unwrap();
    assert_eq!(offered.offered_after, Some(308664));
    assert_eq!(offered.offer_origin, Some(ORIGIN));

    let selected = upgrades
        .execute_command(
            catalog,
            UpgradeCommand {
                owner_uid: 10,
                operation: UpgradeOperation::Select {
                    upgrade_id: 308664,
                    option_id: 3086524,
                },
            },
        )
        .unwrap();
    assert_eq!(selected.offered_after, None);
    assert_eq!(selected.selected_after, Some(3086524));
    assert_eq!(selected.selection.unwrap().option_id, 3086524);
}

#[test]
fn repeated_offer_preserves_the_emitted_operation() {
    crate::test_support::init_config();
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let mut upgrades = UpgradeManager::default();
    let command = UpgradeCommand {
        owner_uid: 10,
        operation: UpgradeOperation::Offer {
            origin: ORIGIN,
            upgrade_id: 308664,
        },
    };

    upgrades.execute_command(catalog, command).unwrap();
    let repeated = upgrades.execute_command(catalog, command).unwrap();

    assert_eq!(repeated.operation, command.operation);
    assert_eq!(repeated.offered_before, repeated.offered_after);
}
