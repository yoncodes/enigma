use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    rc::Rc,
};

use crate::engine::{
    event::{bus::EventBus, dispatcher, payload::BattleEvent},
    manager::BattleManagers,
    runtime::{
        determinism::RoundDeterminism,
        executor::{RuleExecutionError, RuleOutcome, execute_rule_op_with},
        record::{
            FrameOwner, FramePath, FrameTrigger, RoundCue, RoundPhase, SemanticFrame, SetupSide,
            active_skill_scope_path, active_skill_target_uid, entity_entry_halo_scope_path,
            event_scope_path, owner_at_path, push_change, push_child, push_cue, push_frame_child,
            push_root, set_skill_target,
        },
        skill::{self, SkillExecution, SkillOpError, SkillOpTrigger},
    },
    skill::{
        action::{SkillPhase, SkillTarget},
        effect::SkillEffectCatalog,
        rule::{SetupStage, output::RuleOp},
        subscriber::SubscriberError,
        target::{TargetContext, TargetPool, TargetRequest, TargetResolver, targets_enemy},
    },
};

mod command_dispatch;
mod command_prepare;
mod entry;
mod model;
mod reactions;
mod skill_dispatch;
mod state;

use command_dispatch::drain_command;
pub use entry::*;
pub(super) use model::attack_has_no_target;
use model::{
    DefenderCardCleanupOwner, DrainBudget, MAX_DRAIN_DEPTH, MAX_DRAIN_OPERATIONS, QueuedOp,
    ReactionBatch, SubscriberContext, SubscriberDeathGroup, assign_subscriber_death_groups,
    queued_death_event_group_completion, queued_death_event_group_settlement,
    queued_death_settlement, queued_defeated_entity_halo_cleanup,
    queued_defeated_owner_card_cleanup, queued_terminal_settlement, required_buff_act_definition,
};
pub use model::{DrainError, DrainResult, ReactionLane};
#[cfg(test)]
use reactions::{
    dispatch_event_batch, ordered_hit_entities, queued_reactions, reaction_counterparty,
    reaction_skill_target,
};
use reactions::{dispatch_event_groups, dispatch_owner_reactions, dispatch_reactions};
use skill_dispatch::drain_skill;
use state::DrainState;

fn drain_queue(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    queue: &mut VecDeque<QueuedOp>,
) -> Result<DrainResult, DrainError> {
    drain_queue_with_frames(
        managers,
        pool,
        catalog,
        determinism,
        context,
        queue,
        Vec::new(),
    )
}

fn drain_queue_with_frames(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    queue: &mut VecDeque<QueuedOp>,
    frames: Vec<SemanticFrame>,
) -> Result<DrainResult, DrainError> {
    drain_queue_with_frames_and_cleanup_owner(
        managers,
        pool,
        catalog,
        determinism,
        context,
        queue,
        frames,
        DefenderCardCleanupOwner::DeathAction,
    )
}

#[allow(clippy::too_many_arguments)]
fn drain_queue_with_frames_and_cleanup_owner(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    queue: &mut VecDeque<QueuedOp>,
    frames: Vec<SemanticFrame>,
    cleanup_owner: DefenderCardCleanupOwner,
) -> Result<DrainResult, DrainError> {
    let mut state = DrainState::new(context).with_defender_card_cleanup_owner(cleanup_owner);
    drain_queue_with_deferred(
        managers,
        pool,
        catalog,
        determinism,
        queue,
        frames,
        &mut state,
    )
}

fn grouped_hp_events(
    events: &[BattleEvent],
    batch_events: Vec<Vec<BattleEvent>>,
) -> Vec<Vec<BattleEvent>> {
    if batch_events.iter().flatten().eq(events.iter()) {
        batch_events
    } else {
        vec![events.to_vec()]
    }
}

