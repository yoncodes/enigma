use crate::engine::fight::reserve::{CycleCommand, Promotion};

pub(super) fn supports(args: &[i32]) -> bool {
    args.is_empty()
}

pub(super) fn plan(promotion: &Promotion) -> CycleCommand {
    CycleCommand {
        defeated_uid: promotion.defeated_uid,
        team_type: promotion.team_type,
    }
}
