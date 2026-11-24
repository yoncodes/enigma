use super::*;

pub(super) struct Scope<'a> {
    pub(super) base_pool: &'a TargetPool,
    pub(super) frame_path: &'a FramePath,
    pub(super) action_scope: Option<&'a FramePath>,
    pub(super) trigger: &'a SkillOpTrigger,
}

pub(super) struct Committed {
    pub(super) outcome: RuleOutcome,
    pub(super) followups: Vec<(RuleOp, OutputOwner)>,
    pub(super) claimed_counters: Vec<QueuedOp>,
    pub(super) entry_halo_fanout: bool,
}

pub(super) fn apply(
    access: DrainAccess<'_>,
    scope: Scope<'_>,
    queue: &mut VecDeque<QueuedOp>,
    bus: &mut EventBus,
    command: RuleOp,
) -> Result<Committed, DrainError> {
    let DrainAccess {
        managers,
        pool,
        determinism,
        result,
        state,
        ..
    } = access;
    let Scope {
        base_pool,
        frame_path,
        action_scope,
        trigger,
    } = scope;
    let command = prepare_command(managers, pool, determinism, command);

    let entry_halo_fanout = matches!(
        &command,
        RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Buff(
            crate::engine::manager::buff::BuffCommand::FanoutEntryHalos(_)
        ))
    );

    // This is the single durable commit point for non-skill RuleOps.
    // The returned outcome describes committed changes and follow-ups.
    let mut outcome = execute_rule_op_with(managers, bus, determinism, command)?;
    let damage = match &mut outcome {
        RuleOutcome::Hp(hit) => std::slice::from_mut(hit.as_mut()),
        RuleOutcome::HpBatch(hits) => hits.as_mut_slice(),
        _ => &mut [],
    };
    if let Some(chain) = state.action_chain(frame_path) {
        state.shield_counter.earn(
            chain,
            damage
                .iter_mut()
                .flat_map(|hit| std::mem::take(&mut hit.shield_counter_credits)),
        );
    }
    let claimed_counters =
        match &outcome {
            RuleOutcome::SkillLifecycle(
                crate::engine::skill::action::SkillLifecycle::ActionCompleted(action),
            ) if state.action_in_progress(frame_path) => state
                .action_chain(frame_path)
                .map_or_else(Vec::new, |chain| {
                    state
                        .shield_counter
                        .claim(&chain, frame_path, action, managers)
                }),
            _ => Vec::new(),
        };
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
            .cloned()
            .ok_or(DrainError::MissingActiveSkillContext)?;
        state.add_target_modifier(action_scope, additional_count);
    }
    let followups = outcome.followups();
    let damage_amount = outcome.applied_damage();
    let death_count = outcome.death_count();
    let guard_break_count = outcome.guard_break_count();
    let injured_targets = outcome.injured_targets();
    if !injured_targets.is_empty()
        && let Some(action_path) = active_skill_scope_path(&result.frames, frame_path)
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
                && queued.frame_path.as_ref() == Some(frame_path)
                && matches!(queued.op, RuleOp::Skill(_))
        })
    {
        let source_uid = match &queued.op {
            RuleOp::Skill(invocation) => invocation.plan.source_uid,
            _ => 0,
        };
        let source_is_attacker = pool.source_is_attacker(source_uid);
        let allied_injuries = injured_targets
            .into_iter()
            .filter(|target_uid| pool.source_is_attacker(*target_uid) == source_is_attacker);
        if let Some(execution) = queued.skill_execution.as_mut() {
            execution.record_damage(damage_amount);
            execution.record_guard_breaks(guard_break_count);
            execution.record_injuries(allied_injuries);
        }
    }

    Ok(Committed {
        outcome,
        followups,
        claimed_counters,
        entry_halo_fanout,
    })
}
