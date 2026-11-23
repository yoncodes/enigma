use super::*;
use crate::engine::skill::rule::{DefinitionKey, RuleDomain};

fn origin() -> CommandOrigin {
    CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60092, "NotifyHeroContract"),
    }
}

#[test]
fn selection_requires_the_offered_owner_and_candidate() {
    let mut manager = ContractManager::default();
    manager
        .execute(ContractCommand::Offer {
            origin: origin(),
            owner_uid: -1,
            candidates: vec![10, 20],
        })
        .unwrap();
    assert_eq!(
        manager.execute(ContractCommand::SelectOwner {
            owner_uid: -1,
            bound_uid: 30,
        }),
        Err(ContractError::InvalidSelection)
    );
    manager
        .execute(ContractCommand::SelectOwner {
            owner_uid: -1,
            bound_uid: 20,
        })
        .unwrap();
    manager
        .execute(ContractCommand::SelectBound {
            owner_uid: -1,
            bound_uid: 20,
        })
        .unwrap();
    assert_eq!(manager.bound_uid(-1), Some(20));
}

#[test]
fn clear_requires_and_removes_the_selected_pair() {
    let mut manager = ContractManager::default();
    manager
        .execute(ContractCommand::Offer {
            origin: origin(),
            owner_uid: -1,
            candidates: vec![20],
        })
        .unwrap();
    manager
        .execute(ContractCommand::SelectOwner {
            owner_uid: -1,
            bound_uid: 20,
        })
        .unwrap();
    manager
        .execute(ContractCommand::SelectBound {
            owner_uid: -1,
            bound_uid: 20,
        })
        .unwrap();

    assert_eq!(
        manager.execute(ContractCommand::Clear {
            owner_uid: -1,
            bound_uid: 30,
        }),
        Err(ContractError::InvalidSelection)
    );
    assert_eq!(
        manager
            .execute(ContractCommand::Clear {
                owner_uid: -1,
                bound_uid: 20,
            })
            .unwrap(),
        ContractChange::Cleared {
            owner_uid: -1,
            bound_uid: 20,
        }
    );
    assert_eq!(manager.bound_uid(-1), None);
}

#[test]
fn fight_const_maps_the_captured_career_and_ultimate_level() {
    crate::test_support::init_config();
    assert_eq!(
        binding_buffs(crate::test_support::game_data(), 0, 1),
        Some((31000221, 31000191))
    );
    assert_eq!(
        binding_buffs(crate::test_support::game_data(), 4, 1),
        Some((31000222, 31000192))
    );
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    assert_eq!(
        catalog.contract_binding_buffs(0, 1),
        Some((31000221, 31000191))
    );
    assert_eq!(
        catalog.contract_binding_buffs(4, 1),
        Some((31000222, 31000192))
    );
}
