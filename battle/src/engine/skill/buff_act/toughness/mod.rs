use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::ActiveBuffFeature,
        toughness::{STANDARD_DAMAGE_RATE_PERMILLE, ToughnessRecord},
    },
    skill::{
        behavior::{classify::BehaviorKind, registry as behavior_registry},
        buff_act::registry::BuffActKind,
        rule::{
            RuleDomain,
            output::{BattleCommand, RuleOp},
        },
    },
};

pub fn transaction_rule_ops(
    managers: &BattleManagers,
    event: &BattleEvent,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    let BattleEvent::HpLost {
        origin,
        target_uid,
        amount,
        requested_loss,
        ..
    } = event
    else {
        return Vec::new();
    };
    if origin.domain != RuleDomain::Behavior
        || behavior_registry::find_key(origin.key.opcode, origin.key.type_name)
            .is_none_or(|definition| definition.kind != BehaviorKind::ToughnessOverflowDamage)
        || !managers.toughness.is_broken(*target_uid)
    {
        return Vec::new();
    }

    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| {
            feature.owner_uid == *target_uid
                && super::is_kind(feature, BuffActKind::ToughnessOverflowRecord)
        })
        .min_by_key(|feature| feature.buff_uid)
        .map(|feature| {
            (
                feature,
                RuleOp::Command(BattleCommand::ToughnessRecord(ToughnessRecord {
                    target_uid: *target_uid,
                    damage: requested_loss.unwrap_or(*amount),
                    rate_permille: STANDARD_DAMAGE_RATE_PERMILLE,
                })),
            )
        })
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests;
