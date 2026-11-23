use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    damage::{DeadlyPoisonFormulaInput, deadly_poison_damage},
    entity::attr::AttrId,
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffAccumulateActValue, BuffCommand},
        hp::{HpCommand, HpLoss, HurtDamageFromType, HurtInfoData},
    },
    skill::{
        buff_act::{self, registry::BuffActKind},
        rule::{
            CommandOrigin,
            output::{BattleCommand, RuleOp},
        },
        subscriber::BuffActSubscriber,
    },
};

pub fn runtime_rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !buff_act::subscriber_is_kind(subscriber, BuffActKind::DeadlyPoison) {
        return None;
    }
    if !matches!(event, BattleEvent::Kind(EventKind::RoundEnd)) {
        return Some(Vec::new());
    }
    settlement_ops(
        managers,
        &ActiveBuffFeature {
            owner_uid: subscriber.owner_uid,
            source_uid: subscriber.source_uid,
            buff_uid: subscriber.buff_uid,
            buff_id: subscriber.buff_id,
            amount: subscriber.amount,
            team_type: subscriber.team_type,
            owner_alive: subscriber.owner_alive,
            act_type: subscriber.act_type.clone(),
            effect_time: subscriber.effect_time,
            effect_condition: subscriber.effect_condition,
            raw: subscriber.raw.clone(),
            values: std::iter::once(subscriber.key.definition.opcode)
                .chain(subscriber.args.iter().copied())
                .collect(),
        },
        buff_act::command_origin(subscriber)?,
        1,
    )
}

pub fn settlement_ops(
    managers: &BattleManagers,
    feature: &ActiveBuffFeature,
    origin: CommandOrigin,
    times: i32,
) -> Option<Vec<RuleOp>> {
    if !buff_act::is_kind(feature, BuffActKind::DeadlyPoison) || times < 0 {
        return None;
    }
    let [act_id, base_permille, compound_permille, compound_cap, ..] = feature.values.as_slice()
    else {
        return None;
    };
    if *base_permille <= 0 || *compound_permille < 0 || *compound_cap < 0 {
        return None;
    }
    let source_attack = managers.origin_attribute(feature.source_uid, AttrId::Attack);
    let previous = managers
        .buff
        .act_value(feature.buff_uid, *act_id)
        .max(0)
        .saturating_add(super::dudu_bone_continue_channel::compound_offset(
            managers,
            feature.source_uid,
        ));
    let mut ops = Vec::new();
    for offset in 0..times {
        let compound = (previous + offset).min(*compound_cap);
        let amount = deadly_poison_damage(DeadlyPoisonFormulaInput {
            source_attack,
            base_rate: *base_permille,
            stacks: feature.amount.max(1),
            compound_count: compound,
            compound_rate: *compound_permille,
        });
        if amount > 0 {
            ops.push(RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(
                HpLoss {
                    origin,
                    source_uid: feature.source_uid,
                    target_uid: feature.owner_uid,
                    amount,
                    config_effect: 0,
                    hurt: Some(HurtInfoData {
                        from_uid: feature.source_uid,
                        is_crit: false,
                        career_restraint: false,
                        reduce_hp: 0,
                        effect_id: 0,
                        skill_id: 0,
                        damage_from: HurtDamageFromType::Buff,
                        buff_act_id: *act_id,
                        buff_uid: feature.buff_uid,
                        hurt_effect_type: EffectType::Deadlypoisonorigindamage as i32,
                        display_amount: None,
                    }),
                },
            ))));
        }
        ops.push(RuleOp::Command(BattleCommand::Buff(
            BuffCommand::AccumulateActValue(BuffAccumulateActValue {
                origin,
                target_uid: feature.owner_uid,
                buff_uid: feature.buff_uid,
                act_id: *act_id,
                delta: 1,
            }),
        )));
    }
    Some(ops)
}

#[cfg(test)]
mod tests;