fn split_active_hit_event_groups(
    event_groups: Vec<Vec<BattleEvent>>,
) -> (Vec<Vec<BattleEvent>>, Vec<Vec<BattleEvent>>) {
    let mut immediate_groups = Vec::new();
    let mut deferred_groups = Vec::new();
    for events in event_groups {
        let defers_hp_loss = events.iter().any(|event| {
            matches!(
                event,
                BattleEvent::Hit(hit)
                    if matches!(
                        hit.damage_from,
                        crate::engine::manager::hp::HurtDamageFromType::Skill
                            | crate::engine::manager::hp::HurtDamageFromType::Additional
                    )
            )
        });
        let (deferred, immediate): (Vec<_>, Vec<_>) = events.into_iter().partition(|event| {
            matches!(event, BattleEvent::Hit(_))
                || defers_hp_loss
                    && matches!(
                        event,
                        BattleEvent::HpLost { .. } | BattleEvent::DamageShared { .. }
                    )
        });
        if !immediate.is_empty() {
            immediate_groups.push(immediate);
        }
        if !deferred.is_empty() {
            deferred_groups.push(deferred);
        }
    }
    (immediate_groups, deferred_groups)
}

fn flatten_event_groups(event_groups: &[Vec<BattleEvent>]) -> Vec<BattleEvent> {
    event_groups
        .iter()
        .flat_map(|events| events.iter().cloned())
        .collect()
}

fn reaction_already_fired(
    state: &mut DrainState,
    invocation: &crate::engine::skill::action::SkillInvocation,
) -> bool {
    let Some(condition_key) = invocation.condition_key else {
        return false;
    };
    let Some(definition) = crate::engine::skill::condition::registry::find_key(
        condition_key.opcode,
        condition_key.type_name,
    ) else {
        return false;
    };
    let key = (
        invocation.plan.source_uid,
        invocation.plan.skill_id,
        invocation.condition_slot,
        condition_key,
    );
    match definition.reaction_multiplicity {
        crate::engine::skill::condition::registry::ReactionMultiplicity::PerEvent => false,
        crate::engine::skill::condition::registry::ReactionMultiplicity::OncePerEventBatch => {
            invocation
                .reaction_batch_id
                .is_some_and(|batch_id| state.event_batch_reactions(batch_id).contains(&key))
        }
        crate::engine::skill::condition::registry::ReactionMultiplicity::OncePerAction => {
            invocation
                .reaction_action_path
                .as_deref()
                .is_some_and(|path| state.action_reactions(path).contains(&key))
        }
    }
}

fn mark_reaction_fired(
    state: &mut DrainState,
    invocation: &crate::engine::skill::action::SkillInvocation,
    slot_index: usize,
    condition_key: crate::engine::skill::rule::DefinitionKey,
) {
    let Some(definition) = crate::engine::skill::condition::registry::find_key(
        condition_key.opcode,
        condition_key.type_name,
    ) else {
        return;
    };
    let key = (
        invocation.plan.source_uid,
        invocation.plan.skill_id,
        Some(slot_index),
        condition_key,
    );
    match definition.reaction_multiplicity {
        crate::engine::skill::condition::registry::ReactionMultiplicity::PerEvent => {}
        crate::engine::skill::condition::registry::ReactionMultiplicity::OncePerEventBatch => {
            if let Some(batch_id) = invocation.reaction_batch_id {
                state.event_batch_reactions(batch_id).insert(key);
            }
        }
        crate::engine::skill::condition::registry::ReactionMultiplicity::OncePerAction => {
            if let Some(path) = invocation.reaction_action_path.as_deref() {
                state.action_reactions(path).insert(key);
            }
        }
    }
}

