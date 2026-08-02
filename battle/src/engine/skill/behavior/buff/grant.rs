use super::*;

pub(super) fn grant_command(
    source_uid: i64,
    target_uid: i64,
    occurrences: u32,
    behavior: &ParsedBehavior,
) -> Option<BuffCommand> {
    let definition = super::registry::find(behavior)?;
    matches!(
        definition.kind,
        BehaviorKind::AddBuff
            | BehaviorKind::AddBuffPowerUse
            | BehaviorKind::AddBuffRound
            | BehaviorKind::AddBuffRound2
            | BehaviorKind::AddBuffBySkillBuffAdditions
    )
    .then_some(BuffCommand::Grant(BuffGrant {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: definition.key,
        },
        source_uid,
        target_uid,
        buff_id: behavior.arg(0)?,
        amount: behavior.arg(1),
        occurrences,
        child_uid_reservations: 0,
    }))
}

pub(super) fn random_pool_grant_commands(
    context: &mut BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let definition = super::registry::find(behavior)?;
    if definition.kind != BehaviorKind::AddBuffRanId {
        return None;
    }
    let [_, count] = behavior.args.as_slice() else {
        return None;
    };
    let mut candidates = random_buff_pool(behavior)?
        .into_iter()
        .filter(|buff_id| {
            !context
                .managers
                .buff
                .has_buff_id(context.target_uid, *buff_id)
        })
        .collect::<Vec<_>>();
    let mut ops = Vec::new();
    for _ in 0..((*count).max(0) as usize).min(candidates.len()) {
        let index = context
            .determinism
            .take_random_buff(&candidates)
            .and_then(|buff_id| {
                candidates
                    .iter()
                    .position(|candidate| *candidate == buff_id)
            })
            .or_else(|| context.determinism.lua_random_index(candidates.len()))?;
        let buff_id = candidates.remove(index);
        ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
            BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: definition.key,
                },
                source_uid: context.source_uid,
                target_uid: context.target_uid,
                buff_id,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            },
        ))));
    }
    Some(ops)
}

pub(super) fn supports_random_pool(behavior: &ParsedBehavior) -> bool {
    let [pool_buff_id, count] = behavior.args.as_slice() else {
        return false;
    };
    *pool_buff_id > 0
        && *count > 0
        && random_buff_pool(behavior).is_some_and(|pool| *count as usize <= pool.len())
}

pub fn random_buff_pool(behavior: &ParsedBehavior) -> Option<Vec<i32>> {
    let definition = super::registry::find(behavior)?;
    if definition.kind != BehaviorKind::AddBuffRanId {
        return None;
    }
    config::try_get()
        .and_then(|db| db.skill_buff.get(behavior.arg(0)?))
        .map(|row| pool_buff_ids(&row.features))
}
