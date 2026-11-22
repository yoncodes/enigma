use crate::engine::{
    event::payload::BattleEvent,
    manager::BattleManagers,
    skill::{
        action::{BuffActRoundCap, SkillInvocation, SkillRequest, SkillTarget},
        rule::output::RuleOp,
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [hp_permille, skill_id, limit]
        if *hp_permille > 0 && *skill_id > 0 && *limit > 0)
}

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let BattleEvent::Hit(hit) = event else {
        return None;
    };
    let [hp_permille, skill_id, limit] = subscriber.args.as_slice() else {
        return None;
    };
    let current = i64::from(managers.hp.current(subscriber.owner_uid));
    let max = i64::from(managers.hp.max(subscriber.owner_uid));
    if !supports(&subscriber.args)
        || hit.target_uid != subscriber.owner_uid
        || current <= 0
        || current * 1000 >= max * i64::from(*hp_permille)
        || managers.hp.current(subscriber.source_uid) <= 0
        || !managers.can_fire_buff_act(subscriber.source_uid, 0, subscriber.key.definition, *limit)
    {
        return Some(Vec::new());
    }
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: subscriber.source_uid,
        skill_id: *skill_id,
    }
    .into();
    invocation.target = SkillTarget::Explicit(subscriber.owner_uid);
    invocation.round_cap = Some(BuffActRoundCap {
        owner_uid: subscriber.source_uid,
        buff_uid: 0,
        key: subscriber.key.definition,
        limit: *limit,
    });
    Some(vec![RuleOp::Skill(invocation)])
}

#[cfg(test)]
mod tests;
