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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrainError {
    Command(RuleExecutionError),
    Skill(SkillOpError),
    MissingBuffActOp(i32),
    MissingBuffActDefinition {
        opcode: Option<i32>,
        type_name: String,
    },
    MissingAssistBoss,
    InvalidAssistBossSkill(i32),
    InsufficientAssistBossPower(i32),
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
    MissingReactionSide(i64),
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
    subscriber: Option<SubscriberContext>,
    // The reaction step that cast a held skill, recreated when the skill runs.
    caster_frame: Option<FrameOwner>,
}

#[derive(Clone)]
struct SubscriberContext {
    owner_uid: i64,
    skill_id: Option<i32>,
    group: Option<Rc<RefCell<SubscriberDeathGroup>>>,
}

impl SubscriberContext {
    fn new(owner_uid: i64) -> Self {
        Self {
            owner_uid,
            skill_id: None,
            group: None,
        }
    }

    fn for_skill(owner_uid: i64, skill_id: i32) -> Self {
        Self {
            owner_uid,
            skill_id: Some(skill_id),
            group: None,
        }
    }
}

#[derive(Default)]
struct SubscriberDeathGroup {
    deaths: Vec<(crate::engine::manager::hp::PendingDeath, FramePath)>,
}

fn queued_defeated_owner_card_cleanup(
    pool: &TargetPool,
    managers: &BattleManagers,
    death: crate::engine::manager::hp::DeathTransition,
    parent_path: &[usize],
    owner: DefenderCardCleanupOwner,
) -> Option<QueuedOp> {
    let team_type = pool
        .team_type(death.target_uid)
        .or_else(|| managers.buff.team_type(death.target_uid))?;
    if !owner.queues_in_drain(team_type)
        || managers
            .entity
            .has_pending_reserve_promotion(death.target_uid, &managers.hp)
    {
        return None;
    }
    Some(QueuedOp {
        op: RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Card(
            crate::engine::manager::card::CardCommand::remove_owner(
                crate::engine::manager::card::CardRemoveOwner {
                    origin: crate::engine::skill::rule::CommandOrigin {
                        domain: crate::engine::skill::rule::RuleDomain::Lifecycle,
                        key: crate::engine::skill::rule::DefinitionKey::new(0, "EntityCardCleanup"),
                    },
                    owner_uid: death.target_uid,
                    team_type,
                },
            ),
        )),
        trigger: SkillOpTrigger::Event(BattleEvent::EntityDied(
            crate::engine::event::payload::EntityDiedEvent {
                source_uid: death.source_uid,
                target_uid: death.target_uid,
            },
        )),
        skill_execution: None,
        frame_path: None,
        parent_path: Some(parent_path.to_vec()),
        frame_group: None,
        independent_parent_group: None,
        frame_owner: Some(FrameOwner::RoundPhase(RoundPhase::EntityCardCleanup)),
        subscriber: None,
        caster_frame: None,
    })
}

fn queued_defeated_owner_buff_cleanup(
    managers: &BattleManagers,
    death: crate::engine::manager::hp::DeathTransition,
    parent_path: &[usize],
) -> Option<QueuedOp> {
    let commands = managers
        .buff
        .defeated_entity_inactive_halo_cleanup(death.target_uid, &managers.hp);
    (!commands.is_empty()).then(|| QueuedOp {
        op: RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::BuffBatch(commands)),
        trigger: SkillOpTrigger::Event(BattleEvent::EntityDied(
            crate::engine::event::payload::EntityDiedEvent {
                source_uid: death.source_uid,
                target_uid: death.target_uid,
            },
        )),
        skill_execution: None,
        frame_path: None,
        parent_path: Some(parent_path.to_vec()),
        frame_group: None,
        independent_parent_group: None,
        frame_owner: Some(FrameOwner::RoundPhase(RoundPhase::EntityBuffCleanup)),
        subscriber: None,
        caster_frame: None,
    })
}

fn queued_death_settlement(
    death: crate::engine::manager::hp::DeathTransition,
    parent_path: &[usize],
) -> QueuedOp {
    QueuedOp {
        op: RuleOp::DeathSettlement(death),
        trigger: SkillOpTrigger::Event(BattleEvent::EntityDied(
            crate::engine::event::payload::EntityDiedEvent {
                source_uid: death.source_uid,
                target_uid: death.target_uid,
            },
        )),
        skill_execution: None,
        frame_path: None,
        parent_path: Some(parent_path.to_vec()),
        frame_group: None,
        independent_parent_group: None,
        frame_owner: Some(FrameOwner::EventRule),
        subscriber: None,
        caster_frame: None,
    }
}

fn queued_death_event_group_settlement(
    death: crate::engine::manager::hp::DeathTransition,
    frame_path: &[usize],
) -> QueuedOp {
    let mut queued = queued_death_settlement(death, frame_path);
    queued.frame_path = queued.parent_path.take();
    queued.frame_owner = None;
    queued
}

fn queued_death_event_group_completion(
    death: crate::engine::manager::hp::PendingDeath,
    event: &crate::engine::event::payload::EntityDiedEvent,
    parent_path: &[usize],
) -> QueuedOp {
    QueuedOp {
        op: RuleOp::DeathEventGroupComplete(death),
        trigger: SkillOpTrigger::Event(BattleEvent::EntityDied(*event)),
        skill_execution: None,
        frame_path: None,
        parent_path: Some(parent_path.to_vec()),
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber: None,
        caster_frame: None,
    }
}

fn queued_terminal_settlement(event: BattleEvent, parent_path: &[usize]) -> QueuedOp {
    QueuedOp {
        op: RuleOp::TerminalSettlement,
        trigger: SkillOpTrigger::Event(event),
        skill_execution: None,
        frame_path: Some(parent_path.to_vec()),
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber: None,
        caster_frame: None,
    }
}

