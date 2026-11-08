use crate::engine::{
    entity::attr::AttrId,
    manager::{BattleManagers, buff::ActiveBuffFeature},
};

use super::{is_kind, registry::BuffActKind};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [first_attr, first_delta, second_attr, second_delta]
        if AttrId::from_raw(*first_attr) == Some(AttrId::DmgBonus)
            && AttrId::from_raw(*second_attr) == Some(AttrId::DmgBonus)
            && (*first_delta != 0 || *second_delta != 0))
}

fn attributes(feature: &ActiveBuffFeature) -> [Option<(AttrId, i32)>; 2] {
    let deltas = match feature.values.as_slice() {
        [_, _, first_delta, _, second_delta] if is_kind(feature, BuffActKind::HedoneFixAttr) => {
            [*first_delta, *second_delta]
        }
        _ => [0, 0],
    };
    deltas.map(|delta| {
        (delta != 0).then_some((
            if delta > 0 {
                AttrId::DmgBonus
            } else {
                AttrId::DmgTakenReduction
            },
            delta,
        ))
    })
}

pub(crate) fn incoming_attributes(
    managers: &BattleManagers,
    target_uid: i64,
    is_conduit: bool,
) -> Vec<(AttrId, i32)> {
    if !is_conduit {
        return Vec::new();
    }
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| feature.owner_uid == target_uid)
        .flat_map(|feature| attributes(&feature).into_iter().flatten())
        .collect()
}

#[cfg(test)]
mod tests;
