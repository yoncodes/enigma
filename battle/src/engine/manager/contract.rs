use crate::engine::skill::rule::CommandOrigin;

const OWNER_BUFF_MAP: i32 = 30;
const BOUND_BUFF_MAP: i32 = 31;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractCommand {
    Offer {
        origin: CommandOrigin,
        owner_uid: i64,
        candidates: Vec<i64>,
    },
    SelectOwner {
        owner_uid: i64,
        bound_uid: i64,
    },
    SelectBound {
        owner_uid: i64,
        bound_uid: i64,
    },
    Clear {
        owner_uid: i64,
        bound_uid: i64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractChange {
    Offered {
        origin: CommandOrigin,
        owner_uid: i64,
        candidates: Vec<i64>,
    },
    OwnerSelected {
        origin: CommandOrigin,
        owner_uid: i64,
    },
    BoundSelected {
        owner_uid: i64,
        bound_uid: i64,
    },
    Cleared {
        owner_uid: i64,
        bound_uid: i64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractError {
    EmptyOffer,
    InvalidSelection,
}

#[derive(Debug, Clone, Default)]
pub struct ContractManager {
    pending: Option<(CommandOrigin, i64, Vec<i64>)>,
    owner_uid: Option<i64>,
    bound_uid: Option<i64>,
}

impl ContractManager {
    pub fn execute(&mut self, command: ContractCommand) -> Result<ContractChange, ContractError> {
        match command {
            ContractCommand::Offer {
                origin,
                owner_uid,
                candidates,
            } => {
                if candidates.is_empty() {
                    return Err(ContractError::EmptyOffer);
                }
                self.pending = Some((origin, owner_uid, candidates.clone()));
                Ok(ContractChange::Offered {
                    origin,
                    owner_uid,
                    candidates,
                })
            }
            ContractCommand::SelectOwner {
                owner_uid,
                bound_uid,
            } => {
                let origin = self.validate_selection(owner_uid, bound_uid)?;
                self.owner_uid = Some(owner_uid);
                Ok(ContractChange::OwnerSelected { origin, owner_uid })
            }
            ContractCommand::SelectBound {
                owner_uid,
                bound_uid,
            } => {
                self.validate_selection(owner_uid, bound_uid)?;
                if self.owner_uid != Some(owner_uid) {
                    return Err(ContractError::InvalidSelection);
                }
                self.bound_uid = Some(bound_uid);
                self.pending = None;
                Ok(ContractChange::BoundSelected {
                    owner_uid,
                    bound_uid,
                })
            }
            ContractCommand::Clear {
                owner_uid,
                bound_uid,
            } => {
                if self.owner_uid != Some(owner_uid) || self.bound_uid != Some(bound_uid) {
                    return Err(ContractError::InvalidSelection);
                }
                self.owner_uid = None;
                self.bound_uid = None;
                Ok(ContractChange::Cleared {
                    owner_uid,
                    bound_uid,
                })
            }
        }
    }

    pub fn bound_uid(&self, owner_uid: i64) -> Option<i64> {
        (self.owner_uid == Some(owner_uid)).then_some(self.bound_uid?)
    }

    pub fn selection_origin(&self, owner_uid: i64, bound_uid: i64) -> Option<CommandOrigin> {
        self.validate_selection(owner_uid, bound_uid).ok()
    }

    fn validate_selection(
        &self,
        owner_uid: i64,
        bound_uid: i64,
    ) -> Result<CommandOrigin, ContractError> {
        let Some((origin, pending_owner, candidates)) = &self.pending else {
            return Err(ContractError::InvalidSelection);
        };
        (*pending_owner == owner_uid && candidates.contains(&bound_uid))
            .then_some(*origin)
            .ok_or(ContractError::InvalidSelection)
    }
}

pub fn binding_buffs(ex_skill_level: i32, career: i32) -> Option<(i32, i32)> {
    Some((
        mapped_buff(OWNER_BUFF_MAP, ex_skill_level, career)?,
        mapped_buff(BOUND_BUFF_MAP, ex_skill_level, career)?,
    ))
}

fn mapped_buff(config_id: i32, ex_skill_level: i32, career: i32) -> Option<i32> {
    let value = &config::configs::get().fight_const.get(config_id)?.value;
    let levels = value
        .split('|')
        .find_map(|entry| {
            entry
                .split_once('%')
                .filter(|(key, _)| key.parse() == Ok(career))
        })?
        .1;
    levels
        .split(',')
        .find_map(|entry| {
            entry
                .split_once(':')
                .filter(|(key, _)| key.parse() == Ok(ex_skill_level))
        })
        .or_else(|| {
            levels
                .split(',')
                .find_map(|entry| entry.split_once(':').filter(|(key, _)| *key == "0"))
        })?
        .1
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
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
        assert_eq!(binding_buffs(0, 1), Some((31000221, 31000191)));
        assert_eq!(binding_buffs(4, 1), Some((31000222, 31000192)));
    }
}
