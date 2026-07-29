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
        executor::{RuleExecutionError, RuleOutcome, execute_rule_op},
        record::{
            FrameOwner, FramePath, FrameTrigger, SemanticFrame, SetupSide, active_skill_scope_path,
            event_scope_path, owner_at_path, push_change, push_child, push_root, set_skill_target,
        },
        skill::{self, SkillExecution, SkillOpError, SkillOpTrigger},
    },
    skill::{
        effect::SkillEffectCatalog,
        rule::{SetupStage, output::RuleOp},
        subscriber::SubscriberError,
        target::{TargetContext, TargetPool},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrainError {
    Command(RuleExecutionError),
    Skill(SkillOpError),
    MissingBuffActOp(i32),
    MissingBuffActDefinition {
        opcode: Option<i32>,
        type_name: String,
    },
    InvalidGaugeContributionPlan {
        key: crate::engine::manager::gauge::GaugeKey,
        expected: usize,
        actual: usize,
    },
    MissingAssistBoss,
    InvalidAssistBossSkill(i32),
    InsufficientAssistBossPower(i32),
    ForbiddenCardSkill {
        owner_uid: i64,
        skill_id: i32,
    },
    InsufficientUltimateResource {
        owner_uid: i64,
        skill_id: i32,
        required: i32,
        current: i32,
    },
    Subscriber(SubscriberError),
    Impromptu(crate::engine::mechanic::impromptu::ImpromptuError),
    ShadowCloak(crate::engine::mechanic::shadow_cloak::CapacityPlanError),
    BattleTrigger(crate::engine::fight::trigger::BattleTriggerError),
    OperationLimitExceeded,
    RecursionLimitExceeded,
    MissingActiveSkillContext,
}

fn required_buff_act_definition(
    opcode: Option<i32>,
    type_name: &str,
) -> Result<&'static crate::engine::skill::buff_act::registry::BuffActDefinition, DrainError> {
    opcode
        .and_then(|opcode| crate::engine::skill::buff_act::registry::find(opcode, type_name))
        .ok_or_else(|| DrainError::MissingBuffActDefinition {
            opcode,
            type_name: type_name.to_owned(),
        })
}

const MAX_DRAIN_OPERATIONS: usize = 16_384;
const MAX_DRAIN_DEPTH: usize = 64;

struct DrainBudget {
    remaining_operations: usize,
}

impl Default for DrainBudget {
    fn default() -> Self {
        Self {
            remaining_operations: MAX_DRAIN_OPERATIONS,
        }
    }
}

impl DrainBudget {
    fn consume(&mut self, depth: usize) -> Result<(), DrainError> {
        if depth >= MAX_DRAIN_DEPTH {
            return Err(DrainError::RecursionLimitExceeded);
        }
        self.remaining_operations = self
            .remaining_operations
            .checked_sub(1)
            .ok_or(DrainError::OperationLimitExceeded)?;
        Ok(())
    }
}

impl From<crate::engine::fight::trigger::BattleTriggerError> for DrainError {
    fn from(value: crate::engine::fight::trigger::BattleTriggerError) -> Self {
        Self::BattleTrigger(value)
    }
}

impl From<crate::engine::mechanic::shadow_cloak::CapacityPlanError> for DrainError {
    fn from(value: crate::engine::mechanic::shadow_cloak::CapacityPlanError) -> Self {
        Self::ShadowCloak(value)
    }
}

impl From<RuleExecutionError> for DrainError {
    fn from(value: RuleExecutionError) -> Self {
        Self::Command(value)
    }
}

impl From<SkillOpError> for DrainError {
    fn from(value: SkillOpError) -> Self {
        Self::Skill(value)
    }
}

impl From<SubscriberError> for DrainError {
    fn from(value: SubscriberError) -> Self {
        Self::Subscriber(value)
    }
}

#[derive(Default)]
pub struct DrainResult {
    pub outcomes: Vec<RuleOutcome>,
    pub events: Vec<BattleEvent>,
    pub frames: Vec<SemanticFrame>,
}

struct QueuedOp {
    op: RuleOp,
    trigger: SkillOpTrigger,
    skill_execution: Option<SkillExecution>,
    frame_path: Option<FramePath>,
    parent_path: Option<FramePath>,
    frame_group: Option<Rc<RefCell<Option<FramePath>>>>,
    independent_parent_group: Option<Rc<RefCell<Option<FramePath>>>>,
    frame_owner: Option<FrameOwner>,
}

