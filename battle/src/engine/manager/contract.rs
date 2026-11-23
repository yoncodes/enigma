use crate::engine::skill::rule::CommandOrigin;

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

pub fn binding_buffs(
    game_data: &config::GameDB,
    ex_skill_level: i32,
    career: i32,
) -> Option<(i32, i32)> {
    crate::catalog::contract_binding_buffs(game_data, ex_skill_level, career)
}

#[cfg(test)]
mod tests;
