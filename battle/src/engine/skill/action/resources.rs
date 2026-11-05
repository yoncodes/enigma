use crate::engine::manager::ex_point::{ExPointChange, ExPointCommand};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ActionResources {
    pub cost: Option<ExPointCommand>,
    pub composition: Vec<ExPointChange>,
}
