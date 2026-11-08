use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    damage::scale_permille,
    event::payload::BattleEvent,
    manager::{
        buff::{BuffCommand, BuffConsume, BuffSelector, DepletedBuff},
        emitter,
        hp::{HpCommand, HpLoss, HurtDamageFromType, HurtInfoData},
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [rate] if *rate > 0)
}

pub fn rule_ops(subscriber: &BuffActSubscriber, event: &BattleEvent) -> Option<Vec<RuleOp>> {
    let BattleEvent::SkillAction(action) = event else {
        return None;
    };
    let [rate] = subscriber.args.as_slice() else {
        return None;
    };
    if !supports(&subscriber.args)
        || !subscriber.owner_alive
        || action.source_uid != emitter::UID
        || !action.is_attack
        || action.damage_amount <= 0
        || !action.attacked_target_uids.contains(&subscriber.owner_uid)
    {
        return Some(Vec::new());
    }

    let amount = scale_permille(action.damage_amount, *rate);
    if amount <= 0 {
        return Some(Vec::new());
    }
    let origin = super::command_origin(subscriber)?;
    let attacked_targets = action
        .attacked_target_uids
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join("#");

    Some(vec![
        RuleOp::EffectMarker {
            target_uid: subscriber.owner_uid,
            effect_type: EffectType::Emitterextradamage as i32,
            effect_num: i32::try_from(subscriber.buff_uid).ok()?,
            effect_num1: Some(amount),
            config_effect: 0,
            reserve_id: None,
            reserve_str: Some(attacked_targets),
        },
        RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(HpLoss {
            origin,
            source_uid: subscriber.source_uid,
            target_uid: subscriber.owner_uid,
            amount,
            config_effect: 0,
            hurt: Some(HurtInfoData {
                from_uid: subscriber.source_uid,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 0,
                skill_id: 0,
                damage_from: HurtDamageFromType::Buff,
                buff_act_id: subscriber.key.definition.opcode,
                buff_uid: subscriber.buff_uid,
                hurt_effect_type: EffectType::Origindamage as i32,
                display_amount: Some(amount),
            }),
        }))),
        RuleOp::Command(BattleCommand::Buff(BuffCommand::ConsumeCount(
            BuffConsume {
                origin,
                target_uid: subscriber.owner_uid,
                selector: BuffSelector::Uid(subscriber.buff_uid),
                amount: 1,
                depleted: DepletedBuff::Keep,
            },
        ))),
    ])
}

#[cfg(test)]
mod tests;
