use super::*;
use crate::engine::{event::kind::EventKind, manager::buff::BuffDurationAdvance};

pub fn run_event(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    event: BattleEvent,
) -> Result<DrainResult, DrainError> {
    let current_pool = subscriber_view(pool, managers, &event);
    let mut initial = dispatch_reactions(
        &current_pool,
        managers,
        catalog,
        determinism,
        ReactionDispatch {
            execute_unscoped_after_action: context.current_round > 0,
            ..ReactionDispatch::for_event(&event)
        },
        None,
    )?;
    initial.assign_subscriber_death_groups(None);
    let initial = initial.into_ordered();
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
    run_group_event_lanes(
        managers,
        pool,
        catalog,
        determinism,
        context,
        event,
        &[lane],
        owner_uids,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_round_start_card_event(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    owner_uids: &[i64],
) -> Result<DrainResult, DrainError> {
    let event = BattleEvent::Kind(crate::engine::event::kind::EventKind::RoundStartCard);
    let advances = crate::engine::skill::buff_act::effect_time::ROUND_START_CARD_STAGES
        .into_iter()
        .filter_map(|take_stage| {
            super::round_start_card::duration_snapshot(managers, owner_uids, take_stage)
        })
        .collect::<Vec<_>>();
    let mut result = run_group_event_lanes(
        managers,
        pool,
        catalog,
        determinism,
        context,
        event,
        &[ReactionLane::BuffActs, ReactionLane::Skills],
        Some(owner_uids),
    )?;
    let duration =
        run_round_start_card_duration(managers, pool, catalog, determinism, context, advances)?;
    result.outcomes.extend(duration.outcomes);
    result.events.extend(duration.events);
    result.frames.extend(duration.frames);
    Ok(result)
}

pub(crate) fn run_round_start_card_duration(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    advances: impl IntoIterator<Item = BuffDurationAdvance>,
) -> Result<DrainResult, DrainError> {
    let event = BattleEvent::Kind(crate::engine::event::kind::EventKind::RoundStartCard);
    let mut result = DrainResult::default();
    for advance in advances {
        let mut frames = Vec::new();
        let scope = push_root(
            &mut frames,
            FrameOwner::EventRule,
            FrameTrigger::Event(event.clone()),
        );
        let mut queue = VecDeque::from([QueuedOp {
            op: RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Buff(
                crate::engine::manager::buff::BuffCommand::AdvanceDuration(advance),
            )),
            trigger: SkillOpTrigger::Event(event.clone()),
            skill_execution: None,
            frame_path: Some(scope),
            parent_path: None,
            frame_group: None,
            independent_parent_group: None,
            frame_owner: None,
            subscriber: None,
            caster_frame: None,
        }]);
        let settled = drain_queue_with_frames(
            managers,
            pool,
            catalog,
            determinism,
            context,
            &mut queue,
            frames,
        )?;
        result.outcomes.extend(settled.outcomes);
        result.events.extend(settled.events);
        result.frames.extend(settled.frames);
    }
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
pub fn run_group_event_lanes(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    event: BattleEvent,
    lanes: &[ReactionLane],
    owner_uids: Option<&[i64]>,
) -> Result<DrainResult, DrainError> {
    drain_group_event(
        managers,
        pool,
        catalog,
        determinism,
        context,
        event,
        lanes,
        owner_uids,
        DefenderCardCleanupOwner::DeathAction,
    )
}

pub(in crate::engine::runtime) fn run_attacker_round_end_skills(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    owner_uids: &[i64],
) -> Result<DrainResult, DrainError> {
    drain_group_event(
        managers,
        pool,
        catalog,
        determinism,
        context,
        BattleEvent::Kind(EventKind::RoundEnd),
        &[ReactionLane::Skills],
        Some(owner_uids),
        DefenderCardCleanupOwner::ScheduledRoundPhase,
    )
}

#[allow(clippy::too_many_arguments)]
fn drain_group_event(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    event: BattleEvent,
    lanes: &[ReactionLane],
    owner_uids: Option<&[i64]>,
    cleanup_owner: DefenderCardCleanupOwner,
) -> Result<DrainResult, DrainError> {
    let mut frames = Vec::new();
    let root = push_root(&mut frames, FrameOwner::Command, FrameTrigger::Active);
    let mut result = DrainResult {
        frames,
        ..Default::default()
    };
    for &lane in lanes {
        if lane == ReactionLane::Skills
            && let Some(owner_uids) = owner_uids
        {
            for owner_uid in owner_uids {
                let scope = push_child(
                    &mut result.frames,
                    &root,
                    FrameOwner::EventRule,
                    FrameTrigger::Event(event.clone()),
                );
                let next = drain_group_event_lane(
                    managers,
                    pool,
                    catalog,
                    determinism,
                    context,
                    &event,
                    lane,
                    Some(std::slice::from_ref(owner_uid)),
                    &scope,
                    std::mem::take(&mut result.frames),
                    cleanup_owner,
                )?;
                result.outcomes.extend(next.outcomes);
                result.events.extend(next.events);
                result.frames = next.frames;
            }
            continue;
        }
        let scope = match lane {
            ReactionLane::Skills => push_child(
                &mut result.frames,
                &root,
                FrameOwner::EventRule,
                FrameTrigger::Event(event.clone()),
            ),
            ReactionLane::BuffActs
            | ReactionLane::BuffActsBeforeSettlement
            | ReactionLane::BuffActsAfterSettlement => root.clone(),
        };
        let next = drain_group_event_lane(
            managers,
            pool,
            catalog,
            determinism,
            context,
            &event,
            lane,
            owner_uids,
            &scope,
            std::mem::take(&mut result.frames),
            cleanup_owner,
        )?;
        result.outcomes.extend(next.outcomes);
        result.events.extend(next.events);
        result.frames = next.frames;
    }
    result.events.insert(0, event);

    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn drain_group_event_lane(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    event: &BattleEvent,
    lane: ReactionLane,
    owner_uids: Option<&[i64]>,
    scope: &FramePath,
    frames: Vec<SemanticFrame>,
    cleanup_owner: DefenderCardCleanupOwner,
) -> Result<DrainResult, DrainError> {
    let current_pool = subscriber_view(pool, managers, event);
    let mut reactions = dispatch_reactions(
        &current_pool,
        managers,
        catalog,
        determinism,
        ReactionDispatch {
            parent_path: Some(scope),
            lane: Some(lane),
            owner_uids,
            execute_unscoped_after_action: context.current_round > 0,
            ..ReactionDispatch::for_event(event)
        },
        None,
    )?;
    reactions.assign_subscriber_death_groups(None);
    let mut queue = reactions.into_ordered().into();
    if matches!(event, BattleEvent::RoundStart) && lane == ReactionLane::BuffActsBeforeSettlement {
        return super::super::round_start_loss::run(
            managers,
            pool,
            catalog,
            determinism,
            context,
            &mut queue,
            frames,
        );
    }
    drain_queue_with_frames_and_cleanup_owner(
        managers,
        pool,
        catalog,
        determinism,
        context,
        &mut queue,
        frames,
        cleanup_owner,
    )
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
    run_grouped_owner_event_lanes(
        managers,
        pool,
        catalog,
        determinism,
        context,
        event,
        owner_uids,
        &[lane],
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_grouped_owner_event_lanes(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    event: BattleEvent,
    owner_uids: &[i64],
    lanes: &[ReactionLane],
) -> Result<DrainResult, DrainError> {
    let mut result = DrainResult::default();
    let root = push_root(
        &mut result.frames,
        FrameOwner::Command,
        FrameTrigger::Active,
    );
    run_grouped_owner_event_at(
        managers,
        pool,
        catalog,
        determinism,
        context,
        event,
        owner_uids,
        lanes,
        result,
        root,
    )
}

pub(in crate::engine::runtime) fn run_settlement_tail(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    owner_uids: &[i64],
    operations: impl IntoIterator<Item = (FrameOwner, RuleOp)>,
) -> Result<DrainResult, DrainError> {
    let (result, root) = super::schedule::run_owned_command_group_as(
        managers,
        pool,
        catalog,
        determinism,
        context,
        FrameOwner::RoundPhase(RoundPhase::AfterOwnerSettlement),
        operations,
    )?;
    run_grouped_owner_event_at(
        managers,
        pool,
        catalog,
        determinism,
        context,
        BattleEvent::Kind(crate::engine::event::kind::EventKind::RoundEndAfterOwnerSettlement),
        owner_uids,
        &[ReactionLane::Skills],
        result,
        root,
    )
}

#[allow(clippy::too_many_arguments)]
fn run_grouped_owner_event_at(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    event: BattleEvent,
    owner_uids: &[i64],
    lanes: &[ReactionLane],
    mut result: DrainResult,
    root: FramePath,
) -> Result<DrainResult, DrainError> {
    let event_index = result.events.len();
    managers.buff.begin_transaction();
    let result = (|| {
        for &lane in lanes {
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
                let mut reactions = dispatch_reactions(
                    &current_pool,
                    managers,
                    catalog,
                    determinism,
                    ReactionDispatch {
                        parent_path: Some(&scope),
                        lane: Some(lane),
                        owner_uids: Some(std::slice::from_ref(owner_uid)),
                        execute_unscoped_after_action: context.current_round > 0,
                        ..ReactionDispatch::for_event(&event)
                    },
                    None,
                )?;
                reactions.assign_subscriber_death_groups(None);
                let mut queue: VecDeque<_> = reactions.into_ordered().into();
                if lane == ReactionLane::Skills && event.kind() == EventKind::SmallRoundEnd {
                    for operation in &mut queue {
                        if let Some(subscriber) = &mut operation.subscriber {
                            subscriber.settlement_owner = Some(scope.clone());
                        }
                    }
                }
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
        }
        result.events.insert(event_index, event);
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