#[derive(Default)]
struct ReactionBatch {
    before_publish: Vec<QueuedOp>,
    after_publish: Vec<QueuedOp>,
    after_skill: Vec<QueuedOp>,
    after_hit: Vec<QueuedOp>,
    after_action: Vec<QueuedOp>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactionLane {
    Skills,
    BuffActs,
    BuffActsBeforeSettlement,
    BuffActsAfterSettlement,
}

impl ReactionBatch {
    fn into_ordered(self) -> Vec<QueuedOp> {
        self.before_publish
            .into_iter()
            .chain(self.after_publish)
            .chain(self.after_skill)
            .chain(self.after_action)
            .collect()
    }

    fn partition_skill_reactions(self) -> (Self, Self) {
        fn partition(items: Vec<QueuedOp>) -> (Vec<QueuedOp>, Vec<QueuedOp>) {
            items
                .into_iter()
                .partition(|queued| !matches!(queued.frame_owner, Some(FrameOwner::Skill { .. })))
        }

        let (buff_before_publish, skill_before_publish) = partition(self.before_publish);
        let (buff_after_publish, skill_after_publish) = partition(self.after_publish);
        let (buff_after_skill, skill_after_skill) = partition(self.after_skill);
        let (buff_after_hit, skill_after_hit) = partition(self.after_hit);
        let (buff_after_action, skill_after_action) = partition(self.after_action);

        (
            Self {
                before_publish: buff_before_publish,
                after_publish: buff_after_publish,
                after_skill: buff_after_skill,
                after_hit: buff_after_hit,
                after_action: buff_after_action,
            },
            Self {
                before_publish: skill_before_publish,
                after_publish: skill_after_publish,
                after_skill: skill_after_skill,
                after_hit: skill_after_hit,
                after_action: skill_after_action,
            },
        )
    }
}

mod entry;
mod reactions;
mod state;

pub use entry::*;
use reactions::{dispatch_event_batch, dispatch_owner_reactions, dispatch_reactions};
#[cfg(test)]
use reactions::{
    ordered_hit_entities, queued_reactions, reaction_counterparty, reaction_skill_target,
};
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
    let mut state = DrainState::new(context);
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
    let mut pending_hits = HashMap::<FramePath, Vec<BattleEvent>>::new();
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

        // Previous operations may have changed HP, buffs, entities, or resources.
        // Target resolution must see those committed manager values.
        let current_pool = base_pool.runtime_view(managers);
        let pool = &current_pool;

