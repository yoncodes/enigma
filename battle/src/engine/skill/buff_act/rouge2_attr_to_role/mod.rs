use crate::engine::{
    entity::attr::AttrId,
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffAccumulateActValue, BuffCommand, BuffManager},
        hp::{HpCommand, MaxHpAdjust},
    },
    skill::{
        buff_act::{self, registry::BuffActKind},
        rule::output::{BattleCommand, RuleOp},
    },
};

pub const ACT_ID: i32 = 10_021;

const RULES: &[(i32, AttrId, i32)] = &[
    (101, AttrId::Hp, 20),
    (102, AttrId::ExtraDmg, 40),
    (103, AttrId::IncantationMight, 40),
    (104, AttrId::GenesisDmgBonus, 60),
    (104, AttrId::UltimateMight, 40),
];

pub fn supports(args: &[i32]) -> bool {
    let [mode_attr, raw_attr, multiplier] = args else {
        return false;
    };
    let Some(role_attr) = AttrId::from_raw(*raw_attr) else {
        return false;
    };
    RULES.contains(&(*mode_attr, role_attr, *multiplier))
}

pub fn attribute_delta(values: &[i32], attr_id: AttrId, buffs: &BuffManager) -> i32 {
    let [_, mode_attr, raw_attr, multiplier] = values else {
        return 0;
    };
    if !supports(&values[1..]) || AttrId::from_raw(*raw_attr) != Some(attr_id) {
        return 0;
    }
    buffs
        .mode_attribute(*mode_attr)
        .checked_mul(*multiplier)
        .unwrap_or_default()
}

pub fn hp_delta_fits(
    values: &[i32],
    buffs: &BuffManager,
    hp: &crate::engine::manager::hp::HpManager,
    target_uid: i64,
) -> bool {
    if values.get(2).copied() != Some(AttrId::Hp.id()) {
        return true;
    }
    let delta = attribute_delta(values, AttrId::Hp, buffs);
    delta == 0
        || (hp.current(target_uid).checked_add(delta).is_some()
            && hp.max(target_uid).checked_add(delta).is_some())
}

fn add_hp_delta(managers: &BattleManagers, feature: &ActiveBuffFeature) -> Option<i32> {
    let [_, mode_attr, raw_attr, multiplier] = feature.values.as_slice() else {
        return None;
    };
    if !supports(&feature.values[1..]) || AttrId::from_raw(*raw_attr) != Some(AttrId::Hp) {
        return None;
    }
    let delta = managers
        .buff
        .mode_attribute(*mode_attr)
        .checked_mul(*multiplier)?;
    if delta == 0 {
        return None;
    }
    managers.hp.current(feature.owner_uid).checked_add(delta)?;
    managers.hp.max(feature.owner_uid).checked_add(delta)?;
    Some(delta)
}

fn max_hp_rule_op(feature: &ActiveBuffFeature, delta: i32) -> Option<RuleOp> {
    if delta == 0 {
        return None;
    }
    Some(RuleOp::Command(BattleCommand::Hp(HpCommand::AdjustMax(
        MaxHpAdjust {
            origin: buff_act::feature_command_origin(feature)?,
            source_uid: feature.source_uid,
            target_uid: feature.owner_uid,
            delta,
        },
    ))))
}

pub fn transaction_rule_ops(
    managers: &BattleManagers,
    event: &crate::engine::event::payload::BattleEvent,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    use crate::engine::event::payload::BattleEvent;

    let (change, added) = match event {
        BattleEvent::BuffAdded(change) => (change, true),
        BattleEvent::BuffRemoved(change) => (change, false),
        BattleEvent::BuffChanged(_) => return Vec::new(),
        _ => return Vec::new(),
    };
    managers
        .buff
        .definition_features(change.buff_id)
        .into_iter()
        .filter(|feature| buff_act::is_kind(feature, BuffActKind::Rouge2AttrToRole))
        .flat_map(|mut feature| {
            feature.owner_uid = change.target_uid;
            feature.source_uid = change.source_uid;
            feature.buff_uid = change.buff_uid;
            let mut output = Vec::new();
            let hp_delta = if added {
                add_hp_delta(managers, &feature)
            } else if supports(&feature.values[1..])
                && feature.values.get(2).copied() == Some(AttrId::Hp.id())
            {
                change.act_value.checked_neg()
            } else {
                None
            };
            if let Some(delta) = hp_delta {
                if added && let Some(origin) = buff_act::feature_command_origin(&feature) {
                    output.push((
                        feature.clone(),
                        RuleOp::Command(BattleCommand::Buff(BuffCommand::AccumulateActValue(
                            BuffAccumulateActValue {
                                origin,
                                target_uid: feature.owner_uid,
                                buff_uid: feature.buff_uid,
                                act_id: ACT_ID,
                                delta,
                            },
                        ))),
                    ));
                }
                if let Some(op) = max_hp_rule_op(&feature, delta) {
                    output.push((feature.clone(), op));
                }
            }
            if added {
                output.push((
                    feature.clone(),
                    RuleOp::BuffFeatureMarker {
                        target_uid: feature.owner_uid,
                        effect_type: sonettobuf::effect_type_enum::EffectType::None as i32,
                        effect_num: 0,
                        buff_act_id: 0,
                    },
                ));
            }
            output
        })
        .collect()
}

#[cfg(test)]
mod tests;
