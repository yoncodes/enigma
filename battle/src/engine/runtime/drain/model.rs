use super::*;

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
    MissingCardStateTeam(i64),
}

pub(super) fn required_buff_act_definition(
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

pub(super) const MAX_DRAIN_OPERATIONS: usize = 16_384;
pub(super) const MAX_DRAIN_DEPTH: usize = 64;

pub(super) struct DrainBudget {
    pub(super) remaining_operations: usize,
}

impl Default for DrainBudget {
    fn default() -> Self {
        Self {
            remaining_operations: MAX_DRAIN_OPERATIONS,
        }
    }
}

impl DrainBudget {
    pub(super) fn consume(&mut self, depth: usize) -> Result<(), DrainError> {
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

pub(super) struct QueuedOp {
    pub(super) op: RuleOp,
    pub(super) trigger: SkillOpTrigger,
    pub(super) skill_execution: Option<SkillExecution>,
    pub(super) frame_path: Option<FramePath>,
    pub(super) parent_path: Option<FramePath>,
    pub(super) frame_group: Option<Rc<RefCell<Option<FramePath>>>>,
    pub(super) independent_parent_group: Option<Rc<RefCell<Option<FramePath>>>>,
    pub(super) frame_owner: Option<FrameOwner>,
    pub(super) subscriber: Option<SubscriberContext>,
    // The reaction step that cast a held skill, recreated when the skill runs.
    pub(super) caster_frame: Option<FrameOwner>,
}

#[derive(Clone)]
pub(super) struct SubscriberContext {
    pub(super) owner_uid: i64,
    pub(super) skill_id: Option<i32>,
    pub(super) group: Option<Rc<RefCell<SubscriberDeathGroup>>>,
}

impl SubscriberContext {
    pub(super) fn new(owner_uid: i64) -> Self {
        Self {
            owner_uid,
            skill_id: None,
            group: None,
        }
    }

    pub(super) fn for_skill(owner_uid: i64, skill_id: i32) -> Self {
        Self {
            owner_uid,
            skill_id: Some(skill_id),
            group: None,
        }
    }
}

#[derive(Default)]
pub(super) struct SubscriberDeathGroup {
    pub(super) deaths: Vec<(crate::engine::manager::hp::PendingDeath, FramePath)>,
}

pub(super) fn queued_defeated_owner_card_cleanup(
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

pub(super) fn queued_defeated_entity_halo_cleanup(
    managers: &BattleManagers,
    death: crate::engine::manager::hp::DeathTransition,
    parent_path: &[usize],
) -> Option<QueuedOp> {
    let commands = managers
        .buff
        .defeated_entity_halo_cleanup(death.target_uid, &managers.hp);
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

pub(super) fn queued_death_settlement(
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

pub(super) fn queued_death_event_group_settlement(
    death: crate::engine::manager::hp::DeathTransition,
    frame_path: &[usize],
) -> QueuedOp {
    let mut queued = queued_death_settlement(death, frame_path);
    queued.frame_path = queued.parent_path.take();
    queued.frame_owner = None;
    queued
}

pub(super) fn queued_death_event_group_completion(
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

pub(super) fn queued_terminal_settlement(event: BattleEvent, parent_path: &[usize]) -> QueuedOp {
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

pub(in crate::engine::runtime) fn attack_has_no_target(
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
pub(super) struct ReactionBatch {
    pub(super) before_publish: Vec<QueuedOp>,
    pub(super) after_publish: Vec<QueuedOp>,
    pub(super) after_settlement: Vec<QueuedOp>,
    pub(super) after_skill: Vec<QueuedOp>,
    pub(super) after_hit: Vec<QueuedOp>,
    pub(super) attacked_skills: Vec<QueuedOp>,
    pub(super) hp_thresholds: Vec<QueuedOp>,
    pub(super) before_after_hit_observers: Vec<QueuedOp>,
    pub(super) after_hit_skills: Vec<QueuedOp>,
    pub(super) after_hit_shell: Vec<QueuedOp>,
    pub(super) shared_hit_skills: Vec<QueuedOp>,
    pub(super) after_action: Vec<QueuedOp>,
}

pub use crate::engine::event::dispatcher::ReactionLane;

impl ReactionBatch {
    pub(super) fn append(&mut self, mut other: Self) {
        self.before_publish.append(&mut other.before_publish);
        self.after_publish.append(&mut other.after_publish);
        self.after_settlement.append(&mut other.after_settlement);
        self.after_skill.append(&mut other.after_skill);
        self.after_hit.append(&mut other.after_hit);
        self.attacked_skills.append(&mut other.attacked_skills);
        self.hp_thresholds.append(&mut other.hp_thresholds);
        self.before_after_hit_observers
            .append(&mut other.before_after_hit_observers);
        self.after_hit_skills.append(&mut other.after_hit_skills);
        self.after_hit_shell.append(&mut other.after_hit_shell);
        self.shared_hit_skills.append(&mut other.shared_hit_skills);
        self.after_action.append(&mut other.after_action);
    }

    pub(super) fn into_ordered(self) -> Vec<QueuedOp> {
        self.before_publish
            .into_iter()
            .chain(self.after_publish)
            .chain(self.after_settlement)
            .chain(self.after_skill)
            .chain(self.after_action)
            .collect()
    }

    pub(super) fn assign_subscriber_death_groups(&mut self, action_path: Option<&[usize]>) {
        if action_path.is_some() {
            assign_subscriber_death_groups(&mut [
                &mut self.before_publish,
                &mut self.after_publish,
                &mut self.after_settlement,
                &mut self.after_skill,
                &mut self.after_hit,
                &mut self.attacked_skills,
                &mut self.hp_thresholds,
                &mut self.before_after_hit_observers,
                &mut self.after_hit_skills,
                &mut self.after_hit_shell,
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

pub(super) fn assign_subscriber_death_groups(queues: &mut [&mut Vec<QueuedOp>]) {
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum DefenderCardCleanupOwner {
    #[default]
    DeathAction,
    ScheduledRoundPhase,
}

impl DefenderCardCleanupOwner {
    pub(super) fn queues_in_drain(self, team_type: i32) -> bool {
        team_type == 1 || self == Self::DeathAction
    }
}
