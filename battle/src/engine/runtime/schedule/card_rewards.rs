use crate::engine::{manager::ex_point::ExPointChange, skill::rule::CommandOrigin};

pub(super) fn changes(
    origin: CommandOrigin,
    rewards: impl IntoIterator<Item = (i64, i32)>,
) -> Vec<ExPointChange> {
    let rewards = rewards.into_iter().fold(
        Vec::<(i64, i32)>::new(),
        |mut combined, (owner_uid, delta)| {
            if let Some((_, current)) = combined.iter_mut().find(|(uid, _)| *uid == owner_uid) {
                *current = current.saturating_add(delta);
            } else {
                combined.push((owner_uid, delta));
            }
            combined
        },
    );
    rewards
        .into_iter()
        .map(|(owner_uid, delta)| ExPointChange {
            origin,
            source_uid: owner_uid,
            target_uid: owner_uid,
            delta,
            config_effect: 0,
            effect_type: sonettobuf::effect_type_enum::EffectType::Expointchange as i32,
        })
        .collect()
}
