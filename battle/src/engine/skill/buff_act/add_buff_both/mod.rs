use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrant, BuffRemove, BuffRemoveSelector},
    },
    skill::{
        action::SkillPhase,
        buff_act::{command_origin, registry::BuffActKind, subscriber_is_kind},
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
        target::TargetPool,
    },
};

pub fn rule_ops(
    managers: &BattleManagers,
    pool: &TargetPool,
    determinism: &mut crate::engine::runtime::determinism::RoundDeterminism,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !subscriber_is_kind(subscriber, BuffActKind::AddBuffBoth) {
        return None;
    }
    let BattleEvent::SkillAction(action) = event else {
        return None;
    };
    if action.phase != SkillPhase::HitPassives
        || !action.is_attack
        || action.source_uid != subscriber.owner_uid
    {
        return Some(Vec::new());
    }
    let [enemy_buff_id, ally_target_code, ally_buff_id] = subscriber.args.as_slice() else {
        return None;
    };
    if *enemy_buff_id <= 0 || *ally_target_code <= 0 || *ally_buff_id <= 0 {
        return None;
    }

    let origin = command_origin(subscriber)?;
    let source_uid = if subscriber.source_uid == 0 {
        subscriber.owner_uid
    } else {
        subscriber.source_uid
    };
    let ally_targets =
        crate::engine::skill::target::TargetResolver::resolve_with_managers_and_context(
            &crate::engine::skill::target::TargetRequest {
                code: *ally_target_code,
                raw: Vec::new(),
            },
            action.skill_id,
            subscriber.owner_uid,
            pool,
            determinism,
            Some(managers),
            Default::default(),
        );
    let grants = action
        .attacked_target_uids
        .iter()
        .copied()
        .filter(|uid| {
            pool.enemies(subscriber.owner_uid, true)
                .iter()
                .any(|enemy| enemy.uid == *uid)
        })
        .map(|uid| (uid, *enemy_buff_id))
        .chain(ally_targets.into_iter().map(|uid| (uid, *ally_buff_id)))
        .map(|(target_uid, buff_id)| {
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                origin,
                source_uid,
                target_uid,
                buff_id,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            })))
        });

    Some(
        grants
            .chain(std::iter::once(RuleOp::Command(BattleCommand::Buff(
                BuffCommand::Remove(BuffRemove {
                    origin,
                    target_uid: subscriber.owner_uid,
                    selector: BuffRemoveSelector::Uid(subscriber.buff_uid),
                }),
            ))))
            .collect(),
    )
}

#[cfg(test)]
mod tests;
