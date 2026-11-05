use crate::engine::manager::ex_point::{ExPointChange, ExPointCommand};

#[derive(Debug, Clone, PartialEq)]
pub struct ActionPayment {
    pub cost: ExPointCommand,
    pub composition: Vec<ExPointChange>,
}
