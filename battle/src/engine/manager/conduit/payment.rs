use super::{ConduitError, ConduitSkill};

const FLEXIBLE_POWER: i32 = 0;
const UNIQUE_COST: i32 = 999;

#[derive(Clone, Copy)]
pub(crate) struct Payment {
    power_id: i32,
    cost: i32,
}

impl Payment {
    pub(crate) fn new(skill: ConduitSkill, cost: i32) -> Self {
        Self {
            power_id: skill.cost_type,
            cost,
        }
    }

    pub(crate) fn uses_energy(self) -> bool {
        self.power_id != UNIQUE_COST && self.cost > 0
    }

    pub(super) fn is_unique(self) -> bool {
        self.power_id == UNIQUE_COST
    }

    pub(crate) fn split(self, power: impl Fn(i32) -> i32) -> Result<[(i32, i32); 2], ConduitError> {
        if self.is_unique() {
            return Ok([(self.power_id, 0), (FLEXIBLE_POWER, 0)]);
        }
        let specific = power(self.power_id);
        let flexible = if self.power_id > 0 {
            power(FLEXIBLE_POWER)
        } else {
            0
        };
        let available = specific.saturating_add(flexible);
        if available < self.cost {
            return Err(ConduitError::InsufficientPower {
                power_id: self.power_id,
                available,
                required: self.cost,
            });
        }
        let paid = self.cost.min(specific);
        Ok([(self.power_id, paid), (FLEXIBLE_POWER, self.cost - paid)])
    }
}
