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
    });
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

#[allow(clippy::too_many_arguments)]
pub fn run_conduit_action(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    source_uid: i64,
    group: i32,
    skill_position: i32,
    skill_id: i32,
    frame_target_uid: Option<i64>,
    cost_modifier: Option<(i32, RuleOp)>,
) -> Result<DrainResult, DrainError> {
    let mut frames = Vec::new();
    let root = push_root(
        &mut frames,
        FrameOwner::ConduitAction {
            source_uid,
            group,
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
        },
    ]);
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
        },
        QueuedOp {
            op: RuleOp::Skill(crate::engine::skill::action::SkillInvocation {
                plan: crate::engine::skill::action::SkillRequest {
                    source_uid,
                    skill_id,
                },
                card_index: group,
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
            frame_owner: Some(FrameOwner::Skill {
                source_uid,
                skill_id,
                card_index: group,
                target_uid: frame_target_uid,
            }),
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
        },
        QueuedOp {
            op: RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Conduit(
                crate::engine::manager::conduit::ConduitCommand::FinishSkill {
                    source_uid,
                    skill_id,
                },
            )),
            trigger: SkillOpTrigger::Active,
            skill_execution: None,
            frame_path: None,
            parent_path: Some(root),
            frame_group: Some(skill_frame),
            independent_parent_group: None,
            frame_owner: None,
        },
    ]);
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

pub fn run_conduit_stop(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    source_uid: i64,
    group: i32,
) -> Result<DrainResult, DrainError> {
    let mut frames = Vec::new();
    let root = push_root(
        &mut frames,
        FrameOwner::ConduitStopped { source_uid, group },
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