/// Drains queued operations and registered reactions into semantic frames using declared phase and lane order.
/// It follows declared phases; it does not repair ordering or packet shape.
fn drain_queue_with_deferred(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    queue: &mut VecDeque<QueuedOp>,
    frames: Vec<SemanticFrame>,
    state: &mut DrainState,
) -> Result<DrainResult, DrainError> {
    let context = state.context();
    let mut bus = EventBus::default();
    let mut pending_hits = HashMap::<FramePath, Vec<Vec<BattleEvent>>>::new();
    let mut result = DrainResult {
        outcomes: Vec::new(),
        events: Vec::new(),
        frames,
    };
    let base_pool = pool;

    // Eligible skills expand into queued operations. Non-skill operations reach
    // manager commit only after observers at the preceding boundary are released.
    while let Some(QueuedOp {
        op,
        trigger,
        skill_execution,
        frame_path,
        parent_path,
        frame_group,
        independent_parent_group,
        frame_owner,
        subscriber,
        caster_frame,
    }) = queue.pop_front()
    {
        // Root and nested drains share this budget, so reaction cycles fail the
        // battle instead of overflowing the server stack or running forever.
        let depth = state.depth();
        if let Err(error) = state.consume_budget() {
            tracing::error!(
                ?error,
                ?op,
                ?frame_owner,
                depth,
                "battle drain budget exhausted"
            );
            if crate::engine::diagnostics::enabled(crate::engine::diagnostics::TraceArea::Drain) {
                eprintln!(
                    "battle drain budget exhausted: error={error:?} op={op:?} frame_owner={frame_owner:?} depth={depth}"
                );
            }
            return Err(error);
        }
        if crate::engine::diagnostics::enabled(crate::engine::diagnostics::TraceArea::Drain) {
            let summary = format!("{op:?}").chars().take(240).collect::<String>();
            let owner_uid = subscriber.as_ref().map(|subscriber| subscriber.owner_uid);
            eprintln!("drain depth={depth} owner={owner_uid:?} frame={frame_owner:?} op={summary}");
        }

        // Previous operations may have changed HP, buffs, entities, or resources.
        // Target resolution must see those committed manager values.
        let continuing_source = match (&op, &trigger, frame_path.as_ref()) {
            (RuleOp::Skill(invocation), SkillOpTrigger::Event(_), _)
            | (RuleOp::Skill(invocation), SkillOpTrigger::Active, Some(_)) => {
                Some(invocation.plan.source_uid)
            }
            _ => None,
        };
        let current_pool = base_pool.runtime_view_including(managers, continuing_source);
        let pool = &current_pool;

        // Frame groups keep independent reactions under one semantic owner while
        // ordinary children inherit the parent selected by their emitter.
        let frame_path = frame_path.or_else(|| {
            frame_group
                .as_ref()
                .and_then(|group| group.borrow().clone())
        });
        let frame_group_started = frame_group
            .as_ref()
            .is_some_and(|group| group.borrow().is_some());
        let reaction_link = frame_group
            .as_ref()
            .and_then(|group| state.action_reaction_link(group))
            .or_else(|| state.reaction_child_link(frame_path.as_ref()))
            .or_else(|| state.reaction_child_link(parent_path.as_ref()));
        if !matches!(op, RuleOp::SubscriberGroupComplete)
            && matches!(trigger, SkillOpTrigger::Event(_))
            && !frame_group_started
            && managers
                .terminal_outcome()
                .and_then(|outcome| outcome.winning_team())
                .is_some_and(|winning_team| {
                    subscriber
                        .as_ref()
                        .and_then(|subscriber| base_pool.team_type(subscriber.owner_uid))
                        .is_some_and(|team| team != winning_team)
                })
        {
            continue;
        }
        let parent_path = if let Some(group) = &independent_parent_group {
            let existing = group.borrow().clone();
            let path = existing.unwrap_or_else(|| {
                let root = push_root(
                    &mut result.frames,
                    if matches!(frame_owner, Some(FrameOwner::Skill { .. })) {
                        FrameOwner::Command
                    } else {
                        FrameOwner::EventRule
                    },
                    frame_trigger(&trigger),
                );
                let path = if matches!(frame_owner, Some(FrameOwner::Skill { .. })) {
                    push_child(
                        &mut result.frames,
                        &root,
                        FrameOwner::EventRule,
                        frame_trigger(&trigger),
                    )
                } else {
                    root
                };
                *group.borrow_mut() = Some(path.clone());
                path
            });
            Some(path)
        } else {
            parent_path
        };
        match op {
            RuleOp::ImmediateReservePromotion => {
                let parent_path = frame_path
                    .as_deref()
                    .or(parent_path.as_deref())
                    .expect("action-boundary promotion keeps its action frame");
                let promotions = managers.promote_immediate_reserves();
                if promotions.is_empty() {
                    continue;
                }
                let mut promoted = crate::engine::runtime::schedule::run_promotions(
                    base_pool,
                    managers,
                    catalog,
                    determinism,
                    context,
                    promotions,
                )?;
                for frame in promoted.frames.drain(..) {
                    push_frame_child(&mut result.frames, parent_path, frame);
                }
                result.outcomes.append(&mut promoted.outcomes);
                result.events.append(&mut promoted.events);
            }
            RuleOp::DeathEventGroupComplete(death) => {
                state.complete_death_event_group(death);
                if managers.hp.is_current_death(death) {
                    let transition = death.transition();
                    let parent_path = parent_path
                        .as_deref()
                        .or(frame_path.as_deref())
                        .expect("death event group completion keeps its parent frame");
                    let mut settlements =
                        queued_defeated_entity_halo_cleanup(managers, transition, parent_path)
                            .into_iter()
                            .collect::<Vec<_>>();
                    settlements.push(queued_death_event_group_settlement(transition, parent_path));
                    prepend(queue, settlements);
                }
            }
            RuleOp::SubscriberGroupComplete => {
                let group = subscriber
                    .as_ref()
                    .and_then(|subscriber| subscriber.group.as_ref())
                    .expect("subscriber group completion keeps its group identity");
                let deaths = state
                    .take_subscriber_deaths(group)
                    .into_iter()
                    .filter(|(death, _)| managers.hp.is_current_death(*death))
                    .map(|(death, parent_path)| (death.transition(), parent_path))
                    .collect::<Vec<_>>();
                let mut settlements = Vec::new();
                for (death, parent_path) in &deaths {
                    if let Some(cleanup) =
                        queued_defeated_entity_halo_cleanup(managers, *death, parent_path)
                    {
                        settlements.push(cleanup);
                    }
                    settlements.push(queued_death_settlement(*death, parent_path));
                }
                if let Some((death, parent_path)) = deaths.first() {
                    settlements.push(queued_terminal_settlement(
                        BattleEvent::EntityDied(crate::engine::event::payload::EntityDiedEvent {
                            source_uid: death.source_uid,
                            target_uid: death.target_uid,
                        }),
                        parent_path,
                    ));
                }
                prepend(queue, settlements);
            }
            RuleOp::TerminalSettlement => {
                commit_terminal(managers, base_pool, context, state, &mut result.events);
            }
            RuleOp::Skill(invocation) => drain_skill(
                managers,
                base_pool,
                pool,
                catalog,
                determinism,
                context,
                queue,
                &mut result,
                state,
                invocation,
                trigger,
                skill_execution,
                frame_path,
                parent_path,
                frame_group,
                frame_owner,
                subscriber,
                caster_frame,
                reaction_link,
            )?,
            RuleOp::FreezeActiveSkillRates => {
                let frame_path = frame_path.ok_or(DrainError::MissingActiveSkillContext)?;
                let execution = queue
                    .iter_mut()
                    .find_map(|queued| {
                        (matches!(queued.trigger, SkillOpTrigger::Active)
                            && queued.frame_path.as_ref() == Some(&frame_path)
                            && matches!(queued.op, RuleOp::Skill(_)))
                        .then_some(queued.skill_execution.as_mut())
                        .flatten()
                    })
                    .ok_or(DrainError::MissingActiveSkillContext)?;
                execution.freeze_rate_amounts(&managers.gauge);
            }
            command @ (RuleOp::Command(_)
            | RuleOp::Publish(_)
            | RuleOp::SkillLifecycle(_)
            | RuleOp::ActionHandSettlement { .. }
            | RuleOp::DeathSettlement(_)
            | RuleOp::BeginSkillAction { .. }
            | RuleOp::BuffFeatureMarker { .. }
            | RuleOp::EffectMarker { .. }
            | RuleOp::SceneChange { .. }
            | RuleOp::BuffActTrigger(_)
            | RuleOp::BuffActInfoMarker(_)
            | RuleOp::MarkBuffActFired { .. }
            | RuleOp::ModifyActiveSkillTargets { .. }
            | RuleOp::NuoDiKaHit(_)) => drain_command(
                managers,
                base_pool,
                pool,
                catalog,
                determinism,
                context,
                queue,
                &mut result,
                state,
                &mut bus,
                &mut pending_hits,
                command,
                trigger,
                skill_execution,
                frame_path,
                parent_path,
                frame_group,
                independent_parent_group,
                frame_owner,
                subscriber,
                caster_frame,
            )?,
        }
    }

    Ok(result)
}

