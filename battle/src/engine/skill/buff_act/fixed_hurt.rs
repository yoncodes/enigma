use crate::engine::manager::{
    buff::{ActiveBuffFeature, BuffManager},
    hp::{HpCommand, HpManager},
};

use super::{is_kind, registry::BuffActKind};

pub fn amount(buffs: &BuffManager, hp: &HpManager, owner_uid: i64) -> Option<i32> {
    buffs
        .active_features(hp)
        .iter()
        .find_map(|feature| amount_from_feature(feature, owner_uid))
}

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [amount] if *amount >= 0)
}

pub fn resolve_command(buffs: &BuffManager, hp: &HpManager, mut command: HpCommand) -> HpCommand {
    if let HpCommand::Damage(damage) = &mut command
        && damage.amount > 0
        && let Some(amount) = amount(buffs, hp, damage.target_uid)
    {
        if crate::engine::damage::trace_enabled() {
            eprintln!(
                "fixed hurt target={} input={} output={amount}",
                damage.target_uid, damage.amount
            );
        }
        damage.amount = amount;
    }
    command
}

fn amount_from_feature(feature: &ActiveBuffFeature, owner_uid: i64) -> Option<i32> {
    (feature.owner_uid == owner_uid && is_kind(feature, BuffActKind::FixedHurt))
        .then_some(feature.values.as_slice())
        .and_then(|values| match values {
            [_, amount] if *amount >= 0 => Some(*amount),
            _ => None,
        })
}