        // Frame groups keep independent reactions under one semantic owner while
        // ordinary children inherit the parent selected by their emitter.
        let frame_path = frame_path.or_else(|| {
            frame_group
                .as_ref()
                .and_then(|group| group.borrow().clone())
        });
        let parent_path = if let Some(group) = &independent_parent_group {
            let existing = group.borrow().clone();
            let path = existing.unwrap_or_else(|| {
                let path = push_root(
                    &mut result.frames,
                    FrameOwner::EventRule,
                    frame_trigger(&trigger),
                );
                *group.borrow_mut() = Some(path.clone());
                path
            });
            Some(path)
        } else {
            parent_path
        };
        match op {
            RuleOp::Skill(invocation) => {
                // After terminal commitment, skip active skills that have not
                // started a frame. Existing-frame continuations may finish settling.
                if matches!(trigger, SkillOpTrigger::Active)
                    && frame_path.is_none()
                    && managers.terminal_outcome().is_some()
                {
                    continue;
                }
                if matches!(trigger, SkillOpTrigger::Active)
                    && base_pool.team_type(invocation.plan.source_uid).is_some()
                    && ((base_pool.entity(invocation.plan.source_uid).is_some()
                        && pool.entity(invocation.plan.source_uid).is_none())
                        || (invocation
                            .phase
                            .unwrap_or(crate::engine::skill::action::SkillPhase::Immediate)
                            == crate::engine::skill::action::SkillPhase::Immediate
                            && catalog.is_attack(invocation.plan.skill_id)
                            && pool.enemies(invocation.plan.source_uid, false).is_empty()))
                {
                    continue;
                }

                // A skill emitted by a buff act remains nested under that buff-act
                // frame; the skill still runs through the normal skill emitter.
                let (frame_path, parent_path, frame_owner, skill_from_buff_act) =
                    if let Some(owner @ FrameOwner::BuffAct { .. }) = frame_owner {
                        let buff_path = ensure_frame(
                            &mut result.frames,
                            frame_path,
                            parent_path.as_deref(),
                            owner,
                            &trigger,
                        );
                        if let Some(group) = &frame_group {
                            *group.borrow_mut() = Some(buff_path.clone());
                        }
                        (None, Some(buff_path), None, true)
                    } else {
                        (frame_path, parent_path, frame_owner, false)
                    };
                let trigger = if skill_from_buff_act {
                    SkillOpTrigger::Active
                } else {
                    trigger
                };
                let frame_path = ensure_frame(
                    &mut result.frames,
                    frame_path,
                    parent_path.as_deref(),
                    frame_owner.unwrap_or(FrameOwner::Skill {
                        source_uid: invocation.plan.source_uid,
                        skill_id: invocation.plan.skill_id,
                        card_index: invocation.card_index,
                        target_uid: invocation_frame_target(invocation.target, &trigger),
                    }),
                    &trigger,
                );
                if !skill_from_buff_act && let Some(group) = &frame_group {
                    *group.borrow_mut() = Some(frame_path.clone());
                }
                if matches!(trigger, SkillOpTrigger::Active)
                    && invocation.phase
                        == Some(crate::engine::skill::action::SkillPhase::AfterDamage)
                    && let Some(deaths) = state.take_deaths(&frame_path)
                {
                    for death in deaths
                        .into_iter()
                        .filter(|death| managers.hp.current(death.target_uid) == 0)
                    {
                        push_change(
                            &mut result.frames,
                            &frame_path,
                            crate::engine::runtime::change::BattleChange::Death(death),
                        );
                    }
                }
                let mut execution = skill_execution.unwrap_or_else(|| SkillExecution::new(context));
                if matches!(trigger, SkillOpTrigger::Active)
                    && let Some(additional_count) = state.take_target_modifier(&frame_path)
                {
                    execution.add_skill_targets(additional_count);
                }
                if matches!(trigger, SkillOpTrigger::Active)
                    && let Some(injuries) = state.injuries(&frame_path)
                {
                    let source_is_attacker = pool.source_is_attacker(invocation.plan.source_uid);
                    execution.record_injuries(injuries.iter().copied().filter(|target_uid| {
                        pool.source_is_attacker(*target_uid) == source_is_attacker
                    }));
                }
                if invocation.mode == crate::engine::skill::action::SkillExecutionMode::DirectBig {
                    execution.prepare_direct_big(invocation.additional_moxie);
                }

                // Skill evaluation emits RuleOps only. Managers remain the sole
                // owners of durable mutations when those operations are drained.
                let emission = skill::emit_ops(
                    invocation.clone(),
                    managers,
                    pool,
                    catalog,
                    determinism,
                    &mut execution,
                    &trigger,
                )?;
                for &(slot_index, condition_key) in &emission.fired_rules {
                    managers.mark_rule_fired(
                        invocation.plan.source_uid,
                        invocation.plan.skill_id,
                        slot_index,
                        condition_key,
                    );
                }
                set_skill_target(&mut result.frames, &frame_path, emission.target_uid);
                let mut outputs = Vec::new();
                for emission in emission.ops {
                    let skill::SkillEmissionOp {
                        op,
                        owner,
                        consequence,
                        frame_owner,
                    } = emission;
                    let op = attach_buff_grant_relation(op, consequence);
                    match op {
                        RuleOp::Skill(child) => {
                            let after_current_action = child.start
                                == crate::engine::skill::action::SkillStart::AfterCurrentAction;
                            let queued = QueuedOp {
                                op: RuleOp::Skill(child),
                                trigger: SkillOpTrigger::Active,
                                skill_execution: None,
                                frame_path: None,
                                parent_path: Some(frame_path.clone()),
                                frame_group: None,
                                independent_parent_group: None,
                                frame_owner: None,
                            };
                            if after_current_action {
                                state.push_after_action(frame_path.clone(), queued);
                            } else {
                                outputs.push(queued);
                            }
                        }
                        command => outputs.push(match frame_owner {
                            Some(frame_owner) => QueuedOp {
                                op: command,
                                trigger: trigger.clone(),
                                skill_execution: None,
                                frame_path: None,
                                parent_path: Some(frame_path.clone()),
                                frame_group: None,
                                independent_parent_group: None,
                                frame_owner: Some(frame_owner),
                            },
                            None => QueuedOp {
                                op: command,
                                trigger: trigger.clone(),
                                skill_execution: None,
                                frame_path: Some(output_frame_path(owner, &frame_path)),
                                parent_path: None,
                                frame_group: None,
                                independent_parent_group: None,
                                frame_owner: None,
                            },
                        }),
                    }
                }
                if let Some(continuation) = emission.continuation {
                    outputs.push(QueuedOp {
                        op: RuleOp::Skill(continuation),
                        trigger,
                        skill_execution: Some(execution),
                        frame_path: Some(frame_path),
                        parent_path: None,
                        frame_group: None,
                        independent_parent_group: None,
                        frame_owner: None,
                    });
                }
                prepend(queue, outputs);
            }
            mut command @ (RuleOp::Command(_)
            | RuleOp::Publish(_)
            | RuleOp::SkillLifecycle(_)
            | RuleOp::BeginSkillAction { .. }
            | RuleOp::BuffFeatureMarker { .. }
            | RuleOp::EffectMarker { .. }
            | RuleOp::SceneChange { .. }
            | RuleOp::BuffActTrigger(_)
            | RuleOp::BuffActInfoMarker(_)
            | RuleOp::MarkBuffActFired { .. }
            | RuleOp::ModifyActiveSkillTargets { .. }
            | RuleOp::NuoDiKaHit(_)) => {
                // After-hit observers must execute before the lifecycle operation
                // that closes their boundary, so requeue that closer behind them.
                let releases_after_hit = matches!(
                    &command,
                    RuleOp::SkillLifecycle(
                        crate::engine::skill::action::SkillLifecycle::PhaseCompleted(event)
                    ) if event.phase == crate::engine::skill::action::SkillPhase::AfterHit
                ) || matches!(
                    &command,
                    RuleOp::SkillLifecycle(
                        crate::engine::skill::action::SkillLifecycle::ActionCompleted(_)
                    )
                );
                let action_scope = frame_path
                    .as_deref()
                    .and_then(|path| active_skill_scope_path(&result.frames, path));
                if releases_after_hit
                    && let Some(observers) = state.take_after_hit(action_scope.as_ref())
                {
                    queue.push_front(QueuedOp {
                        op: command,
                        trigger,
                        skill_execution,
                        frame_path,
                        parent_path,
                        frame_group,
                        independent_parent_group,
                        frame_owner,
                    });
                    prepend(queue, observers);
                    continue;
                }
                let completes_action = matches!(
                    &command,
                    RuleOp::SkillLifecycle(
                        crate::engine::skill::action::SkillLifecycle::ActionCompleted(_)
                    )
                );
                let deferred_followup_owner = matches!(
                    &command,
                    RuleOp::Command(
                        crate::engine::skill::rule::output::BattleCommand::ThresholdSkill(_)
                    )
                )
                .then(|| frame_owner.clone())
                .flatten();
                let frame_path = if deferred_followup_owner.is_some() {
                    frame_path.or(parent_path).unwrap_or_else(|| {
                        push_root(
                            &mut result.frames,
                            FrameOwner::Command,
                            frame_trigger(&trigger),
                        )
                    })
                } else {
                    ensure_frame(
                        &mut result.frames,
                        frame_path,
                        parent_path.as_deref(),
                        frame_owner.unwrap_or(FrameOwner::Command),
                        &trigger,
                    )
                };
                if deferred_followup_owner.is_none()
                    && let Some(group) = &frame_group
                {
                    *group.borrow_mut() = Some(frame_path.clone());
                }
                if let RuleOp::SkillLifecycle(
                    crate::engine::skill::action::SkillLifecycle::PhaseCompleted(event),
                ) = &mut command
                    && let Some(execution) = queue.iter().find_map(|queued| {
                        (matches!(queued.trigger, SkillOpTrigger::Active)
                            && queued.frame_path.as_ref() == Some(&frame_path))
                        .then_some(queued.skill_execution.as_ref())
                        .flatten()
                    })
                {
                    execution.sync_lifecycle_event(event);
                }
                if let RuleOp::SkillLifecycle(lifecycle) = &mut command
                    && let Some(action_path) = active_skill_scope_path(&result.frames, &frame_path)
                    && let Some(injuries) = state.injuries(&action_path)
                {
                    let source_uid = match lifecycle {
                        crate::engine::skill::action::SkillLifecycle::PhaseCompleted(event) => {
                            event.source_uid
                        }
                        crate::engine::skill::action::SkillLifecycle::ActionCompleted(event) => {
                            event.source_uid
                        }
                        _ => 0,
                    };
                    let source_is_attacker = pool.source_is_attacker(source_uid);
                    let count = injuries
                        .iter()
                        .filter(|target_uid| {
                            pool.source_is_attacker(**target_uid) == source_is_attacker
                        })
                        .count() as i32;
                    match lifecycle {
                        crate::engine::skill::action::SkillLifecycle::PhaseCompleted(event) => {
                            event.teammate_injury_count = count;
                            event.teammate_injury_count_not_reset = count;
                        }
                        crate::engine::skill::action::SkillLifecycle::ActionCompleted(event) => {
                            event.teammate_injury_count = count;
                            event.teammate_injury_count_not_reset = count;
                        }
                        _ => {}
                    }
                }
                if let RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Card(
                    crate::engine::manager::card::CardCommand::RedealKeepRanks { origin },
                )) = command
                {
                    let draw_pile = managers.card.draw_pile().to_vec();
                    let mut replacements = Vec::new();
                    for old in managers.card.redealable_cards() {
                        let candidates = draw_pile
                            .iter()
                            .filter(|candidate| candidate.skill_id != old.skill_id)
                            .cloned()
                            .collect::<Vec<_>>();
                        replacements.extend(determinism.draw_cards(
                            if candidates.is_empty() {
                                &draw_pile
                            } else {
                                &candidates
                            },
                            1,
                        ));
                    }
                    command =
                        RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Card(
                            crate::engine::manager::card::CardCommand::ApplyRedealKeepRanks(
                                crate::engine::manager::card::CardRedealKeepRanks {
                                    origin,
                                    replacements,
                                },
                            ),
                        ));
                }

