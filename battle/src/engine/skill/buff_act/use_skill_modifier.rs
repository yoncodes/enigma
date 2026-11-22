use crate::engine::{
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        hp::{HpCommand, HpLoss, HurtDamageFromType, HurtInfoData},
    },
    skill::{
        action::{SkillExecutionMode, SkillPhase},
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports_loss(args: &[i32]) -> bool {
    matches!(args, [rate] if *rate > 0)
}

pub fn supports_attribute(args: &[i32]) -> bool {
    matches!(args, [raw_attr, value]
        if AttrId::from_raw(*raw_attr).is_some() && *value != 0)
}

fn is_active_cast(mode: SkillExecutionMode) -> bool {
    matches!(
        mode,
        SkillExecutionMode::Active | SkillExecutionMode::DirectBig
    )
}

fn uses_active_attributes(mode: SkillExecutionMode) -> bool {
    is_active_cast(mode) || mode == SkillExecutionMode::Synchronization
}

pub fn loss_rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let BattleEvent::SkillAction(action) = event else {
        return None;
    };
    let [rate] = subscriber.args.as_slice() else {
        return None;
    };
    if !supports_loss(&subscriber.args)
        || action.source_uid != subscriber.owner_uid
        || action.phase != SkillPhase::Immediate
        || !is_active_cast(action.mode)
        || !subscriber.owner_alive
    {
        return Some(Vec::new());
    }
    let amount = managers.hp.current(subscriber.owner_uid).max(0) * *rate / 1000;
    if amount <= 0 {
        return Some(Vec::new());
    }
    let origin = super::command_origin(subscriber)?;
    Some(vec![RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(
        HpLoss {
            origin,
            source_uid: subscriber.owner_uid,
            target_uid: subscriber.owner_uid,
            amount,
            config_effect: 0,
            hurt: Some(HurtInfoData {
                from_uid: subscriber.source_uid,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: action.skill_id,
                skill_id: action.skill_id,
                damage_from: HurtDamageFromType::Buff,
                buff_act_id: subscriber.key.definition.opcode,
                buff_uid: subscriber.buff_uid,
                hurt_effect_type: 0,
                display_amount: None,
            }),
        },
    )))])
}

pub fn attribute_deltas(
    managers: &BattleManagers,
    owner_uid: i64,
    mode: SkillExecutionMode,
) -> Vec<(AttrId, i32)> {
    if !uses_active_attributes(mode) {
        return Vec::new();
    }
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| {
            feature.owner_uid == owner_uid
                && super::is_kind(feature, super::registry::BuffActKind::UseSkillAttrFix)
        })
        .filter_map(|feature| {
            let [_, raw_attr, value] = feature.values.as_slice() else {
                return None;
            };
            supports_attribute(&[*raw_attr, *value])
                .then(|| AttrId::from_raw(*raw_attr).map(|attr| (attr, *value)))
                .flatten()
        })
        .collect()
}

#[cfg(test)]
mod tests;
