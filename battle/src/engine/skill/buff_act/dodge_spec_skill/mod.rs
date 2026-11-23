use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffCommand, BuffConsume, BuffSelector, DepletedBuff},
    },
    skill::{
        buff_act::{is_kind, registry::BuffActKind},
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
        target::EntityDamageType,
    },
};

pub fn supports_skill_slots(skill_slots: &[i32]) -> bool {
    !skill_slots.is_empty()
        && skill_slots
            .iter()
            .all(|skill_slot| (1..=2).contains(skill_slot))
}

pub fn supports_damage_types(selectors: &[i32]) -> bool {
    matches!(selectors, [1] | [2] | [1, 2] | [1, 2, 3])
}

pub fn avoidance(
    managers: &BattleManagers,
    target_uid: i64,
    skill_slot: i32,
    damage_type: EntityDamageType,
) -> Option<ActiveBuffFeature> {
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| feature.owner_uid == target_uid)
        .filter(|feature| {
            let Some(selectors) = feature.values.get(1..) else {
                return false;
            };
            match super::feature_kind(feature) {
                Some(BuffActKind::DodgeSpecSkill) => selectors.contains(&skill_slot),
                Some(BuffActKind::DodgeDamageType) => {
                    selectors.contains(&damage_type.id())
                        && (skill_slot != 3 || selectors.contains(&3))
                }
                _ => false,
            }
        })
        .min_by_key(|feature| (feature.buff_uid, feature.buff_id))
}

pub fn ignored(managers: &BattleManagers, source_uid: i64) -> bool {
    managers
        .buff
        .has_buff_act_kind(source_uid, BuffActKind::IgnoreDodgeSpecSkill)
}

pub fn marker_effect_type(feature: &ActiveBuffFeature) -> Option<i32> {
    super::wire::find(feature.act_id()?, &feature.act_type)?
        .markers(super::wire::WirePhase::Add)
        .first()
        .copied()
}

pub fn trigger_rule_ops(feature: &ActiveBuffFeature) -> Option<Vec<RuleOp>> {
    Some(vec![RuleOp::BuffFeatureMarker {
        target_uid: feature.owner_uid,
        effect_type: marker_effect_type(feature)?,
        effect_num: feature.buff_id,
        buff_act_id: feature.act_id()?,
    }])
}

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<super::BuffActRuleOp>> {
    if let BattleEvent::BuffFeatureTriggered(trigger) = event {
        if trigger.owner_uid != subscriber.owner_uid
            || trigger.source_uid != subscriber.source_uid
            || trigger.target_uid != subscriber.owner_uid
            || trigger.buff_uid != subscriber.buff_uid
            || trigger.buff_id != subscriber.buff_id
            || trigger.act_id != subscriber.key.definition.opcode
            || !configured_trigger_count(managers, subscriber.buff_id)
        {
            return Some(Vec::new());
        }
        return Some(vec![super::BuffActRuleOp::subscriber_from_applier(
            RuleOp::Command(BattleCommand::Buff(BuffCommand::ConsumeCount(
                BuffConsume {
                    origin: super::command_origin(subscriber)?,
                    target_uid: subscriber.owner_uid,
                    selector: BuffSelector::Uid(subscriber.buff_uid),
                    amount: 1,
                    depleted: DepletedBuff::Remove,
                },
            ))),
        )]);
    }
    let expires = match super::subscriber_kind(subscriber) {
        Some(BuffActKind::DodgeSpecSkill) => true,
        Some(BuffActKind::DodgeDamageType) => managers
            .catalog()
            .buff_expires_after_owner_attack(subscriber.buff_id),
        _ => return None,
    };
    if !expires {
        return Some(Vec::new());
    }
    let BattleEvent::AllyAction(action) = event else {
        return Some(Vec::new());
    };
    if action.source_uid != subscriber.owner_uid
        || (super::subscriber_is_kind(subscriber, BuffActKind::DodgeDamageType)
            && !action.is_attack)
    {
        return Some(Vec::new());
    }
    Some(vec![super::BuffActRuleOp::subscriber(RuleOp::Command(
        BattleCommand::Buff(crate::engine::manager::buff::BuffCommand::ExpireAction(
            crate::engine::manager::buff::BuffRemove {
                origin: super::command_origin(subscriber)?,
                target_uid: subscriber.owner_uid,
                selector: crate::engine::manager::buff::BuffRemoveSelector::Uid(
                    subscriber.buff_uid,
                ),
            },
        )),
    ))])
}

fn configured_trigger_count(managers: &BattleManagers, buff_id: i32) -> bool {
    managers.catalog().buff_has_effect_count(buff_id)
        && !managers.catalog().buff_expires_after_owner_attack(buff_id)
}

#[cfg(test)]
mod tests;
