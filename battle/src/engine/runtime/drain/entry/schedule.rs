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
                | RuleOp::ActionHandSettlement { .. }
                | RuleOp::ImmediateReservePromotion
                | RuleOp::DeathSettlement(_)
                | RuleOp::DeathEventGroupComplete(_)
                | RuleOp::SubscriberGroupComplete
                | RuleOp::HeldActionBatchComplete
                | RuleOp::TerminalSettlement
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
                subscriber: None,
                caster_frame: None,
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
pub fn run_owned_command_group(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    operations: impl IntoIterator<Item = (FrameOwner, RuleOp)>,
) -> Result<DrainResult, DrainError> {
    let mut frames = Vec::new();
    let root = push_root(&mut frames, FrameOwner::Command, FrameTrigger::Active);
    let mut queue = operations
        .into_iter()
        .map(|(owner, op)| {
            let path = push_child(&mut frames, &root, owner, FrameTrigger::Active);
            QueuedOp {
                op,
                trigger: SkillOpTrigger::Active,
                skill_execution: None,
                frame_path: Some(path),
                parent_path: None,
                frame_group: None,
                independent_parent_group: None,
                frame_owner: None,
                subscriber: None,
                caster_frame: None,
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
        DefenderCardCleanupOwner::ScheduledRoundPhase,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_setup_schedule_for_owners_in_round_phase_with_death_cleanup(
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
        DefenderCardCleanupOwner::DeathAction,
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
        DefenderCardCleanupOwner::DeathAction,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_skill_setup_schedule_in_opening_round_phase(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    schedule: &[(SetupStage, i32)],
    owner_uids: Option<&[i64]>,
) -> Result<DrainResult, DrainError> {
    run_skill_setup_schedule_with_container(
        managers,
        pool,
        catalog,
        determinism,
        context,
        schedule,
        owner_uids,
        SetupFrameContainer::OpeningRoundPhase,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_opening_settlement_skills_in_owner_order(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    owner_uids: &[i64],
) -> Result<DrainResult, DrainError> {
    const SCHEDULE: &[(SetupStage, i32)] =
        &[(SetupStage::RoundStart, 1), (SetupStage::BuffGate, 0)];

    let mut result = DrainResult::default();
    for owner_uid in owner_uids {
        let owner_result = run_skill_setup_schedule_with_container(
            managers,
            pool,
            catalog,
            determinism,
            context,
            SCHEDULE,
            Some(std::slice::from_ref(owner_uid)),
            SetupFrameContainer::OpeningRoundPhase,
        )?;
        result.outcomes.extend(owner_result.outcomes);
        result.events.extend(owner_result.events);
        result.frames.extend(owner_result.frames);
    }

    let remaining = run_skill_setup_stage_filtered(
        managers,
        pool,
        catalog,
        determinism,
        context,
        SetupStage::RoundStart,
        1,
        None,
        owner_uids,
        false,
        SetupFrameContainer::OpeningRoundPhase,
        DefenderCardCleanupOwner::DeathAction,
    )?;
    result.outcomes.extend(remaining.outcomes);
    result.events.extend(remaining.events);
    result.frames.extend(remaining.frames);
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
pub fn run_buff_act_setup_schedule_in_opening_round_phase(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    schedule: &[(SetupStage, i32)],
    owner_uids: Option<&[i64]>,
) -> Result<DrainResult, DrainError> {
    run_buff_act_setup_schedule(
        managers,
        pool,
        catalog,
        determinism,
        context,
        schedule,
        owner_uids,
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
    for &(stage, priority) in schedule {
        for owner_uid in owner_uids {
            let owner_result = run_setup_schedule_with_container(
                managers,
                pool,
                catalog,
                determinism,
                context,
                &[(stage, priority)],
                Some(std::slice::from_ref(owner_uid)),
                SetupFrameContainer::RoundPhase,
                DefenderCardCleanupOwner::ScheduledRoundPhase,
            )?;
            result.outcomes.extend(owner_result.outcomes);
            result.events.extend(owner_result.events);
            result.frames.extend(owner_result.frames);
        }
    }
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
pub fn run_opening_sync_setup_in_owner_order(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    owner_uids: &[i64],
) -> Result<DrainResult, DrainError> {
    run_sync_setup_in_owner_order(
        managers,
        pool,
        catalog,
        determinism,
        context,
        owner_uids,
        SetupFrameContainer::Standalone,
        DefenderCardCleanupOwner::DeathAction,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_round_sync_setup_in_owner_order(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    owner_uids: &[i64],
) -> Result<DrainResult, DrainError> {
    run_sync_setup_in_owner_order(
        managers,
        pool,
        catalog,
        determinism,
        context,
        owner_uids,
        SetupFrameContainer::RoundPhase,
        DefenderCardCleanupOwner::ScheduledRoundPhase,
    )
}

#[allow(clippy::too_many_arguments)]
fn run_sync_setup_in_owner_order(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    owner_uids: &[i64],
    frame_container: SetupFrameContainer,
    cleanup_owner: DefenderCardCleanupOwner,
) -> Result<DrainResult, DrainError> {
    const SCHEDULE: &[(SetupStage, i32)] =
        &[(SetupStage::BuffSync, 0), (SetupStage::RoundStartLate, 0)];

    let mut result = DrainResult::default();
    for owner_uid in owner_uids {
        for &(stage, priority) in SCHEDULE {
            let owner_result = run_skill_then_buff_act_setup_stage_filtered(
                managers,
                pool,
                catalog,
                determinism,
                context,
                stage,
                priority,
                Some(std::slice::from_ref(owner_uid)),
                &[],
                false,
                frame_container,
                cleanup_owner,
            )?;
            result.outcomes.extend(owner_result.outcomes);
            result.events.extend(owner_result.events);
            result.frames.extend(owner_result.frames);
        }
    }

    for &(stage, priority) in SCHEDULE {
        let remaining = run_skill_then_buff_act_setup_stage_filtered(
            managers,
            pool,
            catalog,
            determinism,
            context,
            stage,
            priority,
            (stage == SetupStage::RoundStartLate).then_some([].as_slice()),
            owner_uids,
            stage == SetupStage::RoundStartLate,
            frame_container,
            cleanup_owner,
        )?;
        result.outcomes.extend(remaining.outcomes);
        result.events.extend(remaining.events);
        result.frames.extend(remaining.frames);
    }
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
pub fn run_skill_setup_schedule(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    schedule: &[(SetupStage, i32)],
) -> Result<DrainResult, DrainError> {
    run_skill_setup_schedule_with_container(
        managers,
        pool,
        catalog,
        determinism,
        context,
        schedule,
        None,
        SetupFrameContainer::Standalone,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_version_seven_round_start_event_setup(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    owner_uids: &[i64],
) -> Result<DrainResult, DrainError> {
    const OWNER_SCHEDULE: &[(SetupStage, i32)] =
        &[(SetupStage::RoundStart, 1), (SetupStage::BuffGate, 0)];
    const TRANSITION_SCHEDULE: &[(SetupStage, i32)] = &[(SetupStage::RoundTransitionStart, 1)];

    let mut result = DrainResult::default();
    for owner_uid in owner_uids {
        let owner_result = run_setup_schedule_with_container(
            managers,
            pool,
            catalog,
            determinism,
            context,
            OWNER_SCHEDULE,
            Some(std::slice::from_ref(owner_uid)),
            SetupFrameContainer::RoundPhase,
            DefenderCardCleanupOwner::ScheduledRoundPhase,
        )?;
        result.outcomes.extend(owner_result.outcomes);
        result.events.extend(owner_result.events);
        result.frames.extend(owner_result.frames);
    }
    for owner_uid in owner_uids {
        let owner_result = run_setup_schedule_with_container(
            managers,
            pool,
            catalog,
            determinism,
            context,
            TRANSITION_SCHEDULE,
            Some(std::slice::from_ref(owner_uid)),
            SetupFrameContainer::RoundPhase,
            DefenderCardCleanupOwner::ScheduledRoundPhase,
        )?;
        result.outcomes.extend(owner_result.outcomes);
        result.events.extend(owner_result.events);
        result.frames.extend(owner_result.frames);
    }
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
pub fn run_buff_act_setup_schedule_for_owners_in_round_phase(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    schedule: &[(SetupStage, i32)],
    owner_uids: &[i64],
) -> Result<DrainResult, DrainError> {
    run_buff_act_setup_schedule(
        managers,
        pool,
        catalog,
        determinism,
        context,
        schedule,
        Some(owner_uids),
    )
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
    cleanup_owner: DefenderCardCleanupOwner,
) -> Result<DrainResult, DrainError> {
    let mut result = DrainResult::default();
    for &(stage, priority) in schedule {
        let stage_result = run_skill_then_buff_act_setup_stage_filtered(
            managers,
            pool,
            catalog,
            determinism,
            context,
            stage,
            priority,
            owner_uids,
            &[],
            false,
            frame_container,
            cleanup_owner,
        )?;
        result.outcomes.extend(stage_result.outcomes);
        result.events.extend(stage_result.events);
        result.frames.extend(stage_result.frames);
    }
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn run_skill_setup_schedule_with_container(
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
        let stage_result = run_skill_setup_stage_filtered(
            managers,
            pool,
            catalog,
            determinism,
            context,
            stage,
            priority,
            owner_uids,
            &[],
            false,
            frame_container,
            DefenderCardCleanupOwner::DeathAction,
        )?;
        result.outcomes.extend(stage_result.outcomes);
        result.events.extend(stage_result.events);
        result.frames.extend(stage_result.frames);
    }
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn run_buff_act_setup_schedule(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    schedule: &[(SetupStage, i32)],
    owner_uids: Option<&[i64]>,
) -> Result<DrainResult, DrainError> {
    let mut result = DrainResult::default();
    for &(stage, priority) in schedule {
        let stage_result = run_buff_act_setup_stage_filtered(
            managers,
            pool,
            catalog,
            determinism,
            context,
            stage,
            priority,
            owner_uids,
            &[],
        )?;
        result.outcomes.extend(stage_result.outcomes);
        result.events.extend(stage_result.events);
        result.frames.extend(stage_result.frames);
    }
    Ok(result)
}