                // This is the single durable commit point for non-skill RuleOps.
                // The returned outcome describes committed changes and follow-ups.
                let mut outcome = execute_rule_op(managers, &mut bus, command)?;
                if let RuleOutcome::ActiveSkillTargetsModified(additional_count) = outcome {
                    let action_scope = action_scope
                        .clone()
                        .ok_or(DrainError::MissingActiveSkillContext)?;
                    state.add_target_modifier(action_scope, additional_count);
                }
                let followups = outcome.followups();
                let damage_amount = outcome.applied_damage();
                let death_count = outcome.death_count();
                let injured_targets = outcome.injured_targets();
                if !injured_targets.is_empty()
                    && let Some(action_path) = active_skill_scope_path(&result.frames, &frame_path)
                {
                    state.record_injuries(action_path, &injured_targets);
                }
                if (damage_amount > 0 || death_count > 0 || !injured_targets.is_empty())
                    && matches!(trigger, SkillOpTrigger::Active)
                    && let Some(queued) = queue.iter_mut().find(|queued| {
                        matches!(queued.trigger, SkillOpTrigger::Active)
                            && queued.frame_path.as_ref() == Some(&frame_path)
                            && matches!(queued.op, RuleOp::Skill(_))
                    })
                {
                    let source_uid = match &queued.op {
                        RuleOp::Skill(invocation) => invocation.plan.source_uid,
                        _ => 0,
                    };
                    let source_is_attacker = pool.source_is_attacker(source_uid);
                    let allied_injuries = injured_targets.into_iter().filter(|target_uid| {
                        pool.source_is_attacker(*target_uid) == source_is_attacker
                    });
                    if let Some(execution) = queued.skill_execution.as_mut() {
                        execution.record_damage(damage_amount);
                        execution.record_injuries(allied_injuries);
                    }
                }
                let event_scope = event_scope_path(&result.frames, &frame_path);
                let fanout = match &outcome {
                    RuleOutcome::Buff(changes) => changes.fanout.clone(),
                    _ => Vec::new(),
                };
                let current_skill = match owner_at_path(&result.frames, &frame_path) {
                    FrameOwner::Skill {
                        source_uid,
                        skill_id,
                        target_uid,
                        ..
                    } => Some((*source_uid, *skill_id, *target_uid)),
                    _ => None,
                };

