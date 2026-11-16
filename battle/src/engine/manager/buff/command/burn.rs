use super::{GrantAction, GrantPlan};
use crate::engine::skill::buff_act::registry::BuffActKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BurnApplication {
    pub source_uid: i64,
    pub buff_id: i32,
    pub layers: i32,
}

impl GrantPlan {
    pub(super) fn burn_application(&self) -> Option<BurnApplication> {
        if self.transition.is_some()
            || !matches!(self.action, GrantAction::Add | GrantAction::RefreshLayer)
            || !self.definition.uses_stack_layer()
            || !self.definition.features().iter().any(|feature| {
                feature.arguments_supported && feature.kind == Some(BuffActKind::Burn)
            })
        {
            return None;
        }
        let layers =
            self.definition
                .raw_layer(self.args.layer, self.args.layer_specified, self.args.count);
        (layers > 0).then_some(BurnApplication {
            source_uid: self.route.source_uid,
            buff_id: self.route.buff_id,
            layers,
        })
    }
}
