use super::*;

pub(super) fn is_threshold_from_loss(op: &RuleOp) -> bool {
    matches!(
        op,
        RuleOp::Command(BattleCommand::ThresholdSkill(command))
            if crate::engine::skill::buff_act::raspberry::owns_loss(command.cause)
    )
}

pub(super) fn run(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    queue: &mut VecDeque<QueuedOp>,
    frames: Vec<SemanticFrame>,
) -> Result<DrainResult, DrainError> {
    let mut state = DrainState::new(context);
    state.round_start_loss_casts = Some(VecDeque::new());
    let mut result = drain_queue_with_deferred(
        managers,
        pool,
        catalog,
        determinism,
        queue,
        frames,
        &mut state,
    )?;
    let mut casts = state.round_start_loss_casts.take().unwrap_or_default();
    if !casts.is_empty() {
        let cast_result = drain_queue_with_deferred(
            managers,
            pool,
            catalog,
            determinism,
            &mut casts,
            std::mem::take(&mut result.frames),
            &mut state,
        )?;
        result.outcomes.extend(cast_result.outcomes);
        result.events.extend(cast_result.events);
        result.frames = cast_result.frames;
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
