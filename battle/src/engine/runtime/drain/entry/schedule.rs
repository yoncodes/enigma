use super::*;

pub fn run_command_group(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    operations: impl IntoIterator<Item = RuleOp>,
) -> Result<DrainResult, DrainError> {
    let mut frames = Vec::new();
    let root = push_root(&mut frames, FrameOwner::Command, FrameTrigger::Active);
    let mut queue = operations
        .into_iter()
        .map(|op| {
            let (frame_path, parent_path) = match op {
                RuleOp::Skill(_) => (None, Some(root.clone())),
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
                | RuleOp::NuoDiKaHit(_) => (Some(root.clone()), None),
            };
            QueuedOp {
                op,
                trigger: SkillOpTrigger::Active,
                skill_execution: None,
                frame_path,
                parent_path,
                frame_group: None,
                independent_parent_group: None,
                frame_owner: None,
                subscriber_owner_uid: None,
            }
        })
        .collect::<VecDeque<_>>();
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
pub fn run_setup_schedule(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    schedule: &[(SetupStage, i32)],
) -> Result<DrainResult, DrainError> {
    let mut result = DrainResult::default();
    for &(stage, priority) in schedule {
        let stage_result = run_setup_stage(
            managers,
            pool,
            catalog,
            determinism,
            context,
            stage,
            priority,
        )?;
        result.outcomes.extend(stage_result.outcomes);
        result.events.extend(stage_result.events);
        result.frames.extend(stage_result.frames);
    }
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
pub fn run_setup_schedule_for_owners_in_round_phase(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    schedule: &[(SetupStage, i32)],
    owner_uids: &[i64],
) -> Result<DrainResult, DrainError> {
    run_setup_schedule_with_container(
        managers,
        pool,
        catalog,
        determinism,
        context,
        schedule,
        Some(owner_uids),
        SetupFrameContainer::RoundPhase,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_setup_schedule_in_opening_round_phase(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    schedule: &[(SetupStage, i32)],
) -> Result<DrainResult, DrainError> {
    run_setup_schedule_with_container(
        managers,
        pool,
        catalog,
        determinism,
        context,
        schedule,
        None,
        SetupFrameContainer::OpeningRoundPhase,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_setup_schedule_for_owners_in_opening_round_phase(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    schedule: &[(SetupStage, i32)],
    owner_uids: &[i64],
) -> Result<DrainResult, DrainError> {
    run_setup_schedule_with_container(
        managers,
        pool,
        catalog,
        determinism,
        context,
        schedule,
        Some(owner_uids),
        SetupFrameContainer::OpeningRoundPhase,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_setup_schedule_in_owner_order_round_phase(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    schedule: &[(SetupStage, i32)],
    owner_uids: &[i64],
) -> Result<DrainResult, DrainError> {
    let mut result = DrainResult::default();
    for owner_uid in owner_uids {
        let owner_result = run_setup_schedule_with_container(
            managers,
            pool,
            catalog,
            determinism,
            context,
            schedule,
            Some(std::slice::from_ref(owner_uid)),
            SetupFrameContainer::RoundPhase,
        )?;
        result.outcomes.extend(owner_result.outcomes);
        result.events.extend(owner_result.events);
        result.frames.extend(owner_result.frames);
    }
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn run_setup_schedule_with_container(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    schedule: &[(SetupStage, i32)],
    owner_uids: Option<&[i64]>,
    frame_container: SetupFrameContainer,
) -> Result<DrainResult, DrainError> {
    let mut result = DrainResult::default();
    for &(stage, priority) in schedule {
        let stage_result = run_setup_stage_filtered(
            managers,
            pool,
            catalog,
            determinism,
            context,
            stage,
            priority,
            std::iter::empty(),
            |_| Vec::new(),
            owner_uids,
            false,
            frame_container,
        )?;
        result.outcomes.extend(stage_result.outcomes);
        result.events.extend(stage_result.events);
        result.frames.extend(stage_result.frames);
    }
    Ok(result)
}
