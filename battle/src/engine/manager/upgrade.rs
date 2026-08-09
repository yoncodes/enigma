use std::collections::HashMap;

use crate::engine::skill::rule::CommandOrigin;
use sonettobuf::FightEntityInfo;

use super::{buff::BuffChanges, card::CardChanges};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeSelection {
    pub upgrade_id: i32,
    pub option_id: i32,
    pub add_buff_ids: Vec<i32>,
    pub del_buff_ids: Vec<i32>,
    pub replace_skill_group1: Vec<i32>,
    pub replace_skill_group2: Vec<i32>,
    pub replace_big_skill: i32,
    pub replace_passive_skills: Vec<(i32, i32)>,
    pub add_passive_skill_ids: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpgradeApplied {
    pub change: UpgradeChange,
    pub entity: FightEntityInfo,
    pub buff_changes: Vec<BuffChanges>,
    pub card_changes: CardChanges,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeOperation {
    Offer {
        origin: CommandOrigin,
        upgrade_id: i32,
    },
    Select {
        upgrade_id: i32,
        option_id: i32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpgradeCommand {
    pub owner_uid: i64,
    pub operation: UpgradeOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeChange {
    pub operation: UpgradeOperation,
    pub offer_origin: Option<CommandOrigin>,
    pub owner_uid: i64,
    pub offered_before: Option<i32>,
    pub offered_after: Option<i32>,
    pub selected_before: Option<i32>,
    pub selected_after: Option<i32>,
    pub selection: Option<UpgradeSelection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeCommandError {
    InvalidCommand,
    SelectionRejected,
}

#[derive(Debug, Clone, Default)]
pub struct UpgradeManager {
    offered: HashMap<i64, i32>,
    offer_origins: HashMap<i64, CommandOrigin>,
    selected: HashMap<i64, i32>,
}

impl UpgradeManager {
    pub(crate) fn execute_command(
        &mut self,
        catalog: crate::catalog::BattleCatalog,
        command: UpgradeCommand,
    ) -> Result<UpgradeChange, UpgradeCommandError> {
        if command.owner_uid == 0 {
            return Err(UpgradeCommandError::InvalidCommand);
        }
        let offered_before = self.offered.get(&command.owner_uid).copied();
        let offer_origin_before = self.offer_origins.get(&command.owner_uid).copied();
        let selected_before = self.selected.get(&command.owner_uid).copied();
        let selection = match command.operation {
            UpgradeOperation::Offer { origin, upgrade_id } if upgrade_id > 0 => {
                if offered_before != Some(upgrade_id) {
                    self.offer(command.owner_uid, upgrade_id);
                    self.offer_origins.insert(command.owner_uid, origin);
                }
                None
            }
            UpgradeOperation::Select {
                upgrade_id,
                option_id,
            } if upgrade_id > 0 && option_id > 0 => Some(
                self.select(catalog, command.owner_uid, upgrade_id, option_id)
                    .ok_or(UpgradeCommandError::SelectionRejected)?,
            ),
            _ => return Err(UpgradeCommandError::InvalidCommand),
        };
        Ok(UpgradeChange {
            operation: command.operation,
            offer_origin: self
                .offer_origins
                .get(&command.owner_uid)
                .copied()
                .or(offer_origin_before),
            owner_uid: command.owner_uid,
            offered_before,
            offered_after: self.offered.get(&command.owner_uid).copied(),
            selected_before,
            selected_after: self.selected.get(&command.owner_uid).copied(),
            selection,
        })
    }

    pub(crate) fn offer(&mut self, owner_uid: i64, upgrade_id: i32) {
        self.offered.insert(owner_uid, upgrade_id);
        self.offer_origins.remove(&owner_uid);
    }

    pub(crate) fn select(
        &mut self,
        catalog: crate::catalog::BattleCatalog,
        owner_uid: i64,
        upgrade_id: i32,
        option_id: i32,
    ) -> Option<UpgradeSelection> {
        if self.offered.get(&owner_uid).copied() != Some(upgrade_id) {
            return None;
        }
        let selection = catalog.upgrade_selection(upgrade_id, option_id)?;
        self.record_selection(owner_uid, selection)
    }

    pub fn selected_option(&self, owner_uid: i64) -> Option<i32> {
        self.selected.get(&owner_uid).copied()
    }

    fn record_selection(
        &mut self,
        owner_uid: i64,
        selection: UpgradeSelection,
    ) -> Option<UpgradeSelection> {
        if self.selected.get(&owner_uid) == Some(&selection.option_id) {
            return None;
        }
        self.offered.remove(&owner_uid);
        self.offer_origins.remove(&owner_uid);
        self.selected.insert(owner_uid, selection.option_id);
        Some(selection)
    }
}

#[cfg(test)]
mod tests {
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
}
