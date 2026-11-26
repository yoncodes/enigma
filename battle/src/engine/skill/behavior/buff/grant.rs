use super::*;

pub(super) fn snapshot_shared_attribute(
    managers: &BattleManagers,
    command: BattleCommand,
) -> BattleCommand {
    let BattleCommand::Buff(BuffCommand::Grant(grant)) = command else {
        return command;
    };
    let buff_command =
        crate::engine::skill::buff_act::each_change_attr::shared_critical_grant(managers, grant)
            .map_or(BuffCommand::Grant(grant), BuffCommand::GrantSharedAttribute);
    BattleCommand::Buff(buff_command)
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

pub(super) fn stacks_as_copies(command: BuffCommand) -> BattleCommand {
    let (BuffCommand::Grant(grant) | BuffCommand::GrantOnDeath(grant)) = command else {
        return BattleCommand::Buff(command);
    };
    let Some(stacks) = grant.amount.filter(|stacks| *stacks > 1) else {
        return BattleCommand::Buff(command);
    };
    if !crate::engine::manager::buff::grants_stacks_as_copies(grant.buff_id) {
        return BattleCommand::Buff(command);
    }
    let single = BuffGrant {
        amount: None,
        ..grant
    };
    let first = match command {
        BuffCommand::GrantOnDeath(_) => BuffCommand::GrantOnDeath(single),
        _ => BuffCommand::Grant(single),
    };
    BattleCommand::BuffBatch(
        std::iter::once(first)
            .chain((1..stacks).map(|_| BuffCommand::GrantUsingChildUid(single)))
            .collect(),
    )
}

#[derive(Clone, Copy)]
pub(super) struct PoolEntry {
    pub(super) id: i32,
    pub(super) stacks: u32,
}

enum RandomPool {
    Ordinary(Vec<PoolEntry>),
    Arsenal(Vec<super::arsenal::Bullet>),
}

fn pool_entries(catalog: crate::catalog::BattleCatalog, raw: &str) -> Option<Vec<PoolEntry>> {
    raw.split('#')
        .map(|entry| {
            let (id, stacks) = entry.split_once(',').unwrap_or((entry, "1"));
            let id = id.trim().parse().ok().filter(|id| *id > 0)?;
            catalog.game_data().skill_buff.get(id)?;
            let stacks = stacks.trim().parse().ok().filter(|stacks| *stacks > 0)?;
            Some(PoolEntry { id, stacks })
        })
        .collect()
}

fn random_pool(
    catalog: crate::catalog::BattleCatalog,
    behavior: &ParsedBehavior,
) -> Option<(usize, RandomPool)> {
    let [pool_id, count] = behavior.args.as_slice() else {
        return None;
    };
    let count = usize::try_from(*count).ok().filter(|count| *count > 0)?;
    let rows = catalog.buff_feature_rows(*pool_id);
    let [raw] = rows.as_slice() else {
        return None;
    };
    let entries = pool_entries(catalog, raw)?;
    let count_from_pool = count.min(entries.len());
    let pool = match super::registry::find(behavior)?.kind {
        BehaviorKind::AddBuffRanId if count <= entries.len() => RandomPool::Ordinary(entries),
        BehaviorKind::AddBuffRanTypeId => match super::arsenal::select(catalog, &entries).ok()? {
            Some(bullets) => RandomPool::Arsenal(bullets),
            None => RandomPool::Ordinary(entries),
        },
        _ => return None,
    };
    Some((count_from_pool, pool))
}

pub(super) fn random_pool_grant_commands(
    context: &mut BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let definition = super::registry::find(behavior)?;
    let (count, pool) = random_pool(context.managers.catalog(), behavior)?;
    let entries = match pool {
        RandomPool::Ordinary(entries) => entries,
        RandomPool::Arsenal(bullets) => {
            return super::arsenal::grants(context, behavior, count, bullets);
        }
    };
    let mut candidates = entries
        .into_iter()
        .map(|entry| entry.id)
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
    let count = count.min(candidates.len());
    let mut observed = context
        .determinism
        .take_random_buff_pool(
            crate::engine::skill::rule::ConfiguredRuleKey::new(
                behavior.effect_id,
                behavior.slot,
                definition.key,
            ),
            context.source_uid,
            context.target_uid,
            count,
            &candidates,
        )
        .unwrap_or_default()
        .into_iter();
    for _ in 0..count {
        let index = observed
            .next()
            .or_else(|| context.determinism.take_random_buff(&candidates))
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
    crate::catalog::BattleCatalog::try_global()
        .is_some_and(|catalog| random_pool(catalog, behavior).is_some())
}

pub fn random_buff_pool(behavior: &ParsedBehavior) -> Option<Vec<i32>> {
    let (_, pool) = random_pool(crate::catalog::BattleCatalog::try_global()?, behavior)?;
    Some(match pool {
        RandomPool::Ordinary(entries) => entries.into_iter().map(|entry| entry.id).collect(),
        RandomPool::Arsenal(bullets) => bullets.into_iter().map(|bullet| bullet.entry.id).collect(),
    })
}

pub fn random_arsenal_pool(behavior: &ParsedBehavior) -> Option<Vec<(i32, u32)>> {
    let (_, pool) = random_pool(crate::catalog::BattleCatalog::try_global()?, behavior)?;
    let RandomPool::Arsenal(bullets) = pool else {
        return None;
    };
    Some(
        bullets
            .into_iter()
            .map(|bullet| (bullet.entry.id, bullet.entry.stacks))
            .collect(),
    )
}

#[cfg(test)]
mod tests;