fn release_invocation_progress(
    managers: &mut BattleManagers,
    invocation: &crate::engine::skill::action::SkillInvocation,
) {
    if invocation.phase.is_none()
        && let Some(key) = invocation.release_progress
    {
        managers.release_held_rule_progress(invocation.plan.source_uid, key);
    }
}

fn cancel_invocation_progress(
    managers: &mut BattleManagers,
    state: &mut DrainState,
    action_path: Option<&FramePath>,
    invocation: &crate::engine::skill::action::SkillInvocation,
) {
    release_invocation_progress(managers, invocation);
    let Some(action_path) = action_path else {
        return;
    };
    for queued in state.cancel_action(action_path) {
        if let RuleOp::Skill(invocation) = queued.op {
            release_invocation_progress(managers, &invocation);
        }
    }
}

fn drain_nested_queue(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    mut queue: VecDeque<QueuedOp>,
    result: &mut DrainResult,
    state: &mut DrainState,
) -> Result<(), DrainError> {
    state.enter_nested();
    let nested = drain_queue_with_deferred(
        managers,
        pool,
        catalog,
        determinism,
        &mut queue,
        std::mem::take(&mut result.frames),
        state,
    );
    state.leave_nested();
    let nested = nested?;
    result.outcomes.extend(nested.outcomes);
    result.events.extend(nested.events);
    result.frames = nested.frames;
    Ok(())
}

