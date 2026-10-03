use std::collections::{HashMap, HashSet};

use crate::engine::{
    manager::hp::DeathTransition,
    runtime::record::{FrameOwner, FramePath},
    skill::target::TargetContext,
};

use super::{DrainBudget, DrainError, QueuedOp};

/// Owns bookkeeping shared by the root drain and every nested reaction drain.
#[derive(Default)]
pub(super) struct DrainState {
    after_hit: HashMap<FramePath, Vec<QueuedOp>>,
    after_action: HashMap<FramePath, Vec<QueuedOp>>,
    // Casts held until their action completes, with the step that cast them.
    after_action_casts: HashMap<FramePath, Vec<(Option<FrameOwner>, QueuedOp)>>,
    // Actions that will publish ActionCompleted and have not yet done so.
    open_actions: HashSet<FramePath>,
    completed_actions: HashSet<FramePath>,
    injuries: HashMap<FramePath, Vec<i64>>,
    deaths: HashMap<FramePath, Vec<DeathTransition>>,
    target_modifiers: HashMap<FramePath, i32>,
    budget: DrainBudget,
    context: TargetContext,
    depth: usize,
    death_settlement_depth: usize,
}

impl DrainState {
    pub(super) fn new(context: TargetContext) -> Self {
        Self {
            context,
            ..Self::default()
        }
    }

    pub(super) fn context(&self) -> TargetContext {
        self.context
    }

    pub(super) fn depth(&self) -> usize {
        self.depth
    }

    pub(super) fn enter_nested(&mut self) {
        self.depth += 1;
    }

    pub(super) fn leave_nested(&mut self) {
        self.depth -= 1;
    }

    pub(super) fn consume_budget(&mut self) -> Result<(), DrainError> {
        self.budget.consume(self.depth)
    }

    pub(super) fn enter_death_settlement(&mut self) {
        self.death_settlement_depth += 1;
    }

    pub(super) fn leave_death_settlement(&mut self) {
        self.death_settlement_depth -= 1;
    }

    pub(super) fn death_settlement_in_progress(&self) -> bool {
        self.death_settlement_depth > 0
    }

    pub(super) fn defer_after_hit(&mut self, action_path: Option<&[usize]>, queued: Vec<QueuedOp>) {
        defer(&mut self.after_hit, action_path, queued);
    }

    pub(super) fn take_after_hit(
        &mut self,
        action_path: Option<&FramePath>,
    ) -> Option<Vec<QueuedOp>> {
        action_path.and_then(|path| self.after_hit.remove(path))
    }

    pub(super) fn defer_after_action(
        &mut self,
        action_path: Option<&[usize]>,
        queued: Vec<QueuedOp>,
    ) {
        defer(&mut self.after_action, action_path, queued);
    }

    pub(super) fn push_after_action_cast(
        &mut self,
        action_path: FramePath,
        caster: Option<FrameOwner>,
        queued: QueuedOp,
    ) {
        self.after_action_casts
            .entry(action_path)
            .or_default()
            .push((caster, queued));
    }

    pub(super) fn take_after_action(&mut self, action_path: &FramePath) -> Vec<QueuedOp> {
        self.after_action.remove(action_path).unwrap_or_default()
    }

    pub(super) fn take_after_action_casts(
        &mut self,
        action_path: &FramePath,
    ) -> Vec<(Option<FrameOwner>, QueuedOp)> {
        self.open_actions.remove(action_path);
        self.completed_actions.insert(action_path.clone());
        self.after_action_casts
            .remove(action_path)
            .unwrap_or_default()
    }

    pub(super) fn cancel_action(&mut self, action_path: &FramePath) -> Vec<QueuedOp> {
        self.open_actions.remove(action_path);
        self.completed_actions.insert(action_path.clone());
        self.injuries.remove(action_path);
        self.deaths.remove(action_path);
        self.target_modifiers.remove(action_path);

        let mut abandoned = self.after_hit.remove(action_path).unwrap_or_default();
        abandoned.extend(self.after_action.remove(action_path).unwrap_or_default());
        abandoned.extend(
            self.after_action_casts
                .remove(action_path)
                .unwrap_or_default()
                .into_iter()
                .map(|(_, queued)| queued),
        );
        abandoned
    }

    pub(super) fn open_action(&mut self, action_path: FramePath) {
        if !self.completed_actions.contains(&action_path) {
            self.open_actions.insert(action_path);
        }
    }

    // Only an action that will still complete releases held casts; otherwise they run at once.
    pub(super) fn action_in_progress(&self, action_path: &FramePath) -> bool {
        self.open_actions.contains(action_path)
    }

    pub(super) fn add_target_modifier(&mut self, action_path: FramePath, amount: i32) {
        *self.target_modifiers.entry(action_path).or_default() += amount;
    }

    pub(super) fn take_target_modifier(&mut self, action_path: &FramePath) -> Option<i32> {
        self.target_modifiers.remove(action_path)
    }

    pub(super) fn injuries(&self, action_path: &FramePath) -> Option<&[i64]> {
        self.injuries.get(action_path).map(Vec::as_slice)
    }

    pub(super) fn record_injuries(&mut self, action_path: FramePath, target_uids: &[i64]) {
        let injuries = self.injuries.entry(action_path).or_default();
        for target_uid in target_uids {
            if !injuries.contains(target_uid) {
                injuries.push(*target_uid);
            }
        }
    }

    pub(super) fn record_deaths(
        &mut self,
        action_path: FramePath,
        deaths: impl IntoIterator<Item = DeathTransition>,
    ) {
        self.deaths.entry(action_path).or_default().extend(deaths);
    }

    pub(super) fn take_deaths(&mut self, action_path: &FramePath) -> Option<Vec<DeathTransition>> {
        self.deaths.remove(action_path)
    }
}

fn defer(
    deferred: &mut HashMap<FramePath, Vec<QueuedOp>>,
    action_path: Option<&[usize]>,
    mut queued: Vec<QueuedOp>,
) {
    let Some(action_path) = action_path else {
        return;
    };
    let existing = deferred.entry(action_path.to_vec()).or_default();
    queued.append(existing);
    *existing = queued;
}
