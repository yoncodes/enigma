use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrant},
    },
    skill::{
        effect::SkillEffectCatalog,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [per_moxie, single_target_buff_id, mass_target_buff_id]
        if *per_moxie > 0 && *single_target_buff_id > 0 && *mass_target_buff_id > 0)
        || matches!(args, [per_moxie, single_target_buff_id, mass_target_buff_id, trigger_skill_id, bonus]
            if *per_moxie > 0 && *single_target_buff_id > 0 && *mass_target_buff_id > 0
                && *trigger_skill_id > 0 && *bonus > 0)
}

pub fn rule_ops(
    catalog: &SkillEffectCatalog,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(
        subscriber,
        super::registry::BuffActKind::AddBuffByOtherExSkill,
    ) || !supports(&subscriber.args)
    {
        return None;
    }
    let BattleEvent::AllyAction(action) = event else {
        return Some(Vec::new());
    };
    Some(
        rule_op(
            catalog,
            subscriber,
            action.source_uid,
            action.skill_id,
            action.skill_type,
            action.moxie_spent,
            action.trigger,
        )
        .into_iter()
        .collect(),
    )
}

pub fn rule_op(
    catalog: &SkillEffectCatalog,
    subscriber: &BuffActSubscriber,
    action_source_uid: i64,
    active_skill_id: i32,
    skill_type: i32,
    moxie_spent: i32,
    trigger: crate::engine::skill::action::SkillTrigger,
) -> Option<RuleOp> {
    if !super::subscriber_is_kind(
        subscriber,
        super::registry::BuffActKind::AddBuffByOtherExSkill,
    ) || subscriber.owner_uid == action_source_uid
        || !catalog.is_big_skill(active_skill_id)
        || moxie_spent <= 0
    {
        return None;
    }
    let [
        per_moxie,
        single_target_buff_id,
        mass_target_buff_id,
        rest @ ..,
    ] = subscriber.args.as_slice()
    else {
        return None;
    };
    let buff_id = if skill_type == 2 {
        *mass_target_buff_id
    } else {
        *single_target_buff_id
    };
    let bonus = match rest {
        [] => 0,
        [trigger_skill_id, bonus]
            if trigger
                == crate::engine::skill::action::SkillTrigger::Explicit(*trigger_skill_id) =>
        {
            *bonus
        }
        [_, _] => 0,
        _ => return None,
    };
    let amount = per_moxie.checked_mul(moxie_spent)?.checked_add(bonus)?;
    if buff_id <= 0 || amount <= 0 {
        return None;
    }
    Some(RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
        BuffGrant {
            origin: super::command_origin(subscriber)?,
            source_uid: if subscriber.source_uid != 0 {
                subscriber.source_uid
            } else {
                subscriber.owner_uid
            },
            target_uid: subscriber.owner_uid,
            buff_id,
            amount: Some(amount),
            occurrences: 1,
            child_uid_reservations: 0,
        },
    ))))
}

pub fn grant_transaction_rule_ops(
    managers: &BattleManagers,
    event: &BattleEvent,
) -> Vec<(crate::engine::manager::buff::ActiveBuffFeature, RuleOp)> {
    super::changed_features(
        managers,
        event,
        super::registry::BuffActKind::AddBuffByOtherExSkill,
    )
    .into_iter()
    .map(|(feature, _)| {
        let op = RuleOp::BuffFeatureMarker {
            target_uid: feature.owner_uid,
            effect_type: sonettobuf::effect_type_enum::EffectType::None as i32,
            effect_num: 0,
            buff_act_id: 0,
        };
        (feature, op)
    })
    .collect()
}

#[cfg(test)]
mod tests;