fn drain_death_settlement_queue(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    queue: VecDeque<QueuedOp>,
    result: &mut DrainResult,
    state: &mut DrainState,
) -> Result<(), DrainError> {
    state.enter_death_settlement();
    let nested = drain_nested_queue(managers, pool, catalog, determinism, queue, result, state);
    state.leave_death_settlement();
    nested
}

fn drain_defeated_entity_halo_cleanup(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    cleanup: Option<QueuedOp>,
    result: &mut DrainResult,
    state: &mut DrainState,
) -> Result<(), DrainError> {
    let Some(cleanup) = cleanup else {
        return Ok(());
    };
    drain_nested_queue(
        managers,
        pool,
        catalog,
        determinism,
        VecDeque::from([cleanup]),
        result,
        state,
    )
}

fn commit_terminal(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    context: TargetContext,
    state: &DrainState,
    events: &mut Vec<BattleEvent>,
) {
    if !state.death_settlement_in_progress()
        && managers.terminal_outcome().is_none()
        && context.battle_id > 0
        && let Some(outcome) = crate::engine::round::outcome::terminal_outcome_for_battle_id(
            context.battle_id,
            pool,
            managers,
        )
        && let Some(winning_team) = outcome.winning_team()
        && managers.commit_terminal(outcome)
    {
        events.push(BattleEvent::BattleTerminalCommitted {
            outcome,
            winning_team,
        });
    }
}

