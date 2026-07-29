use super::*;

pub fn run_event(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    event: BattleEvent,
) -> Result<DrainResult, DrainError> {
    let current_pool = subscriber_view(pool, managers, &event);
    let initial = dispatch_reactions(
        &current_pool,
        managers,
        catalog,
        determinism,
        &event,
        None,
        None,
        None,
        None,
        None,
        None,
        context.current_round > 0,
        None,
    )?
    .into_ordered();
    run_event_queue(
        managers,
        pool,
        catalog,
        determinism,
        context,
        event,
        initial,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_group_event(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    event: BattleEvent,
    lane: ReactionLane,
    owner_uids: Option<&[i64]>,
) -> Result<DrainResult, DrainError> {
    let mut frames = Vec::new();
    let root = push_root(&mut frames, FrameOwner::Command, FrameTrigger::Active);
    let scope = match lane {
        ReactionLane::Skills => push_child(
            &mut frames,
            &root,
            FrameOwner::EventRule,
            FrameTrigger::Event(event.clone()),
        ),
        ReactionLane::BuffActs
        | ReactionLane::BuffActsBeforeSettlement
        | ReactionLane::BuffActsAfterSettlement => root,
    };
    let current_pool = subscriber_view(pool, managers, &event);
    let mut queue = dispatch_reactions(
        &current_pool,
        managers,
        catalog,
        determinism,
        &event,
        Some(&scope),
        None,
        None,
        None,
        Some(lane),
        owner_uids,
        context.current_round > 0,
        None,
    )?
    .into_ordered()
    .into();
    let mut result = drain_queue_with_frames(
        managers,
        pool,
        catalog,
        determinism,
        context,
        &mut queue,
        frames,
    )?;
    result.events.insert(0, event);

    Ok(result)
}

#[allow(clippy::too_many_arguments)]
pub fn run_grouped_owner_event(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    event: BattleEvent,
    owner_uids: &[i64],
    lane: ReactionLane,
) -> Result<DrainResult, DrainError> {
    managers.buff.begin_transaction();
    let result = (|| {
        let mut result = DrainResult::default();
        let root = push_root(
            &mut result.frames,
            FrameOwner::Command,
            FrameTrigger::Active,
        );
        for owner_uid in owner_uids {
            let scope = match lane {
                ReactionLane::Skills => push_child(
                    &mut result.frames,
                    &root,
                    FrameOwner::EventRule,
                    FrameTrigger::Event(event.clone()),
                ),
                _ => root.clone(),
            };
            let current_pool = subscriber_view(pool, managers, &event);
            let mut queue = dispatch_reactions(
                &current_pool,
                managers,
                catalog,
                determinism,
                &event,
                Some(&scope),
                None,
                None,
                None,
                Some(lane),
                Some(std::slice::from_ref(owner_uid)),
                context.current_round > 0,
                None,
            )?
            .into_ordered()
            .into();
            let next = drain_queue_with_frames(
                managers,
                pool,
                catalog,
                determinism,
                context,
                &mut queue,
                std::mem::take(&mut result.frames),
            )?;
            result.outcomes.extend(next.outcomes);
            result.events.extend(next.events);
            result.frames = next.frames;
        }
        result.events.insert(0, event);
        Ok(result)
    })();
    managers.buff.end_transaction();
    result
}

pub fn run_owner_event(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    event: BattleEvent,
    owner_uids: &[i64],
) -> Result<DrainResult, DrainError> {
    let current_pool = subscriber_view(pool, managers, &event);
    let initial = dispatch_owner_reactions(
        &current_pool,
        managers,
        catalog,
        determinism,
        &event,
        owner_uids,
    )?;
    run_event_queue(
        managers,
        pool,
        catalog,
        determinism,
        context,
        event,
        initial,
    )
}

fn subscriber_view(
    pool: &TargetPool,
    managers: &BattleManagers,
    event: &BattleEvent,
) -> TargetPool {
    match event {
        BattleEvent::EntityDied(death) => {
            pool.runtime_view_including(managers, Some(death.target_uid))
        }
        _ => pool.runtime_view(managers),
    }
}

fn run_event_queue(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    event: BattleEvent,
    initial: Vec<QueuedOp>,
) -> Result<DrainResult, DrainError> {
    let mut queue = initial.into();
    let mut result = drain_queue(managers, pool, catalog, determinism, context, &mut queue)?;
    result.events.insert(0, event);
    Ok(result)
}