                // Managers publish semantic events through the bus. Reactions are
                // derived from these committed events, never fired by packet code.
                let mut events = std::iter::from_fn(|| bus.pop()).collect::<Vec<_>>();
                if matches!(trigger, SkillOpTrigger::Active)
                    && let Some(queued) = queue.iter_mut().find(|queued| {
                        matches!(queued.trigger, SkillOpTrigger::Active)
                            && queued.frame_path.as_ref() == Some(&frame_path)
                            && matches!(queued.op, RuleOp::Skill(_))
                    })
                {
                    let source_uid = match &queued.op {
                        RuleOp::Skill(invocation) => invocation.plan.source_uid,
                        _ => 0,
                    };
                    if let Some(execution) = queued.skill_execution.as_mut() {
                        execution.record_attacks(events.iter().filter_map(|event| {
                            let BattleEvent::Hit(hit) = event else {
                                return None;
                            };
                            (hit.damage_from
                                == crate::engine::manager::hp::HurtDamageFromType::Skill)
                                .then_some(hit.target_uid)
                        }));
                        execution.record_buff_additions(
                            events.iter().filter_map(|event| {
                                let (BattleEvent::BuffAdded(change)
                                | BattleEvent::BuffChanged(change)) = event
                                else {
                                    return None;
                                };
                                (change.source_uid == source_uid).then_some((
                                    change.buff_id,
                                    change.after_amount.saturating_sub(change.before_amount),
                                ))
                            }),
                        );
                    }
                }

