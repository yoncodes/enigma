use super::*;

#[allow(clippy::too_many_arguments)]
pub fn run_buff_act_setup_stage_for_owners(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    stage: SetupStage,
    priority: i32,
    owner_uids: &[i64],
) -> Result<DrainResult, DrainError> {
    let trigger = FrameTrigger::Setup { stage, priority };
    let operations = dispatcher::dispatch_buff_act_setup(managers, catalog, stage, priority)
        .into_iter()
        .filter(|(subscriber, _)| owner_uids.contains(&subscriber.feature.owner_uid))
        .map(|(subscriber, operations)| {
            let operations =
                operations.ok_or(DrainError::MissingBuffActOp(subscriber.key.opcode))?;
            Ok((
                FrameOwner::BuffAct {
                    owner_uid: subscriber.feature.owner_uid,
                    source_uid: subscriber.feature.source_uid,
                    buff_uid: subscriber.feature.buff_uid,
                    buff_id: subscriber.feature.buff_id,
                    key: subscriber.key,
                },
                operations,
            ))
        });
    run_owned_buff_act_groups(
        managers,
        pool,
        catalog,
        determinism,
        context,
        trigger,
        SkillOpTrigger::Setup { stage, priority },
        operations,
        false,
    )
}

pub fn run_buff_act_ops(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    operations: impl IntoIterator<Item = (crate::engine::skill::subscriber::BuffActSubscriber, RuleOp)>,
) -> Result<DrainResult, DrainError> {
    run_owned_buff_act_ops(
        managers,
        pool,
        catalog,
        determinism,
        context,
        FrameTrigger::Active,
        SkillOpTrigger::Active,
        operations.into_iter().map(|(subscriber, op)| {
            Ok((
                FrameOwner::BuffAct {
                    owner_uid: subscriber.owner_uid,
                    source_uid: subscriber.source_uid,
                    buff_uid: subscriber.buff_uid,
                    buff_id: subscriber.buff_id,
                    key: subscriber.key.definition,
                },
                op,
            ))
        }),
        true,
    )
}

#[allow(clippy::too_many_arguments)]
fn run_owned_buff_act_ops(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    trigger: FrameTrigger,
    skill_trigger: SkillOpTrigger,
    operations: impl IntoIterator<Item = Result<(FrameOwner, RuleOp), DrainError>>,
    wrap_in_command: bool,
) -> Result<DrainResult, DrainError> {
    run_owned_buff_act_groups(
        managers,
        pool,
        catalog,
        determinism,
        context,
        trigger,
        skill_trigger,
        operations
            .into_iter()
            .map(|operation| operation.map(|(owner, op)| (owner, vec![op]))),
        wrap_in_command,
    )
}

#[allow(clippy::too_many_arguments)]
fn run_owned_buff_act_groups(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    trigger: FrameTrigger,
    skill_trigger: SkillOpTrigger,
    operations: impl IntoIterator<Item = Result<(FrameOwner, Vec<RuleOp>), DrainError>>,
    wrap_in_command: bool,
) -> Result<DrainResult, DrainError> {
    let mut frames = Vec::new();
    let root =
        wrap_in_command.then(|| push_root(&mut frames, FrameOwner::Command, trigger.clone()));
    let mut queue = VecDeque::new();
    for operation in operations {
        let (owner, ops) = operation?;
        let buff_act = if let Some(root) = root.as_ref() {
            push_child(&mut frames, root, owner, trigger.clone())
        } else {
            push_root(&mut frames, owner, trigger.clone())
        };
        for op in ops {
            let (frame_path, parent_path) = match &op {
                RuleOp::Command(_)
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
                | RuleOp::FreezeActiveSkillRates
                | RuleOp::NuoDiKaHit(_) => (Some(buff_act.clone()), None),
                RuleOp::Skill(_) => (None, Some(buff_act.clone())),
            };
            queue.push_back(QueuedOp {
                op,
                trigger: skill_trigger.clone(),
                skill_execution: None,
                frame_path,
                parent_path,
                frame_group: None,
                independent_parent_group: None,
                frame_owner: None,
                subscriber_owner_uid: None,
            });
        }
    }
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
