use crate::engine::{
    event::payload::BattleEvent,
    manager::buff::{BuffCommand, BuffRemove, BuffRemoveSelector},
    skill::{
        action::{SkillInvocation, SkillRequest, SkillTarget},
        buff_act::registry::BuffActKind,
        condition::extra::ExtraSkillKind,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
        target::TargetPool,
    },
};

mod setup;
pub(crate) use setup::grant_setup;

pub fn rule_ops(
    pool: &TargetPool,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::BeatBackDependOnAttackMe) {
        return None;
    }
    let BattleEvent::SkillAction(action) = event else {
        return Some(Vec::new());
    };
    let owner_team = pool.source_is_attacker(subscriber.owner_uid);
    if !subscriber.owner_alive
        || !action.is_attack
        || action.ignore_riposte
        || pool.source_is_attacker(action.source_uid) == owner_team
    {
        return Some(Vec::new());
    }
    let owner_was_attacked = action.attacked_target_uids.contains(&subscriber.owner_uid);
    let ally_was_attacked = action.attacked_target_uids.iter().any(|target_uid| {
        *target_uid != subscriber.owner_uid && pool.source_is_attacker(*target_uid) == owner_team
    });
    let skill_id = if owner_was_attacked {
        subscriber.args.first()
    } else if ally_was_attacked {
        subscriber.args.get(1)
    } else {
        return Some(Vec::new());
    }
    .copied()
    .filter(|skill_id| *skill_id > 0)?;
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: subscriber.owner_uid,
        skill_id,
    }
    .into();
    invocation.target = SkillTarget::Explicit(action.source_uid);
    invocation.extra_skill_kind = Some(ExtraSkillKind::Riposte);
    Some(vec![RuleOp::Skill(invocation)])
}

pub fn holder_rule_ops(
    pool: &TargetPool,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::BeatBack) {
        return None;
    }
    let BattleEvent::SkillAction(action) = event else {
        return Some(Vec::new());
    };
    let owner_team = pool.source_is_attacker(subscriber.owner_uid);
    let consumed_buff = holder_consumed_buff(&subscriber.args);
    let ally_was_attacked = action
        .attacked_target_uids
        .iter()
        .any(|target_uid| pool.source_is_attacker(*target_uid) == owner_team);
    if !subscriber.owner_alive
        || !action.is_attack
        || action.ignore_riposte
        || if consumed_buff.is_some() {
            !ally_was_attacked
        } else {
            !action.attacked_target_uids.contains(&subscriber.owner_uid)
        }
        || pool.source_is_attacker(action.source_uid) == owner_team
    {
        return Some(Vec::new());
    }
    let skill_id = holder_skill(&subscriber.args)?;
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: subscriber.owner_uid,
        skill_id,
    }
    .into();
    invocation.target = SkillTarget::Explicit(action.source_uid);
    invocation.extra_skill_kind = Some(ExtraSkillKind::Riposte);
    let mut ops = Vec::new();
    if let Some(buff_id) = consumed_buff {
        ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(
            BuffRemove {
                origin: super::command_origin(subscriber)?,
                target_uid: subscriber.owner_uid,
                selector: BuffRemoveSelector::ExactId(buff_id),
            },
        ))));
    }
    ops.push(RuleOp::Skill(invocation));
    Some(ops)
}

pub fn holder_skill(args: &[i32]) -> Option<i32> {
    match args {
        [0, skill_id] if *skill_id > 0 => Some(*skill_id),
        [0, skill_id, 1, buff_id] if *skill_id > 0 && *buff_id > 0 => Some(*skill_id),
        _ => None,
    }
}

pub fn holder_consumed_buff(args: &[i32]) -> Option<i32> {
    match args {
        [0, skill_id, 1, buff_id] if *skill_id > 0 && *buff_id > 0 => Some(*buff_id),
        _ => None,
    }
}

pub fn counter_skill(args: &[i32]) -> Option<i32> {
    match args {
        [skill_id] if *skill_id > 0 => Some(*skill_id),
        _ => None,
    }
}

pub fn supports_holder(args: &[i32]) -> bool {
    holder_skill(args).is_some()
}

pub fn supports_dependent(args: &[i32]) -> bool {
    matches!(args, [self_skill, ally_skill] if *self_skill > 0 && *ally_skill > 0)
}

#[cfg(test)]
mod tests;