fn invocation_frame_target(
    target: crate::engine::skill::action::SkillTarget,
    trigger: &SkillOpTrigger,
) -> Option<i64> {
    match target {
        crate::engine::skill::action::SkillTarget::Explicit(uid) => Some(uid),
        crate::engine::skill::action::SkillTarget::Inherited => match trigger {
            SkillOpTrigger::Event(event) => event.target_uid(),
            SkillOpTrigger::Active | SkillOpTrigger::Setup { .. } => None,
        },
        crate::engine::skill::action::SkillTarget::Configured
        | crate::engine::skill::action::SkillTarget::LogicRule(_) => None,
    }
}

fn attach_buff_grant_relation(
    op: RuleOp,
    consequence: crate::engine::skill::condition::registry::ConsequencePolicy,
) -> RuleOp {
    use crate::engine::skill::rule::output::BattleCommand;

    match op {
        RuleOp::Command(BattleCommand::Buff(command)) => {
            RuleOp::Command(BattleCommand::Buff(attach_relation(command, consequence)))
        }
        RuleOp::Command(BattleCommand::BloodtitheSpend(mut command)) => {
            command.buff = attach_relation(command.buff, consequence);
            RuleOp::Command(BattleCommand::BloodtitheSpend(command))
        }
        other => other,
    }
}

fn attach_relation(
    command: crate::engine::manager::buff::BuffCommand,
    consequence: crate::engine::skill::condition::registry::ConsequencePolicy,
) -> crate::engine::manager::buff::BuffCommand {
    use crate::engine::manager::buff::{BuffCommand, BuffGrantRelation, RelatedBuffGrant};
    use crate::engine::skill::condition::registry::ConsequencePolicy;

    match command {
        BuffCommand::Grant(grant) if consequence == ConsequencePolicy::ChildBuffGrant => {
            BuffCommand::GrantRelated(RelatedBuffGrant {
                grant,
                relation: BuffGrantRelation::Child,
            })
        }
        BuffCommand::Grant(grant) if consequence == ConsequencePolicy::NormalBuffGrant => {
            BuffCommand::GrantRelated(RelatedBuffGrant {
                grant,
                relation: BuffGrantRelation::Normal,
            })
        }
        BuffCommand::GrantSourceRelative(mut stateful)
            if consequence == ConsequencePolicy::ChildBuffGrant =>
        {
            stateful.relation = Some(BuffGrantRelation::Child);
            BuffCommand::GrantSourceRelative(stateful)
        }
        BuffCommand::GrantSourceRelative(mut stateful)
            if consequence == ConsequencePolicy::NormalBuffGrant =>
        {
            stateful.relation = Some(BuffGrantRelation::Normal);
            BuffCommand::GrantSourceRelative(stateful)
        }
        other => other,
    }
}

fn output_frame_path(
    owner: crate::engine::skill::behavior::registry::OutputOwner,
    current: &[usize],
) -> Vec<usize> {
    use crate::engine::skill::behavior::registry::OutputOwner;
    match owner {
        OutputOwner::Skill => return current.to_vec(),
        OutputOwner::Parent => {}
        OutputOwner::CausingEvent
        | OutputOwner::ActionSkillReaction
        | OutputOwner::NestedLifecycleMechanic
        | OutputOwner::SetupParent
        | OutputOwner::SetupOrFinalSettlementParent => {
            unreachable!("conditional output ownership must be resolved before draining")
        }
    }
    let mut parent = current.to_vec();
    parent.pop();
    if parent.is_empty() {
        current.to_vec()
    } else {
        parent
    }
}

fn ensure_frame(
    frames: &mut Vec<SemanticFrame>,
    current: Option<FramePath>,
    parent: Option<&[usize]>,
    owner: FrameOwner,
    trigger: &SkillOpTrigger,
) -> FramePath {
    current.unwrap_or_else(|| {
        let trigger = frame_trigger(trigger);
        match parent {
            Some(parent) => push_child(frames, parent, owner, trigger),
            None => push_root(frames, owner, trigger),
        }
    })
}