                let has_active_continuation = queue.iter().any(|queued| {
                    matches!(&queued.trigger, SkillOpTrigger::Active)
                        && queued.frame_path.as_ref() == Some(&frame_path)
                        && matches!(&queued.op, RuleOp::Skill(_))
                });
                let defer_hits = matches!(&outcome, RuleOutcome::HpBatch(_))
                    && matches!(trigger, SkillOpTrigger::Active)
                    && current_skill.is_some()
                    && has_active_continuation;
                if defer_hits {
                    // Multi-part active hits share one HitPassives boundary. Hold
                    // only Hit events; unrelated events remain immediately visible.
                    let mut immediate = Vec::new();
                    for event in events {
                        if matches!(event, BattleEvent::Hit(_)) {
                            pending_hits
                                .entry(frame_path.clone())
                                .or_default()
                                .push(event);
                        } else {
                            immediate.push(event);
                        }
                    }
                    events = immediate;
                }

                // BeforePublish reactions run before this outcome is recorded in
                // its semantic frame. Their own manager commits use nested drains.
                let action_scope = active_skill_scope_path(&result.frames, &frame_path);
                let mut before_reactions = dispatch_event_batch(
                    pool,
                    managers,
                    catalog,
                    determinism,
                    &events,
                    &event_scope,
                    &frame_path,
                    action_scope.as_deref(),
                    current_skill,
                    matches!(&outcome, RuleOutcome::HpBatch(_)),
                    context.current_round > 0,
                    crate::engine::event::subscription::PublicationPhase::BeforePublish,
                    None,
                )?;
                let mut started_after_reactions = None;
                if matches!(&outcome, RuleOutcome::SkillActionStarted { .. }) {
                    let mut started_reactions = dispatch_event_batch(
                        pool,
                        managers,
                        catalog,
                        determinism,
                        &events,
                        &event_scope,
                        &frame_path,
                        action_scope.as_deref(),
                        current_skill,
                        matches!(&outcome, RuleOutcome::HpBatch(_)),
                        context.current_round > 0,
                        crate::engine::event::subscription::PublicationPhase::AfterPublish,
                        None,
                    )?;
                    before_reactions
                        .before_publish
                        .append(&mut started_reactions.after_publish);
                    started_after_reactions = Some(started_reactions);
                }
                result.events.extend(events.iter().cloned());
                let releases_pending_hits = matches!(
                    &outcome,
                    RuleOutcome::SkillLifecycle(
                        crate::engine::skill::action::SkillLifecycle::PhaseCompleted(event)
                    ) if event.phase == crate::engine::skill::action::SkillPhase::HitPassives
                );
                let mut pending_hit_skills = ReactionBatch::default();
                let released_hit_events = releases_pending_hits
                    .then(|| pending_hits.remove(&frame_path))
                    .flatten();
                if let Some(hit_events) = released_hit_events.as_ref() {
                    let hit_reactions = dispatch_event_batch(
                        pool,
                        managers,
                        catalog,
                        determinism,
                        hit_events,
                        &event_scope,
                        &frame_path,
                        action_scope.as_deref(),
                        current_skill,
                        true,
                        context.current_round > 0,
                        crate::engine::event::subscription::PublicationPhase::BeforePublish,
                        None,
                    )?;
                    result.events.extend(hit_events.iter().cloned());
                    let (hit_buff_acts, hit_skills) = hit_reactions.partition_skill_reactions();
                    pending_hit_skills = hit_skills;
                    drain_nested_queue(
                        managers,
                        pool,
                        catalog,
                        determinism,
                        hit_buff_acts.into_ordered().into(),
                        &mut result,
                        state,
                    )?;
                }
                if !before_reactions.before_publish.is_empty() {
                    drain_nested_queue(
                        managers,
                        pool,
                        catalog,
                        determinism,
                        before_reactions.before_publish.into(),
                        &mut result,
                        state,
                    )?;
                }
                if !pending_hit_skills.before_publish.is_empty()
                    || !pending_hit_skills.after_publish.is_empty()
                {
                    drain_nested_queue(
                        managers,
                        pool,
                        catalog,
                        determinism,
                        pending_hit_skills.into_ordered().into(),
                        &mut result,
                        state,
                    )?;
                }

