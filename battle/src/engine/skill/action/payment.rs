use crate::engine::manager::ex_point::{ExPointChange, ExPointCommand};

#[derive(Debug, Clone, PartialEq)]
pub struct ActionPayment {
    pub cost: ExPointCommand,
    pub composition: Vec<ExPointChange>,
}

impl ActionPayment {
    pub fn spent(&self) -> i32 {
        match &self.cost {
            ExPointCommand::Spend(change) => change.delta.saturating_neg().max(0),
            _ => 0,
        }
    }
}