fn frame_trigger(trigger: &SkillOpTrigger) -> FrameTrigger {
    match trigger {
        SkillOpTrigger::Active => FrameTrigger::Active,
        SkillOpTrigger::Event(event) => FrameTrigger::Event(event.clone()),
        SkillOpTrigger::Setup { stage, priority } => FrameTrigger::Setup {
            stage: *stage,
            priority: *priority,
        },
    }
}

fn prepend(queue: &mut VecDeque<QueuedOp>, items: impl IntoIterator<Item = QueuedOp>) {
    let items = items.into_iter().collect::<Vec<_>>();
    for item in items.into_iter().rev() {
        queue.push_front(item);
    }
}

fn take_conduit_activation_completion(
    queue: &mut VecDeque<QueuedOp>,
    source_uid: i64,
    skill_id: i32,
) -> Option<QueuedOp> {
    let index = queue.iter().position(|queued| {
        matches!(
            queued.op,
            RuleOp::Command(
                crate::engine::skill::rule::output::BattleCommand::Conduit(
                    crate::engine::manager::conduit::ConduitCommand::CompleteActivation {
                        source_uid: queued_source,
                        skill_id: queued_skill,
                    }
                )
            ) if queued_source == source_uid && queued_skill == skill_id
        )
    })?;
    queue.remove(index)
}

// The reaction step inside an action that cast a held skill; the action's own casts have none.
fn casting_reaction(
    frames: &[SemanticFrame],
    action_path: &[usize],
    frame_path: &[usize],
) -> Option<FrameOwner> {
    (action_path.len() + 1..=frame_path.len())
        .rev()
        .find_map(|len| match owner_at_path(frames, &frame_path[..len]) {
            owner @ FrameOwner::Skill { .. } => Some(owner.clone()),
            _ => None,
        })
}

fn join_caster_to_action(
    frames: &[SemanticFrame],
    action_path: &[usize],
    mut caster: FrameOwner,
) -> FrameOwner {
    if let Some(target) = active_skill_target_uid(frames, action_path)
        && let FrameOwner::Skill { target_uid, .. } = &mut caster
    {
        *target_uid = Some(target);
    }
    caster
}

fn ally_action_reaction_group(queued: &QueuedOp) -> Option<Rc<RefCell<Option<FramePath>>>> {
    let SkillOpTrigger::Event(BattleEvent::AllyAction(_)) = &queued.trigger else {
        return None;
    };
    let RuleOp::Skill(_) = &queued.op else {
        return None;
    };
    queued.frame_group.clone()
}

fn ally_action_reaction_groups(queued: &[QueuedOp]) -> Vec<Rc<RefCell<Option<FramePath>>>> {
    let mut groups = Vec::new();
    for group in queued.iter().filter_map(ally_action_reaction_group) {
        if !groups.iter().any(|known| Rc::ptr_eq(known, &group)) {
            groups.push(group);
        }
    }
    groups
}

fn insert_after_frame(
    queue: &mut VecDeque<QueuedOp>,
    frame_path: &[usize],
    items: impl IntoIterator<Item = QueuedOp>,
) {
    let index = queue
        .iter()
        .rposition(|queued| queued_belongs_to_frame_subtree(queued, frame_path))
        .map_or(0, |index| index + 1);
    for (offset, item) in items.into_iter().enumerate() {
        queue.insert(index + offset, item);
    }
}

fn queued_belongs_to_frame_subtree(queued: &QueuedOp, frame_path: &[usize]) -> bool {
    queued
        .frame_path
        .as_deref()
        .is_some_and(|path| path.starts_with(frame_path))
        || queued
            .parent_path
            .as_deref()
            .is_some_and(|path| path.starts_with(frame_path))
        || queued.frame_group.as_ref().is_some_and(|group| {
            group
                .borrow()
                .as_deref()
                .is_some_and(|path| path.starts_with(frame_path))
        })
}

#[cfg(test)]
mod tests;
