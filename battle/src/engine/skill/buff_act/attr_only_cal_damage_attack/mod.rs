use crate::engine::{
    entity::attr::AttrId,
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffCommand, BuffConsume, BuffSelector, DepletedBuff},
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        target::EntityDamageType,
    },
};

pub fn supports_extra_action(args: &[i32]) -> bool {
    matches!(args, [raw_attr, value]
        if AttrId::from_raw(*raw_attr).is_some() && *value != 0)
}

pub fn supports_be_attacked_type(args: &[i32]) -> bool {
    matches!(
        args,
        [raw_damage_type, raw_attr, value, 1]
            if matches!(EntityDamageType::from_wire(*raw_damage_type), EntityDamageType::Reality | EntityDamageType::Mental)
                && AttrId::from_raw(*raw_attr) == Some(AttrId::DmgTakenReduction)
                && *value != 0
    )
}

pub fn supports_be_attacked(args: &[i32]) -> bool {
    matches!(
        args,
        [raw_attr, value, 1]
            if AttrId::from_raw(*raw_attr) == Some(AttrId::DmgTakenReduction) && *value != 0
    )
}

pub fn supports_basic_attacked(args: &[i32]) -> bool {
    matches!(args, [raw_attr, value]
        if matches!(AttrId::from_raw(*raw_attr),
            Some(AttrId::CriticalResistRate | AttrId::DmgTakenReduction))
            && *value != 0)
}

pub fn attribute_delta(feature: &ActiveBuffFeature, attr_id: AttrId) -> i32 {
    match feature.values.as_slice() {
        [113, raw_attr_id, value, ..] if *raw_attr_id == attr_id as i32 => *value,
        [_, raw_attr_id, value, ..] if *raw_attr_id == attr_id as i32 => value * feature.amount,
        _ => 0,
    }
}

pub fn extra_action_attribute(feature: &ActiveBuffFeature) -> Option<(AttrId, i32)> {
    matches!(
        super::feature_kind(feature),
        Some(super::registry::BuffActKind::AttrOnlyCalDamageInExtra)
    )
    .then_some(())?;
    let raw_attr = *feature.values.get(1)?;
    let attr_id = AttrId::from_raw(raw_attr)?;
    let delta = attribute_delta(feature, attr_id);
    (delta != 0).then_some((attr_id, delta))
}

pub fn consumes_after_attack(feature: &ActiveBuffFeature) -> bool {
    match super::feature_kind(feature) {
        Some(super::registry::BuffActKind::AttrOnlyCalDamageBeAttackedType) => {
            matches!(feature.values.as_slice(), [_, _, _, _, consume] if *consume != 0)
        }
        _ => matches!(feature.values.as_slice(), [_, _, _, consume, ..] if *consume != 0),
    }
}

pub fn applies_to_incoming_damage(
    feature: &ActiveBuffFeature,
    damage_type: EntityDamageType,
) -> bool {
    match super::feature_kind(feature) {
        Some(super::registry::BuffActKind::AttrOnlyCalDamageBeAttacked) => true,
        Some(super::registry::BuffActKind::AttrOnlyCalDamageBeAttackedType) => {
            matches!(feature.values.as_slice(), [_, raw_damage_type, ..]
                if EntityDamageType::from_wire(*raw_damage_type) == damage_type)
        }
        _ => false,
    }
}

pub fn applies_to_any_incoming_damage(
    feature: &ActiveBuffFeature,
    damage_types: &[EntityDamageType],
    is_big_skill: bool,
) -> bool {
    match super::feature_kind(feature) {
        Some(super::registry::BuffActKind::AttrOnlyCalDamageBasicAttacked) => {
            !is_big_skill && !damage_types.is_empty()
        }
        Some(super::registry::BuffActKind::AttrOnlyCalDamageBeAttacked) => true,
        Some(super::registry::BuffActKind::AttrOnlyCalDamageBeAttackedType) => damage_types
            .iter()
            .any(|damage_type| applies_to_incoming_damage(feature, *damage_type)),
        _ => false,
    }
}

pub fn incoming_attribute_delta(
    feature: &ActiveBuffFeature,
    damage_type: EntityDamageType,
    attr_id: AttrId,
) -> i32 {
    if !applies_to_incoming_damage(feature, damage_type) {
        return 0;
    }
    match super::feature_kind(feature) {
        Some(super::registry::BuffActKind::AttrOnlyCalDamageBeAttackedType) => {
            match feature.values.as_slice() {
                [_, _, raw_attr, value, _] if AttrId::from_raw(*raw_attr) == Some(attr_id) => {
                    value.saturating_mul(feature.amount)
                }
                _ => 0,
            }
        }
        Some(super::registry::BuffActKind::AttrOnlyCalDamageBeAttacked) => {
            match feature.values.as_slice() {
                [_, raw_attr, value, _] if AttrId::from_raw(*raw_attr) == Some(attr_id) => *value,
                _ => 0,
            }
        }
        _ => 0,
    }
}

pub fn basic_attacked_attribute_delta(
    feature: &ActiveBuffFeature,
    attr_id: AttrId,
    is_big_skill: bool,
) -> i32 {
    if is_big_skill
        || super::feature_kind(feature)
            != Some(super::registry::BuffActKind::AttrOnlyCalDamageBasicAttacked)
    {
        return 0;
    }
    attribute_delta(feature, attr_id)
}

pub fn applies_to_skill(feature: &ActiveBuffFeature, is_big_skill: bool) -> bool {
    match feature.values.as_slice() {
        [_, raw_attr_id, ..] if *raw_attr_id == AttrId::UltimateMight as i32 => is_big_skill,
        [_, raw_attr_id, ..] if *raw_attr_id == AttrId::IncantationMight as i32 => !is_big_skill,
        _ => true,
    }
}

pub fn consume_rule_op(managers: &BattleManagers, feature: &ActiveBuffFeature) -> Option<RuleOp> {
    let command = consume_command(managers, feature)?;
    Some(RuleOp::Command(BattleCommand::Buff(command)))
}

fn consume_command(managers: &BattleManagers, feature: &ActiveBuffFeature) -> Option<BuffCommand> {
    let kind = super::feature_kind(feature);
    if kind == Some(super::registry::BuffActKind::AttrOnlyCalDamageAttack)
        && managers
            .buff
            .settles_on_owner_cast(feature.owner_uid, feature.buff_uid)
    {
        return None;
    }
    let consumes_effect_count = kind
        == Some(super::registry::BuffActKind::AttrOnlyCalDamageBasicAttacked)
        && managers.catalog().buff_has_effect_count(feature.buff_id);
    if !consumes_effect_count && !consumes_after_attack(feature) {
        return None;
    }
    managers
        .buff
        .snapshot(feature.owner_uid, feature.buff_uid)?;
    Some(BuffCommand::Consume(BuffConsume {
        origin: super::feature_command_origin(feature)?,
        target_uid: feature.owner_uid,
        selector: BuffSelector::Uid(feature.buff_uid),
        amount: 1,
        depleted: DepletedBuff::Remove,
    }))
}

pub(super) mod ultimate;

#[cfg(test)]
mod tests;
