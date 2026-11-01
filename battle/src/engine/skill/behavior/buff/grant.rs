use super::*;

// This enemy's mark designates the last acting opponent. Move the mark from
// its previous holder before granting it to the current actor.
pub(super) fn fifth_chapter_last_actor_mark_ops(
    context: &BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    if behavior.effect_id != 1141011 || behavior.arg(0) != Some(11410111) {
        return None;
    }
    let origin = super::command_origin(behavior)?;
    let mut ops = context.pool.enemies(context.source_uid, true).iter()
        .filter(|enemy| enemy.uid != context.target_uid)
        .flat_map(|enemy| context.managers.buff.active_for(enemy.uid).filter_map(move |buff| {
            (buff.buff_id == Some(11410111) && buff.from_uid == Some(context.source_uid))
                .then(|| buff.uid.map(|uid| (enemy.uid, uid))).flatten()
        }))
        .map(|(target_uid, uid)| RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(
            BuffRemove { origin, target_uid, selector: BuffRemoveSelector::Uid(uid) },
        ))))
        .collect::<Vec<_>>();
    ops.push(RuleOp::Command(BattleCommand::Buff(grant_command(
        context.source_uid, context.target_uid, context.transfer_count.max(0) as u32,
        context.event, behavior,
    )?)));
    Some(ops)
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
        // AddBuffRanTypeId prioritizes types the target does not already have,
        // then refreshes existing types if the requested count is larger.
        let missing = if definition.kind == BehaviorKind::AddBuffRanTypeId {
            candidates.iter().copied().filter(|buff_id| {
                let type_id = context.managers.catalog().buff_type_id(*buff_id);
                let key = if type_id > 0 { type_id } else { *buff_id };
                !context.managers.buff.has_active_buff_id_or_type(context.target_uid, key)
            }).collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let available = if missing.is_empty() { &candidates } else { &missing };
        let chosen = context
            .determinism
            .take_random_buff(available)
            .or_else(|| context.determinism.lua_random_index(available.len()).map(|index| available[index]))?;
        let index = candidates.iter().position(|buff_id| *buff_id == chosen)?;
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