pub(super) fn attack_has_no_target(
    invocation: &crate::engine::skill::action::SkillInvocation,
    catalog: &SkillEffectCatalog,
    pool: &TargetPool,
    managers: &BattleManagers,
    determinism: &RoundDeterminism,
    context: TargetContext,
) -> bool {
    if invocation
        .phase
        .unwrap_or(crate::engine::skill::action::SkillPhase::Immediate)
        != crate::engine::skill::action::SkillPhase::Immediate
        || !catalog.is_attack(invocation.plan.skill_id)
    {
        return false;
    }
    let logic_target = match invocation.target {
        SkillTarget::LogicRule(code) => code,
        _ => catalog.logic_target(invocation.plan.skill_id),
    };
    match invocation.target {
        SkillTarget::Configured | SkillTarget::LogicRule(_)
            if targets_enemy(logic_target).is_some() =>
        {
            TargetResolver::resolve_primary_candidates(
                &TargetRequest {
                    code: logic_target,
                    raw: Vec::new(),
                },
                invocation.plan.skill_id,
                invocation.plan.source_uid,
                pool,
                determinism,
                Some(managers),
                TargetContext {
                    active_skill_is_attack: true,
                    active_skill_id: invocation.plan.skill_id,
                    active_skill_source_uid: invocation.plan.source_uid,
                    logic_target,
                    ..context
                },
            )
            .is_empty()
        }
        SkillTarget::Configured
        | SkillTarget::LogicRule(_)
        | SkillTarget::Inherited
        | SkillTarget::Explicit(_) => pool.enemies(invocation.plan.source_uid, false).is_empty(),
    }
}

#[derive(Default)]
struct ReactionBatch {
    before_publish: Vec<QueuedOp>,
    after_publish: Vec<QueuedOp>,
    after_settlement: Vec<QueuedOp>,
    after_skill: Vec<QueuedOp>,
    after_hit: Vec<QueuedOp>,
    before_after_hit_observers: Vec<QueuedOp>,
    after_hit_skills: Vec<QueuedOp>,
    shared_hit_skills: Vec<QueuedOp>,
    after_action: Vec<QueuedOp>,
}

pub use crate::engine::event::dispatcher::ReactionLane;

impl ReactionBatch {
    fn queued_mut(&mut self) -> impl Iterator<Item = &mut QueuedOp> {
        self.before_publish
            .iter_mut()
            .chain(&mut self.after_publish)
            .chain(&mut self.after_settlement)
            .chain(&mut self.after_skill)
            .chain(&mut self.after_hit)
            .chain(&mut self.before_after_hit_observers)
            .chain(&mut self.after_hit_skills)
            .chain(&mut self.shared_hit_skills)
            .chain(&mut self.after_action)
    }

    fn append(&mut self, mut other: Self) {
        self.before_publish.append(&mut other.before_publish);
        self.after_publish.append(&mut other.after_publish);
        self.after_settlement.append(&mut other.after_settlement);
        self.after_skill.append(&mut other.after_skill);
        self.after_hit.append(&mut other.after_hit);
        self.before_after_hit_observers
            .append(&mut other.before_after_hit_observers);
        self.after_hit_skills.append(&mut other.after_hit_skills);
        self.shared_hit_skills.append(&mut other.shared_hit_skills);
        self.after_action.append(&mut other.after_action);
    }

    fn into_ordered(self) -> Vec<QueuedOp> {
        self.before_publish
            .into_iter()
            .chain(self.after_publish)
            .chain(self.after_settlement)
            .chain(self.after_skill)
            .chain(self.after_action)
            .collect()
    }

    fn assign_subscriber_death_groups(&mut self, action_path: Option<&[usize]>) {
        if action_path.is_some() {
            assign_subscriber_death_groups(&mut [
                &mut self.before_publish,
                &mut self.after_publish,
                &mut self.after_settlement,
                &mut self.after_skill,
                &mut self.after_hit,
                &mut self.before_after_hit_observers,
                &mut self.after_hit_skills,
                &mut self.shared_hit_skills,
                &mut self.after_action,
            ]);
        } else {
            assign_subscriber_death_groups(&mut [
                &mut self.before_publish,
                &mut self.after_publish,
                &mut self.after_settlement,
                &mut self.after_skill,
                &mut self.after_action,
            ]);
        }
    }
}

fn assign_subscriber_death_groups(queues: &mut [&mut Vec<QueuedOp>]) {
    for queued in queues.iter_mut() {
        queued.retain(|op| !matches!(op.op, RuleOp::SubscriberGroupComplete));
    }
    let mut groups = HashMap::<i64, Rc<RefCell<SubscriberDeathGroup>>>::new();
    let mut latest = HashMap::<i64, (usize, usize)>::new();
    for (queue_index, queued) in queues.iter_mut().enumerate() {
        for (op_index, op) in queued.iter_mut().enumerate() {
            let Some(subscriber) = op.subscriber.as_mut() else {
                continue;
            };
            let group = groups.entry(subscriber.owner_uid).or_default().clone();
            subscriber.group = Some(group);
            latest.insert(subscriber.owner_uid, (queue_index, op_index));
        }
    }

    let mut completions = latest.into_iter().collect::<Vec<_>>();
    completions.sort_unstable_by_key(|(_, location)| std::cmp::Reverse(*location));
    for (owner_uid, (queue_index, op_index)) in completions {
        let after = &queues[queue_index][op_index];
        let subscriber = after
            .subscriber
            .as_ref()
            .expect("subscriber completion follows subscriber work")
            .clone();
        debug_assert_eq!(subscriber.owner_uid, owner_uid);
        let completion = QueuedOp {
            op: RuleOp::SubscriberGroupComplete,
            trigger: after.trigger.clone(),
            skill_execution: None,
            frame_path: None,
            parent_path: None,
            frame_group: None,
            independent_parent_group: None,
            frame_owner: None,
            subscriber: Some(subscriber),
            caster_frame: None,
        };
        queues[queue_index].insert(op_index + 1, completion);
    }
}

mod entry;
mod reactions;
mod state;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum DefenderCardCleanupOwner {
    #[default]
    DeathAction,
    ScheduledRoundPhase,
}

impl DefenderCardCleanupOwner {
    fn queues_in_drain(self, team_type: i32) -> bool {
        team_type == 1 || self == Self::DeathAction
    }
}

