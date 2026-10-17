use super::*;

pub fn run(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    initial: impl IntoIterator<Item = RuleOp>,
) -> Result<DrainResult, DrainError> {
    let mut queue = initial
        .into_iter()
        .map(|op| QueuedOp {
            op,
            trigger: SkillOpTrigger::Active,
            skill_execution: None,
            frame_path: None,
            parent_path: None,
            frame_group: None,
            independent_parent_group: None,
            frame_owner: None,
            subscriber: None,
            caster_frame: None,
        })
        .collect::<VecDeque<_>>();
    drain_queue(managers, pool, catalog, determinism, context, &mut queue)
}

pub fn run_skill(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    invocation: crate::engine::skill::action::SkillInvocation,
    modifiers: crate::engine::skill::action::SkillModifiers,
) -> Result<DrainResult, DrainError> {
    let mut queue = VecDeque::from([QueuedOp {
        op: RuleOp::Skill(invocation),
        trigger: SkillOpTrigger::Active,
        skill_execution: Some(SkillExecution::with_modifiers(context, modifiers)),
        frame_path: None,
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber: None,
        caster_frame: None,
    }]);
    drain_queue(managers, pool, catalog, determinism, context, &mut queue)
}

pub fn run_action(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    prelude: impl IntoIterator<Item = RuleOp>,
    invocation: crate::engine::skill::action::SkillInvocation,
) -> Result<DrainResult, DrainError> {
    run_action_with_cost(
        managers,
        pool,
        catalog,
        determinism,
        context,
        prelude,
        None,
        invocation,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_action_with_cost(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    prelude: impl IntoIterator<Item = RuleOp>,
    action_cost: Option<crate::engine::manager::ex_point::ExPointCommand>,
    invocation: crate::engine::skill::action::SkillInvocation,
) -> Result<DrainResult, DrainError> {
    run_action_with_cost_and_cleanup_owner(
        managers,
        pool,
        catalog,
        determinism,
        context,
        prelude,
        action_cost,
        invocation,
        DefenderCardCleanupOwner::DeathAction,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_round_action_with_cost(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    prelude: impl IntoIterator<Item = RuleOp>,
    action_cost: Option<crate::engine::manager::ex_point::ExPointCommand>,
    invocation: crate::engine::skill::action::SkillInvocation,
) -> Result<DrainResult, DrainError> {
    run_action_with_cost_and_cleanup_owner(
        managers,
        pool,
        catalog,
        determinism,
        context,
        prelude,
        action_cost,
        invocation,
        DefenderCardCleanupOwner::ScheduledRoundPhase,
    )
}

#[allow(clippy::too_many_arguments)]
fn run_action_with_cost_and_cleanup_owner(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    prelude: impl IntoIterator<Item = RuleOp>,
    action_cost: Option<crate::engine::manager::ex_point::ExPointCommand>,
    invocation: crate::engine::skill::action::SkillInvocation,
    cleanup_owner: DefenderCardCleanupOwner,
) -> Result<DrainResult, DrainError> {
    let current_pool = pool.runtime_view(managers);
    if attack_has_no_target(
        &invocation,
        catalog,
        &current_pool,
        managers,
        determinism,
        context,
    ) {
        return Ok(DrainResult::default());
    }
    let mut frames = Vec::new();
    let root = push_root(
        &mut frames,
        FrameOwner::Skill {
            source_uid: invocation.plan.source_uid,
            skill_id: invocation.plan.skill_id,
            card_index: invocation.card_index,
            target_uid: invocation_frame_target(invocation.target, &SkillOpTrigger::Active),
        },
        FrameTrigger::Active,
    );
    let mut queue = prelude
        .into_iter()
        .map(|op| QueuedOp {
            op,
            trigger: SkillOpTrigger::Active,
            skill_execution: None,
            frame_path: Some(root.clone()),
            parent_path: None,
            frame_group: None,
            independent_parent_group: None,
            frame_owner: None,
            subscriber: None,
            caster_frame: None,
        })
        .collect::<VecDeque<_>>();
    queue.push_back(QueuedOp {
        op: RuleOp::Skill(invocation),
        trigger: SkillOpTrigger::Active,
        skill_execution: Some(SkillExecution::with_action_cost(context, action_cost)),
        frame_path: Some(root.clone()),
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber: None,
        caster_frame: None,
    });
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
pub fn run_conduit_action(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    source_uid: i64,
    selected_group: i32,
    device_index: i32,
    skill_position: i32,
    skill_id: i32,
    frame_target_uid: Option<i64>,
    cost_modifier: Option<(i32, RuleOp)>,
) -> Result<DrainResult, DrainError> {
    let personal_resource_cost = if catalog.is_big_skill(skill_id)
        && crate::engine::manager::ex_point::ExPointKind::from_wire(
            managers.ex_point.kind(source_uid),
        ) == crate::engine::manager::ex_point::ExPointKind::DevicePower
    {
        let current = managers.ex_point.get(source_uid);
        let required = managers.ex_point.maximum(source_uid).ok_or({
            DrainError::Skill(
                crate::engine::runtime::skill::SkillOpError::MissingSourceEntity(source_uid),
            )
        })?;
        if current < required {
            return Err(DrainError::InsufficientUltimateResource {
                owner_uid: source_uid,
                skill_id,
                required,
                current,
            });
        }
        Some(crate::engine::manager::ex_point::ExPointCommand::Spend(
            crate::engine::manager::ex_point::ExPointChange {
                origin: crate::engine::skill::rule::CommandOrigin {
                    domain: crate::engine::skill::rule::RuleDomain::Lifecycle,
                    key: crate::engine::skill::rule::DefinitionKey::new(0, "ConduitUltimateCost"),
                },
                source_uid,
                target_uid: source_uid,
                delta: -required,
                config_effect: 0,
                effect_type: sonettobuf::effect_type_enum::EffectType::Expointchange as i32,
            },
        ))
    } else {
        None
    };
    let mut frames = Vec::new();
    let root = push_root(
        &mut frames,
        FrameOwner::ConduitAction {
            source_uid,
            selected_group,
            device_index,
            skill_position,
            target_uid: frame_target_uid,
        },
        FrameTrigger::Active,
    );
    let cost_reduction = cost_modifier
        .as_ref()
        .map(|(reduction, _)| *reduction)
        .unwrap_or_default();
    let skill_frame = Rc::new(RefCell::new(None));
    let mut queue = VecDeque::from([
        QueuedOp {
            op: RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Conduit(
                crate::engine::manager::conduit::ConduitCommand::SetRunning {
                    source_uid,
                    running: true,
                },
            )),
            trigger: SkillOpTrigger::Active,
            skill_execution: None,
            frame_path: Some(root.clone()),
            parent_path: None,
            frame_group: None,
            independent_parent_group: None,
            frame_owner: None,
            subscriber: None,
            caster_frame: None,
        },
        QueuedOp {
            op: RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Conduit(
                crate::engine::manager::conduit::ConduitCommand::BeginSkill {
                    source_uid,
                    skill_id,
                    cost_reduction,
                },
            )),
            trigger: SkillOpTrigger::Active,
            skill_execution: None,
            frame_path: Some(root.clone()),
            parent_path: None,
            frame_group: None,
            independent_parent_group: None,
            frame_owner: None,
            subscriber: None,
            caster_frame: None,
        },
    ]);
    if let Some(cost) = personal_resource_cost {
        queue.push_back(QueuedOp {
            op: RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::ExPoint(
                cost,
            )),
            trigger: SkillOpTrigger::Active,
            skill_execution: None,
            frame_path: Some(root.clone()),
            parent_path: None,
            frame_group: None,
            independent_parent_group: None,
            frame_owner: None,
            subscriber: None,
            caster_frame: None,
        });
    }
    if let Some((_, consume)) = cost_modifier {
        queue.push_back(QueuedOp {
            op: consume,
            trigger: SkillOpTrigger::Active,
            skill_execution: None,
            frame_path: Some(root.clone()),
            parent_path: None,
            frame_group: None,
            independent_parent_group: None,
            frame_owner: None,
            subscriber: None,
            caster_frame: None,
        });
    }
    queue.extend([
        QueuedOp {
            op: RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Conduit(
                crate::engine::manager::conduit::ConduitCommand::CommitSkillCost {
                    source_uid,
                    skill_id,
                },
            )),
            trigger: SkillOpTrigger::Active,
            skill_execution: None,
            frame_path: Some(root.clone()),
            parent_path: None,
            frame_group: None,
            independent_parent_group: None,
            frame_owner: None,
            subscriber: None,
            caster_frame: None,
        },
        QueuedOp {
            op: RuleOp::Skill(crate::engine::skill::action::SkillInvocation {
                plan: crate::engine::skill::action::SkillRequest {
                    source_uid,
                    skill_id,
                },
                mode: crate::engine::skill::action::SkillExecutionMode::Device,
                ..crate::engine::skill::action::SkillInvocation::from(
                    crate::engine::skill::action::SkillRequest {
                        source_uid,
                        skill_id,
                    },
                )
            }),
            trigger: SkillOpTrigger::Active,
            skill_execution: Some(SkillExecution::new(context)),
            frame_path: None,
            parent_path: Some(root.clone()),
            frame_group: Some(skill_frame.clone()),
            independent_parent_group: None,
            frame_owner: Some(FrameOwner::ConduitSkill {
                source_uid,
                skill_id,
                device_index,
                target_uid: frame_target_uid,
            }),
            subscriber: None,
            caster_frame: None,
        },
        QueuedOp {
            op: RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Conduit(
                crate::engine::manager::conduit::ConduitCommand::CompleteActivation {
                    source_uid,
                    skill_id,
                },
            )),
            trigger: SkillOpTrigger::Active,
            skill_execution: None,
            frame_path: None,
            parent_path: Some(root.clone()),
            frame_group: Some(skill_frame.clone()),
            independent_parent_group: None,
            frame_owner: None,
            subscriber: None,
            caster_frame: None,
        },
    ]);
    let mut result = drain_queue_with_frames_and_cleanup_owner(
        managers,
        pool,
        catalog,
        determinism,
        context,
        &mut queue,
        frames,
        DefenderCardCleanupOwner::ScheduledRoundPhase,
    )?;
    let frame_path = skill_frame
        .borrow()
        .clone()
        .ok_or(DrainError::MissingActiveSkillContext)?;
    let mut completion = VecDeque::from([QueuedOp {
        op: RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Conduit(
            crate::engine::manager::conduit::ConduitCommand::FinishSkill {
                source_uid,
                skill_id,
            },
        )),
        trigger: SkillOpTrigger::Active,
        skill_execution: None,
        frame_path: Some(frame_path),
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber: None,
        caster_frame: None,
    }]);
    let mut completed = drain_queue_with_frames_and_cleanup_owner(
        managers,
        pool,
        catalog,
        determinism,
        context,
        &mut completion,
        std::mem::take(&mut result.frames),
        DefenderCardCleanupOwner::ScheduledRoundPhase,
    )?;
    result.outcomes.append(&mut completed.outcomes);
    result.events.append(&mut completed.events);
    result.frames = completed.frames;
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
pub fn run_conduit_stop(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    source_uid: i64,
    device_index: i32,
    skill_position: i32,
) -> Result<DrainResult, DrainError> {
    let mut frames = Vec::new();
    let root = push_root(
        &mut frames,
        FrameOwner::ConduitStopped {
            source_uid,
            device_index,
            skill_position,
        },
        FrameTrigger::Active,
    );
    let mut queue = VecDeque::from([QueuedOp {
        op: RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Conduit(
            crate::engine::manager::conduit::ConduitCommand::SetRunning {
                source_uid,
                running: false,
            },
        )),
        trigger: SkillOpTrigger::Active,
        skill_execution: None,
        frame_path: Some(root),
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber: None,
        caster_frame: None,
    }]);
    drain_queue_with_frames(
        managers,
        pool,
        catalog,
        determinism,
        context,
        &mut queue,
        frames,
    )
}
