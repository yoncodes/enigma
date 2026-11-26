use super::*;

pub(super) fn hold(state: &mut DrainState, mut queued: QueuedOp) -> Option<QueuedOp> {
    use crate::engine::skill::action::SkillStart;

    if let Some(casts) = &mut state.round_start_loss_casts
        && matches!(&queued.op, RuleOp::Skill(invocation) if invocation.start == SkillStart::AfterLossBatch)
    {
        queued.parent_path = None;
        casts.push_back(queued);
        None
    } else {
        Some(queued)
    }
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
