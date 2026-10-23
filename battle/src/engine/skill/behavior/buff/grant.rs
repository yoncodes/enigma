use super::*;

pub(crate) fn tagged_group_completion(
    managers: &BattleManagers,
    source_uid: i64,
    target: &TargetRequest,
    target_uids: &[i64],
    behavior: &ParsedBehavior,
) -> Option<BuffCommand> {
    let buff_id = behavior.arg(0)?;
    if behavior.spec.kind != BehaviorKind::AddBuff
        || target.code != 132
        || target.raw.as_slice() != [131]
        || target_uids.len() <= 1
        || target_uids
            .iter()
            .any(|uid| managers.buff.has_buff_id(*uid, buff_id))
    {
        return None;
    }
    Some(BuffCommand::CompleteTaggedGroup(
        BuffTaggedGroupCompletion {
            origin: command_origin(behavior)?,
            source_uid,
            target_uids: target_uids.to_vec(),
            buff_id,
        },
    ))
}

pub(super) fn grant_command(
    source_uid: i64,
    target_uid: i64,
    occurrences: u32,
    event: Option<&crate::engine::event::payload::BattleEvent>,
    behavior: &ParsedBehavior,
) -> Option<BuffCommand> {
    let definition = super::registry::find(behavior)?;
    let buff_id = behavior.arg(0)?;
    matches!(
        definition.kind,
        BehaviorKind::AddBuff
            | BehaviorKind::AddBuffPowerUse
            | BehaviorKind::AddBuffRound
            | BehaviorKind::AddBuffRound2
            | BehaviorKind::AddBuffBySkillBuffAdditions
    )
    .then(|| BuffGrant {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: definition.key,
        },
        source_uid,
        target_uid,
        buff_id,
        amount: behavior.arg(1),
        occurrences,
        child_uid_reservations: 0,
    })
    .map(|grant| {
        if matches!(
            event,
            Some(crate::engine::event::payload::BattleEvent::EntityDied(_))
        ) {
            BuffCommand::GrantOnDeath(grant)
        } else {
            BuffCommand::Grant(grant)
        }
    })
}

pub(super) fn random_pool_grant_commands(
    context: &mut BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let definition = super::registry::find(behavior)?;
    if !matches!(
        definition.kind,
        BehaviorKind::AddBuffRanId | BehaviorKind::AddBuffRanTypeId
    ) {
        return None;
    }
    let [_, count] = behavior.args.as_slice() else {
        return None;
    };
    let mut candidates = resolve_random_buff_pool(context.managers.catalog(), behavior)?
        .into_iter()
        .filter(|buff_id| match definition.kind {
            BehaviorKind::AddBuffRanId => !context
                .managers
                .buff
                .has_buff_id(context.target_uid, *buff_id),
            BehaviorKind::AddBuffRanTypeId => true,
            _ => false,
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
    resolve_random_buff_pool(crate::catalog::BattleCatalog::try_global()?, behavior)
}

fn resolve_random_buff_pool(
    catalog: crate::catalog::BattleCatalog,
    behavior: &ParsedBehavior,
) -> Option<Vec<i32>> {
    let definition = super::registry::find(behavior)?;
    if !matches!(
        definition.kind,
        BehaviorKind::AddBuffRanId | BehaviorKind::AddBuffRanTypeId
    ) {
        return None;
    }
    catalog.buff_pool(behavior.arg(0)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tagged_multi_target_buff_application_owns_one_completion_uid() {
        crate::test_support::init_config();
        let behavior = ParsedBehavior::new(1, "AddBuff", vec![31450132]);
        let tagged_allies = TargetRequest {
            code: 132,
            raw: vec![131],
        };

        let managers = BattleManagers::default();
        assert!(matches!(
            tagged_group_completion(&managers, 10, &tagged_allies, &[10, 11, 12, 13], &behavior),
            Some(BuffCommand::CompleteTaggedGroup(
                BuffTaggedGroupCompletion {
                    source_uid: 10,
                    buff_id: 31450132,
                    ..
                }
            ))
        ));
        assert!(tagged_group_completion(&managers, 10, &tagged_allies, &[10], &behavior).is_none());
        assert!(
            tagged_group_completion(
                &managers,
                10,
                &TargetRequest {
                    code: 132,
                    raw: vec![114],
                },
                &[10, 11, 12, 13],
                &behavior,
            )
            .is_none()
        );
    }
}
