pub(crate) mod card_selection;
pub mod pool;
pub mod request;
pub mod resolve;

pub(crate) use card_selection::CardSelectionResolver;
pub use pool::{EntityDamageType, TargetContext, TargetEntity, TargetPool};
pub use request::TargetRequest;
pub(crate) use resolve::frame_anchor_for_rule;
pub use resolve::{TargetResolver, is_mapped_target_code, targets_enemy};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BehaviorTargetOrder {
    #[default]
    Resolved,
    EntityIdentityMap,
}
