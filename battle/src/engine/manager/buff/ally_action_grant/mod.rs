use super::{BuffDefinition, BuffStatus};

pub(super) fn uses_configured_uid(
    shared_uid_lane: bool,
    source_uid: i64,
    target_uid: i64,
    definition: &BuffDefinition,
) -> bool {
    shared_uid_lane
        && source_uid == target_uid
        && definition.status == BuffStatus::Special
        && definition.is_layer_type()
}