                // Once pre-publication work is complete, record the authoritative
                // outcome and any buff fanout beneath their semantic owners.
                let pending_deaths = outcome.take_deaths();
                let changes = outcome.changes();
                for change in changes {
                    push_change(&mut result.frames, &frame_path, change);
                }
                for fanout in fanout {
                    let fanout_path = push_child(
                        &mut result.frames,
                        &event_scope,
                        FrameOwner::BuffRule {
                            emitter_uid: fanout.emitter_uid,
                            carrier_buff_uid: fanout.carrier_buff_uid,
                            carrier_buff_id: fanout.carrier_buff_id,
                            rule: fanout.rule,
                        },
                        frame_trigger(&trigger),
                    );
                    push_change(
                        &mut result.frames,
                        &fanout_path,
                        crate::engine::runtime::change::BattleChange::BuffFanout(Box::new(fanout)),
                    );
                }
                let was_skill_action_started =
                    matches!(&outcome, RuleOutcome::SkillActionStarted { .. });
                let was_hp_batch = matches!(&outcome, RuleOutcome::HpBatch(_));
                result.outcomes.push(outcome);

                // Battle outcome logic selects the terminal boundary. Its manager
                // commits that boundary here; later reactions are limited to the winner.
                if managers.terminal_outcome().is_none()
                    && context.battle_id > 0
                    && let Some(outcome) =
                        crate::engine::round::outcome::terminal_outcome_for_battle_id(
                            context.battle_id,
                            pool,
                            managers,
                        )
                    && let Some(winning_team) = outcome.winning_team()
                    && managers.commit_terminal(outcome)
                {
                    result.events.push(BattleEvent::BattleTerminalCommitted {
                        outcome,
                        winning_team,
                    });
                }
                let terminal_owner_uids = managers
                    .terminal_outcome()
                    .and_then(|outcome| outcome.winning_team())
                    .map(|winning_team| pool.team_uids(winning_team));

                // AfterPublish reactions are partitioned by their declared release
                // lane; after-hit and after-action work stays action-scoped in state.
                let mut reactions = if was_skill_action_started {
                    started_after_reactions.unwrap_or_default()
                } else {
                    dispatch_event_batch(
                        pool,
                        managers,
                        catalog,
                        determinism,
                        &events,
                        &event_scope,
                        &frame_path,
                        action_scope.as_deref(),
                        current_skill,
                        was_hp_batch,
                        context.current_round > 0,
                        crate::engine::event::subscription::PublicationPhase::AfterPublish,
                        terminal_owner_uids.as_deref(),
                    )?
                };
                state.defer_after_hit(
                    action_scope.as_deref(),
                    std::mem::take(&mut reactions.after_hit),
                );
                let after_action = std::mem::take(&mut reactions.after_action);
                if action_scope.is_some() {
                    state.defer_after_action(action_scope.as_deref(), after_action);
                } else {
                    reactions.after_publish.extend(after_action);
                }
                if let Some(hit_events) = released_hit_events.as_ref() {
                    let mut hit_reactions = dispatch_event_batch(
                        pool,
                        managers,
                        catalog,
                        determinism,
                        hit_events,
                        &event_scope,
                        &frame_path,
                        action_scope.as_deref(),
                        current_skill,
                        true,
                        context.current_round > 0,
                        crate::engine::event::subscription::PublicationPhase::AfterPublish,
                        terminal_owner_uids.as_deref(),
                    )?;
                    state.defer_after_hit(
                        action_scope.as_deref(),
                        std::mem::take(&mut hit_reactions.after_hit),
                    );
                    let hit_after_action = std::mem::take(&mut hit_reactions.after_action);
                    if action_scope.is_some() {
                        state.defer_after_action(action_scope.as_deref(), hit_after_action);
                    } else {
                        hit_reactions.after_publish.extend(hit_after_action);
                    }
                    let (hit_buff_acts, hit_skills) = hit_reactions.partition_skill_reactions();
                    let hit_queue = hit_buff_acts
                        .into_ordered()
                        .into_iter()
                        .chain(hit_skills.into_ordered())
                        .collect::<VecDeque<_>>();
                    drain_nested_queue(
                        managers,
                        pool,
                        catalog,
                        determinism,
                        hit_queue,
                        &mut result,
                        state,
                    )?;
                }
                let mut after_publish = reactions.after_publish;

                // Death-sensitive reactions declared before settlement get one
                // chance to change HP before death transitions are finalized.
                if managers.terminal_outcome().is_none()
                    && !pending_deaths.is_empty()
                    && !after_publish.is_empty()
                {
                    let (after_settlement, before_settlement): (Vec<_>, Vec<_>) = after_publish
                        .into_iter()
                        .partition(|queued| {
                            queued_runtime_settlement_phase(queued)
                                == crate::engine::skill::buff_act::registry::RuntimeSettlementPhase::After
                        });
                    drain_nested_queue(
                        managers,
                        pool,
                        catalog,
                        determinism,
                        before_settlement.into(),
                        &mut result,
                        state,
                    )?;
                    after_publish = after_settlement;
                }