pub use entry::*;
#[cfg(test)]
use reactions::{
    dispatch_event_batch, ordered_hit_entities, queued_reactions, reaction_counterparty,
    reaction_skill_target,
};
use reactions::{dispatch_event_groups, dispatch_owner_reactions, dispatch_reactions};
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
                        queued_defeated_owner_buff_cleanup(managers, transition, parent_path)
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
                        queued_defeated_owner_buff_cleanup(managers, *death, parent_path)
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
            RuleOp::Skill(invocation) => {
                // After terminal commitment, skip active skills that have not
                // started a frame. Existing-frame continuations may finish settling.
                if matches!(trigger, SkillOpTrigger::Active)
                    && frame_path.is_none()
                    && managers.terminal_outcome().is_some()
                    && !invocation.mode.finishes_selected_conduit_phase()
                {
                    cancel_invocation_progress(managers, state, None, &invocation);
                    continue;
                }
                let attack_has_no_target = matches!(trigger, SkillOpTrigger::Active)
                    && attack_has_no_target(
                        &invocation,
                        catalog,
                        pool,
                        managers,
                        determinism,
                        context,
                    );
                if matches!(trigger, SkillOpTrigger::Active)
                    && base_pool.team_type(invocation.plan.source_uid).is_some()
                    && ((frame_path.is_none()
                        && base_pool.entity(invocation.plan.source_uid).is_some()
                        && pool.entity(invocation.plan.source_uid).is_none())
                        || attack_has_no_target)
                {
                    cancel_invocation_progress(managers, state, frame_path.as_ref(), &invocation);
                    continue;
                }
                if reaction_already_fired(state, &invocation) {
                    continue;
                }

                let parent_path = match caster_frame {
                    Some(caster) => Some(ensure_frame(
                        &mut result.frames,
                        None,
                        parent_path.as_deref(),
                        caster,
                        &trigger,
                    )),
                    None => parent_path,
                };
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
                if matches!(trigger, SkillOpTrigger::Event(_))
                    && let Some(action_path) = active_skill_scope_path(&result.frames, &frame_path)
                    && let FrameOwner::Skill {
                        source_uid,
                        skill_id,
                        ..
                    } = owner_at_path(&result.frames, &frame_path)
                {
                    state.action_skill_reaction_frame(
                        &action_path,
                        *source_uid,
                        *skill_id,
                        &frame_path,
                    );
                }
                if !skill_from_buff_act && let Some(group) = &frame_group {
                    *group.borrow_mut() = Some(frame_path.clone());
                }
                if invocation.phase.is_none()
                    && matches!(trigger, SkillOpTrigger::Active)
                    && frame_path.len() > 1
                    && matches!(
                        owner_at_path(&result.frames, &frame_path[..frame_path.len() - 1]),
                        FrameOwner::Skill { .. }
                    )
                {
                    crate::engine::runtime::record::push_frame_cue(
                        &mut result.frames,
                        &frame_path,
                        crate::engine::runtime::record::RoundCue::CastStep,
                    );
                }
                let mut defeated_owner_card_cleanups = Vec::new();
                if matches!(trigger, SkillOpTrigger::Active)
                    && invocation.phase
                        == Some(crate::engine::skill::action::SkillPhase::AfterDamage)
                    && !skill_execution
                        .as_ref()
                        .is_some_and(|execution| execution.resumes_phase())
                    && let Some(deaths) = state.take_deaths(&frame_path)
                {
                    let deaths = deaths
                        .into_iter()
                        .filter(|death| managers.hp.is_current_death(*death))
                        .map(crate::engine::manager::hp::PendingDeath::transition)
                        .collect::<Vec<_>>();
                    for death in deaths {
                        let cleanup =
                            queued_defeated_owner_buff_cleanup(managers, death, &frame_path);
                        drain_defeated_owner_buff_cleanup(
                            managers,
                            pool,
                            catalog,
                            determinism,
                            cleanup,
                            &mut result,
                            state,
                        )?;
                        push_change(
                            &mut result.frames,
                            &frame_path,
                            crate::engine::runtime::change::BattleChange::Death(death),
                        );
                        defeated_owner_card_cleanups.push(death);
                    }
                }
                let mut execution = skill_execution.unwrap_or_else(|| SkillExecution::new(context));
                if matches!(trigger, SkillOpTrigger::Event(_)) {
                    execution.set_causing_skill_target(
                        active_skill_target_uid(&result.frames, &frame_path).unwrap_or_default(),
                    );
                }
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

                release_invocation_progress(managers, &invocation);
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
                let configured_target_replaces_selection =
                    matches!(trigger, SkillOpTrigger::Active)
                        && execution.configured_target_replaces(
                            invocation.target,
                            invocation.plan.source_uid,
                            pool,
                        );
                let preserves_selected_frame_target = execution.preserves_selected_frame_target(
                    invocation.target,
                    invocation.plan.source_uid,
                    pool,
                );
                for &(slot_index, condition_key) in &emission.fired_rules {
                    managers.mark_rule_fired(
                        invocation.plan.source_uid,
                        invocation.plan.skill_id,
                        slot_index,
                        condition_key,
                    );
                    mark_reaction_fired(state, &invocation, slot_index, condition_key);
                }
                set_skill_target(
                    &mut result.frames,
                    &frame_path,
                    emission.target_uid,
                    configured_target_replaces_selection
                        || matches!(trigger, SkillOpTrigger::Active)
                            && invocation.phase == Some(SkillPhase::Damage)
                            && !preserves_selected_frame_target,
                );
                // Dispatch settles the action mode, so open the action from what it emitted.
                let completes_action = emission
                    .continuation
                    .as_ref()
                    .is_some_and(|continuation| continuation.mode.completes_action())
                    || emission.ops.iter().any(|emitted| {
                        matches!(
                            emitted.op,
                            RuleOp::SkillLifecycle(
                                crate::engine::skill::action::SkillLifecycle::ActionCompleted(_)
                            )
                        )
                    });
                if completes_action {
                    state.open_action(frame_path.clone());
                    if let Some(link) = &reaction_link {
                        state.claim_action_link(&frame_path, link);
                    }
                    if let Some(group) = &frame_group {
                        for (caster, mut queued) in state.take_after_reaction_casts(group) {
                            queued.parent_path = Some(frame_path.clone());
                            let caster = caster.map(|caster| {
                                join_caster_to_action(&result.frames, &frame_path, caster)
                            });
                            state.push_after_action_cast(frame_path.clone(), caster, queued);
                        }
                    }
                } else if let Some(link) = &reaction_link {
                    state.defer_reaction_child_link(frame_path.clone(), link);
                }
                let mut outputs = defeated_owner_card_cleanups
                    .into_iter()
                    .filter_map(|death| {
                        queued_defeated_owner_card_cleanup(
                            pool,
                            managers,
                            death,
                            &frame_path,
                            state.defender_card_cleanup_owner(),
                        )
                    })
                    .collect::<Vec<_>>();
                for emission in emission.ops {
                    let skill::SkillEmissionOp {
                        op,
                        owner,
                        consequence,
                        frame_owner,
                    } = emission;
                    let op = attach_buff_grant_relation(op, consequence);
                    if let RuleOp::SkillLifecycle(
                        crate::engine::skill::action::SkillLifecycle::ActionCompleted(event),
                    ) = &op
                        && let Some(completion) = take_conduit_activation_completion(
                            queue,
                            event.source_uid,
                            event.skill_id,
                        )
                    {
                        outputs.push(completion);
                    }
                    match op {
                        RuleOp::Skill(child) => {
                            let after_current_action = child.start.waits_for_current_action();
                            let mut child_execution = SkillExecution::new(context);
                            child_execution.inherit_assassination_rate_marks(&execution);
                            let mut queued = QueuedOp {
                                op: RuleOp::Skill(child),
                                trigger: SkillOpTrigger::Active,
                                skill_execution: Some(child_execution),
                                frame_path: None,
                                parent_path: Some(frame_path.clone()),
                                frame_group: None,
                                independent_parent_group: None,
                                frame_owner: None,
                                subscriber: subscriber.clone(),
                                caster_frame: None,
                            };
                            // Like manager follow-ups, wait for the enclosing action; with no
                            // action in progress the cast runs now.
                            match after_current_action
                                .then(|| active_skill_scope_path(&result.frames, &frame_path))
                                .flatten()
                                .filter(|action_path| state.action_in_progress(action_path))
                            {
                                Some(action_path) => {
                                    let caster =
                                        casting_reaction(&result.frames, &action_path, &frame_path);
                                    queued.parent_path = Some(action_path.clone());
                                    state.push_after_action_cast(action_path, caster, queued)
                                }
                                None => outputs.push(queued),
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
                                subscriber: subscriber.clone(),
                                caster_frame: None,
                            },
                            None => QueuedOp {
                                op: command,
                                trigger: trigger.clone(),
                                skill_execution: None,
                                frame_path: Some(if owner
                                    == crate::engine::skill::behavior::registry::OutputOwner::ActionSkillReaction
                                {
                                    let action_path = active_skill_scope_path(
                                        &result.frames,
                                        &frame_path,
                                    )
                                    .ok_or(DrainError::MissingActiveSkillContext)?;
                                    let FrameOwner::Skill {
                                        source_uid,
                                        skill_id,
                                        ..
                                    } = owner_at_path(&result.frames, &frame_path)
                                    else {
                                        return Err(DrainError::MissingActiveSkillContext);
                                    };
                                    state.action_skill_reaction_frame(
                                        &action_path,
                                        *source_uid,
                                        *skill_id,
                                        &frame_path,
                                    )
                                } else {
                                    output_frame_path(owner, &frame_path)
                                }),
                                parent_path: None,
                                frame_group: None,
                                independent_parent_group: None,
                                frame_owner: None,
                                subscriber: subscriber.clone(),
                                caster_frame: None,
                            },
                        }),
                    }
                }
                if let Some(continuation) = emission.continuation {
                    outputs.push(QueuedOp {
                        op: RuleOp::Skill(continuation),
                        trigger,
                        skill_execution: Some(execution),
                        frame_path: Some(frame_path.clone()),
                        parent_path: None,
                        frame_group: None,
                        independent_parent_group: None,
                        frame_owner: None,
                        subscriber: subscriber.clone(),
                        caster_frame: None,
                    });
                }
                prepend(queue, outputs);
            }
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
            mut command @ (RuleOp::Command(_)
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
                        subscriber,
                        caster_frame,
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
                    for _ in managers.card.redealable_cards() {
                        replacements.extend(determinism.draw_cards(&draw_pile, 1));
                    }
                    if crate::engine::fight::versions::round_start_setup_layout(
                        managers.fight_version(),
                    ) == Some(crate::engine::fight::versions::RoundStartSetupLayout::Version7)
                    {
                        let current_pool = pool.runtime_view(managers);
                        let basics =
                            crate::engine::manager::card::pool::target_basic_candidate_pool(
                                &current_pool.attacker_main,
                            );
                        if basics.len() == current_pool.attacker_main.len().saturating_mul(2) {
                            crate::engine::manager::card::start::preserve_basic_coverage(
                                &basics,
                                &mut replacements,
                            );
                        }
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

                let entry_halo_fanout = matches!(
                    &command,
                    RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Buff(
                        crate::engine::manager::buff::BuffCommand::FanoutEntryHalos(_)
                    ))
                );

                // This is the single durable commit point for non-skill RuleOps.
                // The returned outcome describes committed changes and follow-ups.
                let mut outcome = execute_rule_op_with(managers, &mut bus, determinism, command)?;
                let transformed = match &outcome {
                    RuleOutcome::Entity(changes)
                        if matches!(
                            changes.operation,
                            crate::engine::manager::entity::EntityOperation::Transform { .. }
                        ) =>
                    {
                        Some(changes.as_ref())
                    }
                    RuleOutcome::MonsterChange(changes) => Some(&changes.entity),
                    _ => None,
                };
                if let Some(changes) = transformed {
                    let current_pool = base_pool.runtime_view(managers);
                    let active_skills =
                        crate::engine::skill::subscriber::active_skills(&current_pool, managers);
                    state.retire_replaced_subscriber_casts(
                        changes.target_uid,
                        active_skills.iter().filter_map(|(owner_uid, skill_id)| {
                            (*owner_uid == changes.target_uid).then_some(*skill_id)
                        }),
                    );
                }
                if let RuleOutcome::ActiveSkillTargetsModified(additional_count) = outcome {
                    let action_scope = action_scope
                        .clone()
                        .ok_or(DrainError::MissingActiveSkillContext)?;
                    state.add_target_modifier(action_scope, additional_count);
                }
                let followups = outcome.followups();
                let damage_amount = outcome.applied_damage();
                let death_count = outcome.death_count();
                let guard_break_count = outcome.guard_break_count();
                let injured_targets = outcome.injured_targets();
                if !injured_targets.is_empty()
                    && let Some(action_path) = active_skill_scope_path(&result.frames, &frame_path)
                {
                    state.record_injuries(action_path, &injured_targets);
                }
                if (damage_amount > 0
                    || death_count > 0
                    || guard_break_count > 0
                    || !injured_targets.is_empty())
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
                        execution.record_guard_breaks(guard_break_count);
                        execution.record_injuries(allied_injuries);
                    }
                }
                let event_scope = event_scope_path(&result.frames, &frame_path);
                let entry_halo_scope = entity_entry_halo_scope_path(&result.frames, &frame_path);
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
                    }
                    | FrameOwner::ConduitSkill {
                        source_uid,
                        skill_id,
                        target_uid,
                        ..
                    } => Some((*source_uid, *skill_id, *target_uid)),
                    _ => None,
                };
                let fanout_output_owner = if entry_halo_fanout || current_skill.is_none() {
                    crate::engine::skill::behavior::registry::OutputOwner::Skill
                } else {
                    crate::engine::skill::behavior::registry::OutputOwner::CausingEvent.resolve(
                        match trigger {
                            SkillOpTrigger::Event(ref event) => Some(event.kind()),
                            _ => None,
                        },
                        false,
                    )
                };

                // Managers publish semantic events through the bus. Reactions are
                // derived from these committed events, never fired by packet code.
                let mut events = std::iter::from_fn(|| bus.pop()).collect::<Vec<_>>();
                let mut event_groups = match &outcome {
                    RuleOutcome::HpBatch(batch) => grouped_hp_events(
                        &events,
                        batch
                            .iter()
                            .map(|execution| execution.changes.events())
                            .collect(),
                    ),
                    _ => vec![events.clone()],
                };
                if matches!(trigger, SkillOpTrigger::Active)
                    && let Some(queued) = queue.iter_mut().find(|queued| {
                        matches!(queued.trigger, SkillOpTrigger::Active)
                            && queued.frame_path.as_ref() == Some(&frame_path)
                            && matches!(queued.op, RuleOp::Skill(_))
                    })
                    && let Some(execution) = queued.skill_execution.as_mut()
                {
                    execution.record_attacks(events.iter().filter_map(|event| {
                        let BattleEvent::Hit(hit) = event else {
                            return None;
                        };
                        matches!(
                            hit.damage_from,
                            crate::engine::manager::hp::HurtDamageFromType::Skill
                                | crate::engine::manager::hp::HurtDamageFromType::SkillEffect
                        )
                        .then_some(hit.target_uid)
                    }));
                    let source_uid = current_skill.map(|skill| skill.0).unwrap_or_default();
                    execution.record_buff_additions(events.iter().filter_map(|event| {
                        let (BattleEvent::BuffAdded(change) | BattleEvent::BuffChanged(change)) =
                            event
                        else {
                            return None;
                        };
                        (change.source_uid == source_uid).then_some((
                            change.buff_id,
                            change.after_amount.saturating_sub(change.before_amount),
                        ))
                    }));
                }

                let action_scope = active_skill_scope_path(&result.frames, &frame_path);
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
                    // primary hits and their own HP loss. Their BeforePublish
                    // buff acts still belong before this HP outcome.
                    let (immediate, deferred) = split_active_hit_event_groups(event_groups);
                    let hit_buff_acts = dispatch_event_groups(
                        pool,
                        managers,
                        catalog,
                        determinism,
                        state,
                        &deferred,
                        &event_scope,
                        &frame_path,
                        action_scope.as_deref(),
                        &entry_halo_scope,
                        current_skill,
                        true,
                        context.current_round > 0,
                        crate::engine::event::subscription::PublicationPhase::BeforePublish,
                        None,
                        Some(ReactionLane::BuffActs),
                        None,
                    )?;
                    drain_nested_queue(
                        managers,
                        pool,
                        catalog,
                        determinism,
                        hit_buff_acts.into_ordered().into(),
                        &mut result,
                        state,
                    )?;
                    pending_hits
                        .entry(frame_path.clone())
                        .or_default()
                        .extend(deferred);
                    event_groups = immediate;
                    events = flatten_event_groups(&event_groups);
                }

                // BeforePublish reactions run before this outcome is recorded in
                // its semantic frame. Their own manager commits use nested drains.
                let mut before_reactions = dispatch_event_groups(
                    pool,
                    managers,
                    catalog,
                    determinism,
                    state,
                    &event_groups,
                    &event_scope,
                    &frame_path,
                    action_scope.as_deref(),
                    &entry_halo_scope,
                    current_skill,
                    matches!(&outcome, RuleOutcome::HpBatch(_)),
                    context.current_round > 0,
                    crate::engine::event::subscription::PublicationPhase::BeforePublish,
                    None,
                    None,
                    None,
                )?;
                let mut started_after_reactions = None;
                if matches!(&outcome, RuleOutcome::SkillActionStarted { .. }) {
                    let mut started_reactions = dispatch_event_groups(
                        pool,
                        managers,
                        catalog,
                        determinism,
                        state,
                        &event_groups,
                        &event_scope,
                        &frame_path,
                        action_scope.as_deref(),
                        &entry_halo_scope,
                        current_skill,
                        matches!(&outcome, RuleOutcome::HpBatch(_)),
                        context.current_round > 0,
                        crate::engine::event::subscription::PublicationPhase::AfterPublish,
                        None,
                        None,
                        None,
                    )?;
                    before_reactions
                        .before_publish
                        .append(&mut started_reactions.after_publish);
                    before_reactions.assign_subscriber_death_groups(action_scope.as_deref());
                    started_after_reactions = Some(started_reactions);
                }
                result.events.extend(events.iter().cloned());
                let releases_pending_hits = matches!(
                    &outcome,
                    RuleOutcome::SkillLifecycle(
                        crate::engine::skill::action::SkillLifecycle::PhaseCompleted(event)
                    ) if matches!(
                        event.phase,
                        crate::engine::skill::action::SkillPhase::AdditionalDamage
                            | crate::engine::skill::action::SkillPhase::HitPassives
                    )
                );
                let releases_hit_passive_skills = matches!(
                    &outcome,
                    RuleOutcome::SkillLifecycle(
                        crate::engine::skill::action::SkillLifecycle::PhaseCompleted(event)
                    ) if event.phase == crate::engine::skill::action::SkillPhase::HitPassives
                );
                let releases_after_hit_skills = matches!(
                    &outcome,
                    RuleOutcome::SkillLifecycle(
                        crate::engine::skill::action::SkillLifecycle::PhaseCompleted(event)
                    ) if event.phase == crate::engine::skill::action::SkillPhase::AfterHit
                );
                let mut pending_hit_skills = ReactionBatch::default();
                let released_hit_events = releases_pending_hits
                    .then(|| pending_hits.remove(&frame_path))
                    .flatten();
                if let Some(hit_event_groups) = released_hit_events.as_ref() {
                    let hit_events = flatten_event_groups(hit_event_groups);
                    let hit_skills = dispatch_event_groups(
                        pool,
                        managers,
                        catalog,
                        determinism,
                        state,
                        hit_event_groups,
                        &event_scope,
                        &frame_path,
                        action_scope.as_deref(),
                        &entry_halo_scope,
                        current_skill,
                        true,
                        context.current_round > 0,
                        crate::engine::event::subscription::PublicationPhase::BeforePublish,
                        None,
                        Some(ReactionLane::Skills),
                        None,
                    )?;
                    result.events.extend(hit_events.iter().cloned());
                    pending_hit_skills = hit_skills;
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
                    || !pending_hit_skills.shared_hit_skills.is_empty()
                {
                    let shared_hit_skills =
                        std::mem::take(&mut pending_hit_skills.shared_hit_skills);
                    state.defer_hit_skills(action_scope.as_deref(), shared_hit_skills);
                    state.defer_hit_passive_skills(
                        action_scope.as_deref(),
                        pending_hit_skills.into_ordered(),
                    );
                }

                // Once pre-publication work is complete, record the authoritative
                // outcome and any buff fanout beneath their semantic owners.
                let settles_death_immediately = outcome.settles_death_immediately();
                let mut pending_deaths = outcome.take_deaths();
                let settlement_action_scope = action_scope.as_ref().filter(|path| {
                    matches!(trigger, SkillOpTrigger::Active) || state.action_in_progress(path)
                });
                let active_action_scope = (!settles_death_immediately)
                    .then_some(settlement_action_scope)
                    .flatten();
                let original_pending_deaths = pending_deaths.clone();
                let redeal_hand = match &outcome {
                    RuleOutcome::Card(changes)
                        if changes.kind
                            == crate::engine::manager::card::CardChangeKind::RedealtKeepRanks =>
                    {
                        Some(changes.after.clone())
                    }
                    _ => None,
                };
                let changes = outcome.changes();
                for change in changes {
                    push_change(&mut result.frames, &frame_path, change);
                }
                if let Some(cards) = redeal_hand {
                    push_cue(&mut result.frames, RoundCue::RedealHandSync { cards });
                }
                for (owner, change) in outcome.owned_changes() {
                    if owner
                        == crate::engine::skill::behavior::registry::OutputOwner::NestedLifecycleMechanic
                    {
                        let nested = push_child(
                            &mut result.frames,
                            &frame_path,
                            FrameOwner::LifecycleMechanic,
                            frame_trigger(&trigger),
                        );
                        push_change(&mut result.frames, &nested, change);
                    } else {
                        push_change(
                            &mut result.frames,
                            &output_frame_path(owner, &frame_path),
                            change,
                        );
                    }
                }
                for fanout in fanout {
                    let fanout_parent = output_frame_path(fanout_output_owner, &event_scope);
                    let fanout_path = push_child(
                        &mut result.frames,
                        &fanout_parent,
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

                let terminal_owner_uids = managers
                    .terminal_outcome()
                    .and_then(|outcome| outcome.winning_team())
                    .map(|winning_team| base_pool.team_uids(winning_team));

                // AfterPublish reactions are partitioned by their declared release
                // lane; after-hit and after-action work stays action-scoped in state.
                let mut reactions = if was_skill_action_started {
                    started_after_reactions.unwrap_or_default()
                } else {
                    dispatch_event_groups(
                        pool,
                        managers,
                        catalog,
                        determinism,
                        state,
                        &event_groups,
                        &event_scope,
                        &frame_path,
                        action_scope.as_deref(),
                        &entry_halo_scope,
                        current_skill,
                        was_hp_batch,
                        context.current_round > 0,
                        crate::engine::event::subscription::PublicationPhase::AfterPublish,
                        terminal_owner_uids.as_deref(),
                        None,
                        active_action_scope
                            .is_some()
                            .then_some(pending_deaths.as_slice()),
                    )?
                };
                state.defer_before_after_hit_observers(
                    action_scope.as_deref(),
                    std::mem::take(&mut reactions.before_after_hit_observers),
                );
                state.defer_after_hit_skills(
                    action_scope.as_deref(),
                    std::mem::take(&mut reactions.after_hit_skills),
                );
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
                if let Some(hit_event_groups) = released_hit_events.as_ref() {
                    let mut hit_buff_acts = dispatch_event_groups(
                        pool,
                        managers,
                        catalog,
                        determinism,
                        state,
                        hit_event_groups,
                        &event_scope,
                        &frame_path,
                        action_scope.as_deref(),
                        &entry_halo_scope,
                        current_skill,
                        true,
                        context.current_round > 0,
                        crate::engine::event::subscription::PublicationPhase::AfterPublish,
                        terminal_owner_uids.as_deref(),
                        Some(ReactionLane::BuffActs),
                        None,
                    )?;
                    let mut hit_skills = dispatch_event_groups(
                        pool,
                        managers,
                        catalog,
                        determinism,
                        state,
                        hit_event_groups,
                        &event_scope,
                        &frame_path,
                        action_scope.as_deref(),
                        &entry_halo_scope,
                        current_skill,
                        true,
                        context.current_round > 0,
                        crate::engine::event::subscription::PublicationPhase::AfterPublish,
                        terminal_owner_uids.as_deref(),
                        Some(ReactionLane::Skills),
                        None,
                    )?;
                    state.defer_before_after_hit_observers(
                        action_scope.as_deref(),
                        std::mem::take(&mut hit_buff_acts.before_after_hit_observers),
                    );
                    state.defer_after_hit_skills(
                        action_scope.as_deref(),
                        std::mem::take(&mut hit_buff_acts.after_hit_skills),
                    );
                    state.defer_after_hit(
                        action_scope.as_deref(),
                        std::mem::take(&mut hit_skills.after_hit),
                    );
                    let hit_after_action = std::mem::take(&mut hit_buff_acts.after_action);
                    if action_scope.is_some() {
                        state.defer_after_action(action_scope.as_deref(), hit_after_action);
                    } else {
                        hit_buff_acts.after_publish.extend(hit_after_action);
                    }
                    let shared_hit_skills = std::mem::take(&mut hit_skills.shared_hit_skills);
                    state.defer_hit_skills(action_scope.as_deref(), shared_hit_skills);
                    state.defer_hit_passive_skills(
                        action_scope.as_deref(),
                        hit_skills.into_ordered(),
                    );
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
                let mut after_publish = reactions.after_publish;
                let after_settlement = reactions.after_settlement;
                if releases_hit_passive_skills {
                    let mut released_hit_passives = Vec::new();
                    if let Some(hit_skills) = state.take_hit_passive_skills(action_scope.as_ref()) {
                        released_hit_passives.extend(hit_skills);
                    }
                    let before_observers = state
                        .take_before_after_hit_observers(action_scope.as_ref())
                        .unwrap_or_default()
                        .into_iter();
                    released_hit_passives.extend(before_observers);
                    after_publish.splice(0..0, released_hit_passives);
                }
                if releases_after_hit_skills {
                    let hit_skills = state
                        .take_hit_skills(action_scope.as_ref())
                        .unwrap_or_default();
                    let observers = state
                        .take_after_hit_skills(action_scope.as_ref())
                        .unwrap_or_default();
                    after_publish.extend(hit_skills);
                    after_publish.extend(observers);
                }
                // Death-sensitive reactions declared before settlement get one
                // chance to change HP before death transitions are finalized.
                if managers.terminal_outcome().is_none() && !pending_deaths.is_empty() {
                    drain_death_settlement_queue(
                        managers,
                        pool,
                        catalog,
                        determinism,
                        after_publish.into(),
                        &mut result,
                        state,
                    )?;
                    after_publish = after_settlement;
                } else {
                    after_publish.extend(after_settlement);
                }

                // A pending death becomes a frame change only if HP is still zero.
                // Active-action deaths wait for the shared HitPassives release.
                let completed_death_event_groups =
                    state.take_completed_death_event_groups(&original_pending_deaths);
                pending_deaths.retain(|death| !completed_death_event_groups.contains(death));
                let event_grouped_deaths = completed_death_event_groups
                    .into_iter()
                    .filter(|death| managers.hp.is_current_death(*death))
                    .collect::<Vec<_>>();
                let settled_deaths = pending_deaths
                    .into_iter()
                    .filter(|death| managers.hp.is_current_death(*death))
                    .collect::<Vec<_>>();
                if settles_death_immediately
                    && let Some(source_uid) = current_skill.map(|skill| skill.0)
                    && settled_deaths
                        .iter()
                        .any(|death| death.transition().target_uid == source_uid)
                    && let Some(execution) = queue
                        .iter_mut()
                        .find(|queued| {
                            matches!(queued.trigger, SkillOpTrigger::Active)
                                && queued.frame_path.as_ref() == Some(&frame_path)
                                && matches!(queued.op, RuleOp::Skill(_))
                        })
                        .and_then(|queued| queued.skill_execution.as_mut())
                {
                    execution.record_immediate_source_death();
                }
                let mut terminal_death = settled_deaths
                    .first()
                    .or_else(|| event_grouped_deaths.first())
                    .map(|death| death.transition());
                let mut cleanup_deaths = event_grouped_deaths
                    .iter()
                    .map(|death| death.transition())
                    .collect::<Vec<_>>();
                let mut non_action_deaths = Vec::new();
                if settles_death_immediately && let Some(action_scope) = settlement_action_scope {
                    for death in &settled_deaths {
                        let cleanup = queued_defeated_owner_buff_cleanup(
                            managers,
                            death.transition(),
                            action_scope,
                        );
                        drain_defeated_owner_buff_cleanup(
                            managers,
                            pool,
                            catalog,
                            determinism,
                            cleanup,
                            &mut result,
                            state,
                        )?;
                        push_change(
                            &mut result.frames,
                            action_scope,
                            crate::engine::runtime::change::BattleChange::Death(death.transition()),
                        );
                    }
                    cleanup_deaths.extend(settled_deaths.iter().map(|death| death.transition()));
                } else if let Some(action_scope) = active_action_scope {
                    state.record_deaths(action_scope.clone(), settled_deaths.iter().copied());
                } else {
                    cleanup_deaths.extend(settled_deaths.iter().map(|death| death.transition()));
                    if let Some(group) = subscriber
                        .as_ref()
                        .and_then(|subscriber| subscriber.group.as_ref())
                    {
                        state.record_subscriber_deaths(
                            group,
                            &event_scope,
                            settled_deaths.iter().copied(),
                        );
                        terminal_death = None;
                    } else {
                        non_action_deaths
                            .extend(settled_deaths.iter().map(|death| death.transition()));
                    }
                }
                if releases_pending_hits && let Some(action_scope) = active_action_scope {
                    let deaths = state
                        .take_deaths(action_scope)
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|death| managers.hp.is_current_death(*death))
                        .map(crate::engine::manager::hp::PendingDeath::transition)
                        .collect::<Vec<_>>();
                    for death in &deaths {
                        let cleanup =
                            queued_defeated_owner_buff_cleanup(managers, *death, action_scope);
                        drain_defeated_owner_buff_cleanup(
                            managers,
                            pool,
                            catalog,
                            determinism,
                            cleanup,
                            &mut result,
                            state,
                        )?;
                        push_change(
                            &mut result.frames,
                            action_scope,
                            crate::engine::runtime::change::BattleChange::Death(*death),
                        );
                    }
                    terminal_death = terminal_death.or_else(|| deaths.first().copied());
                    cleanup_deaths.extend(deaths);
                }
                let death_count = settled_deaths.len() + event_grouped_deaths.len();
                if death_count != 0
                    && matches!(trigger, SkillOpTrigger::Active)
                    && let Some(queued) = queue.iter_mut().find(|queued| {
                        matches!(queued.trigger, SkillOpTrigger::Active)
                            && queued.frame_path.as_ref() == Some(&frame_path)
                            && matches!(queued.op, RuleOp::Skill(_))
                    })
                    && let Some(execution) = queued.skill_execution.as_mut()
                {
                    execution.record_kills(death_count as i32);
                }
                let cleanup_parent_path = settlement_action_scope
                    .map(Vec::as_slice)
                    .unwrap_or(event_scope.as_slice());
                after_publish.extend(cleanup_deaths.into_iter().filter_map(|death| {
                    queued_defeated_owner_card_cleanup(
                        pool,
                        managers,
                        death,
                        cleanup_parent_path,
                        state.defender_card_cleanup_owner(),
                    )
                }));
                let mut death_settlements = Vec::new();
                for death in non_action_deaths {
                    if let Some(cleanup) =
                        queued_defeated_owner_buff_cleanup(managers, death, &event_scope)
                    {
                        death_settlements.push(cleanup);
                    }
                    death_settlements.push(queued_death_settlement(death, &event_scope));
                }
                after_publish.extend(death_settlements);
                if let Some(death) = terminal_death {
                    after_publish.push(queued_terminal_settlement(
                        BattleEvent::EntityDied(crate::engine::event::payload::EntityDiedEvent {
                            source_uid: death.source_uid,
                            target_uid: death.target_uid,
                        }),
                        &event_scope,
                    ));
                }
                let (after_action, held_casts, claimed_casts) = if completes_action {
                    let casts = state.take_after_action_casts(&frame_path);
                    let claimed = state.take_claimed_reaction_casts(&frame_path);
                    (state.take_after_action(&frame_path), casts, claimed)
                } else {
                    (Vec::new(), Vec::new(), Vec::new())
                };
                let (mut reaction_casts, held_casts): (Vec<_>, Vec<_>) =
                    held_casts.into_iter().partition(|(_, queued)| {
                        matches!(
                            &queued.op,
                            RuleOp::Skill(invocation)
                                if invocation.start.follows_action_reaction()
                        )
                    });
                let action_reactions = if reaction_casts.is_empty() {
                    Vec::new()
                } else {
                    ally_action_reaction_groups(&after_publish)
                };
                if !action_reactions.is_empty() {
                    let link = DrainState::reaction_cast_link(std::mem::take(&mut reaction_casts));
                    for group in &action_reactions {
                        state.defer_action_reaction_link(group, &link);
                    }
                    drain_nested_queue(
                        managers,
                        pool,
                        catalog,
                        determinism,
                        std::mem::take(&mut after_publish).into(),
                        &mut result,
                        state,
                    )?;
                    reaction_casts = DrainState::take_unclaimed_reaction_casts(&link);
                }
                if !reaction_casts.is_empty()
                    && let Some(group) = after_action
                        .iter()
                        .rev()
                        .chain(reactions.after_skill.iter().rev())
                        .find_map(|queued| {
                            matches!(queued.op, RuleOp::Skill(_))
                                .then(|| queued.frame_group.clone())
                                .flatten()
                        })
                {
                    state.defer_after_reaction_casts(&group, std::mem::take(&mut reaction_casts));
                }
                let held_casts = held_casts
                    .into_iter()
                    .chain(reaction_casts)
                    .chain(claimed_casts)
                    .map(|(caster, mut queued)| {
                        queued.caster_frame = caster;
                        queued
                    });

                // Manager-produced follow-ups re-enter the same queue. Skills marked
                // AfterCurrentAction are retained until that action closes.
                let mut immediate_followups = Vec::new();
                for (op, owner) in followups {
                    let after_current_action = matches!(
                        &op,
                        RuleOp::Skill(invocation) if invocation.start.waits_for_current_action()
                    );
                    let skill_path = after_current_action
                        .then(|| active_skill_scope_path(&result.frames, &frame_path))
                        .flatten()
                        .filter(|action_path| state.action_in_progress(action_path));
                    let (followup_frame_path, followup_parent_path) = match owner {
                        crate::engine::skill::behavior::registry::OutputOwner::Skill => (
                            None,
                            skill_path.clone().or_else(|| Some(frame_path.clone())),
                        ),
                        crate::engine::skill::behavior::registry::OutputOwner::Parent => {
                            (Some(output_frame_path(owner, &frame_path)), None)
                        }
                        crate::engine::skill::behavior::registry::OutputOwner::CausingEvent
                        | crate::engine::skill::behavior::registry::OutputOwner::ActionSkillReaction
                        | crate::engine::skill::behavior::registry::OutputOwner::NestedLifecycleMechanic
                        | crate::engine::skill::behavior::registry::OutputOwner::SetupParent
                        | crate::engine::skill::behavior::registry::OutputOwner::SetupOrFinalSettlementParent => {
                            unreachable!("manager follow-up owner must be resolved")
                        }
                    };
                    let queued = QueuedOp {
                        op,
                        trigger: SkillOpTrigger::Active,
                        skill_execution: None,
                        frame_path: followup_frame_path,
                        parent_path: followup_parent_path,
                        frame_group: None,
                        independent_parent_group: None,
                        frame_owner: deferred_followup_owner.clone(),
                        subscriber: subscriber.clone(),
                        caster_frame: None,
                    };
                    if let Some(skill_path) = skill_path {
                        let caster = deferred_followup_owner
                            .is_none()
                            .then(|| casting_reaction(&result.frames, &skill_path, &frame_path))
                            .flatten();
                        state.push_after_action_cast(skill_path, caster, queued);
                    } else {
                        immediate_followups.push(queued);
                    }
                }
                prepend(queue, immediate_followups);
                insert_after_frame(queue, &frame_path, reactions.after_skill);
                if completes_action {
                    insert_after_frame(
                        queue,
                        &frame_path,
                        [QueuedOp {
                            op: RuleOp::ImmediateReservePromotion,
                            trigger: SkillOpTrigger::Active,
                            skill_execution: None,
                            frame_path: Some(frame_path.clone()),
                            parent_path: None,
                            frame_group: None,
                            independent_parent_group: None,
                            frame_owner: None,
                            subscriber: None,
                            caster_frame: None,
                        }],
                    );
                }
                // Buff acts waiting on the action, then held casts, follow its own reactions.
                insert_after_frame(queue, &frame_path, after_action);
                insert_after_frame(queue, &frame_path, held_casts);
                prepend(queue, after_publish);
            }
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

fn drain_defeated_owner_buff_cleanup(
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
