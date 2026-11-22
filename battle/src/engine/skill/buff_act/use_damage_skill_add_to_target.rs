use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffConsume, BuffGrant, BuffSelector, DepletedBuff},
    },
    skill::{
        action::SkillPhase,
        effect::SkillEffectCatalog,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn rule_ops(
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<super::BuffActRuleOp>> {
    if !super::subscriber_is_kind(
        subscriber,
        super::registry::BuffActKind::UseDamageSkillAddToTarget,
    ) {
        return None;
    }
    let (source_uid, target_uids) = match event {
        BattleEvent::SkillEffectStarted(action)
            if action.source_uid == subscriber.owner_uid
                && action.phase == SkillPhase::Immediate
                && catalog.damage_rate(action.skill_id) > 0 =>
        {
            (action.source_uid, Some(action.target_uids.as_slice()))
        }
        BattleEvent::SkillAction(action)
            if action.source_uid == subscriber.owner_uid
                && action.phase == SkillPhase::HitPassives
                && subscriber.key.event == crate::engine::event::kind::EventKind::SkillCast
                && catalog.damage_rate(action.skill_id) > 0 =>
        {
            (action.source_uid, None)
        }
        _ => return Some(Vec::new()),
    };
    let [_, buff_id, ..] = subscriber.args.as_slice() else {
        return None;
    };
    let stored = managers
        .buff
        .snapshot(subscriber.owner_uid, subscriber.buff_uid)?;
    let activation_count = stored.count.unwrap_or_default();
    if activation_count <= 0 {
        return Some(Vec::new());
    }
    let origin = super::command_origin(subscriber)?;
    if target_uids.is_none() {
        return Some(vec![super::BuffActRuleOp::subscriber_from_applier(
            RuleOp::Command(BattleCommand::Buff(BuffCommand::ConsumeCount(
                BuffConsume {
                    origin,
                    target_uid: subscriber.owner_uid,
                    selector: BuffSelector::Uid(subscriber.buff_uid),
                    amount: activation_count,
                    depleted: DepletedBuff::Remove,
                },
            ))),
        )]);
    }
    let stacks = stored.layer.unwrap_or_default().max(0);
    if stacks == 0 || *buff_id <= 0 {
        return Some(Vec::new());
    }
    let occurrences = u32::try_from(stacks).ok()?;
    let mut ops = Vec::new();
    for target_uid in target_uids
        .expect("skill-action branch has targets")
        .iter()
        .copied()
        .filter(|target_uid| *target_uid != 0)
    {
        ops.push(super::BuffActRuleOp::causing(RuleOp::Command(
            BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                origin,
                source_uid,
                target_uid,
                buff_id: *buff_id,
                amount: None,
                occurrences,
                child_uid_reservations: 0,
            })),
        )));
        ops.push(super::BuffActRuleOp::causing(RuleOp::BuffActTrigger(
            crate::engine::manager::buff::BuffActTriggerResult {
                target_uid: source_uid,
                buff_id: subscriber.buff_id,
                buff_act_id: subscriber.key.definition.opcode,
            },
        )));
    }
    Some(ops)
}

#[cfg(test)]
mod tests;