                // A pending death becomes a frame change only if HP is still zero.
                // Active-action deaths wait for the shared HitPassives release.
                let settled_deaths = pending_deaths
                    .into_iter()
                    .filter(|death| managers.hp.current(death.target_uid) == 0)
                    .collect::<Vec<_>>();
                if let Some(action_scope) = action_scope.as_ref() {
                    state.record_deaths(action_scope.clone(), settled_deaths.iter().copied());
                } else {
                    for death in &settled_deaths {
                        push_change(
                            &mut result.frames,
                            &frame_path,
                            crate::engine::runtime::change::BattleChange::Death(*death),
                        );
                    }
                }
                if releases_pending_hits
                    && let Some(action_scope) = action_scope.as_ref()
                    && let Some(deaths) = state.take_deaths(action_scope)
                {
                    for death in deaths
                        .into_iter()
                        .filter(|death| managers.hp.current(death.target_uid) == 0)
                    {
                        push_change(
                            &mut result.frames,
                            action_scope,
                            crate::engine::runtime::change::BattleChange::Death(death),
                        );
                    }
                }
                if !settled_deaths.is_empty()
                    && matches!(trigger, SkillOpTrigger::Active)
                    && let Some(queued) = queue.iter_mut().find(|queued| {
                        matches!(queued.trigger, SkillOpTrigger::Active)
                            && queued.frame_path.as_ref() == Some(&frame_path)
                            && matches!(queued.op, RuleOp::Skill(_))
                    })
                    && let Some(execution) = queued.skill_execution.as_mut()
                {
                    execution.record_kills(settled_deaths.len() as i32);
                }
                let after_action = if completes_action {
                    state.take_after_action(&frame_path)
                } else {
                    Vec::new()
                };
                prepend(queue, after_action);

                // Manager-produced follow-ups re-enter the same queue. Skills marked
                // AfterCurrentAction are retained until that action closes.
                let mut immediate_followups = Vec::new();
                for op in followups {
                    let after_current_action = matches!(
                        &op,
                        RuleOp::Skill(invocation)
                            if invocation.start
                                == crate::engine::skill::action::SkillStart::AfterCurrentAction
                    );
                    let skill_path = after_current_action
                        .then(|| active_skill_scope_path(&result.frames, &frame_path))
                        .flatten();
                    let queued = QueuedOp {
                        op,
                        trigger: SkillOpTrigger::Active,
                        skill_execution: None,
                        frame_path: None,
                        parent_path: skill_path.clone().or_else(|| Some(frame_path.clone())),
                        frame_group: None,
                        independent_parent_group: None,
                        frame_owner: deferred_followup_owner.clone(),
                    };
                    if let Some(skill_path) = skill_path {
                        state.push_after_action(skill_path, queued);
                    } else {
                        immediate_followups.push(queued);
                    }
                }
                prepend(queue, immediate_followups);
                insert_after_frame(queue, &frame_path, reactions.after_skill);
                prepend(queue, after_publish);
            }
        }
    }

    Ok(result)
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
        crate::engine::skill::action::SkillTarget::Configured => None,
    }
}

fn queued_runtime_settlement_phase(
    queued: &QueuedOp,
) -> crate::engine::skill::buff_act::registry::RuntimeSettlementPhase {
    match queued.frame_owner {
        Some(FrameOwner::BuffAct { key, .. }) => {
            crate::engine::skill::buff_act::registry::runtime_settlement_phase(
                key.opcode,
                key.type_name,
            )
        }
        _ => crate::engine::skill::buff_act::registry::RuntimeSettlementPhase::Before,
    }
}

fn attach_buff_grant_relation(
    op: RuleOp,
    consequence: crate::engine::skill::condition::registry::ConsequencePolicy,
) -> RuleOp {
    use crate::engine::manager::buff::{BuffCommand, BuffGrantRelation, RelatedBuffGrant};
    use crate::engine::skill::condition::registry::ConsequencePolicy;

    match op {
        RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Buff(
            BuffCommand::Grant(grant),
        )) if consequence == ConsequencePolicy::ChildBuffGrant => {
            RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Buff(
                BuffCommand::GrantRelated(RelatedBuffGrant {
                    grant,
                    relation: BuffGrantRelation::Child,
                }),
            ))
        }
        RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Buff(
            BuffCommand::Grant(grant),
        )) if consequence == ConsequencePolicy::NormalBuffGrant => {
            RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Buff(
                BuffCommand::GrantRelated(RelatedBuffGrant {
                    grant,
                    relation: BuffGrantRelation::Normal,
                }),
            ))
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
        OutputOwner::CausingEvent | OutputOwner::SetupParent => {
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
