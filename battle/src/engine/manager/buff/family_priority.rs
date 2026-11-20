use super::{BuffDefinition, BuffManager, BuffPolicy, BuffRoute, BuffStatus, ExistingBuffMatch};
use crate::engine::{entity::attr::AttrId, skill::buff_act::registry::BuffActKind};

pub(super) fn retains_stronger(
    manager: &BuffManager,
    route: BuffRoute,
    incoming: &BuffDefinition,
    policy: &BuffPolicy,
) -> bool {
    if policy.match_existing != ExistingBuffMatch::SharedTypeFamily {
        return false;
    }
    let Some((attribute, value)) = static_attribute(incoming) else {
        return false;
    };
    manager.buffs.iter().any(|active| {
        active.effective
            && policy.matches(active, route)
            && active.definition.as_ref().is_some_and(|resident| {
                static_attribute(resident).is_some_and(|(held_attribute, held_value)| {
                    attribute == held_attribute && held_value > value
                })
            })
    })
}

fn static_attribute(definition: &BuffDefinition) -> Option<(AttrId, i32)> {
    if definition.status != BuffStatus::StatsUp
        || !definition.uses_shared_type_family()
        || !definition
            .features()
            .iter()
            .all(|feature| feature.arguments_supported && feature.kind == Some(BuffActKind::Attr))
    {
        return None;
    }
    match definition.attribute_deltas() {
        &[(attribute, value)] if value > 0 => Some((attribute, value)),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
