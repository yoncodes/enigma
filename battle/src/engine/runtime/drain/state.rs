use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use crate::engine::{
    event::payload::BattleEvent,
    manager::hp::{HurtDamageFromType, PendingDeath},
    runtime::record::{FrameOwner, FramePath},
    skill::{rule::DefinitionKey, target::TargetContext},
};

use super::{
    DefenderCardCleanupOwner, DrainBudget, DrainError, QueuedOp, SubscriberDeathGroup,
    assign_subscriber_death_groups,
};

type DeferredCast = (Option<FrameOwner>, QueuedOp);
pub(super) type ActionReactionKey = (i64, i32, Option<usize>, DefinitionKey);

/// Owns bookkeeping shared by the root drain and every nested reaction drain.
#[derive(Default)]
pub(super) struct DrainState {
    after_hit: HashMap<FramePath, Vec<QueuedOp>>,
    attacked_skills: HashMap<FramePath, Vec<QueuedOp>>,
    hp_thresholds: HashMap<FramePath, Vec<QueuedOp>>,
    hit_passive_skills: HashMap<FramePath, Vec<QueuedOp>>,
    hit_skills: HashMap<FramePath, Vec<QueuedOp>>,
    before_after_hit_observers: HashMap<FramePath, Vec<QueuedOp>>,
    after_hit_skills: HashMap<FramePath, Vec<QueuedOp>>,
    after_hit_shell: HashMap<FramePath, Vec<QueuedOp>>,
    after_action: HashMap<FramePath, Vec<QueuedOp>>,
    // Casts held until their action completes, with the step that cast them.
    after_action_casts: HashMap<FramePath, Vec<DeferredCast>>,
    // Actions that will publish ActionCompleted and have not yet done so.
    open_actions: HashSet<FramePath>,
    completed_actions: HashSet<FramePath>,
    action_reactions: HashMap<FramePath, HashSet<ActionReactionKey>>,
    action_shared_damage: HashMap<FramePath, HashMap<i64, [i32; 2]>>,
    action_skill_reaction_frames: HashMap<(FramePath, i64, i32), FramePath>,
    event_batch_reactions: HashMap<u64, HashSet<ActionReactionKey>>,
    next_event_batch_id: u64,
    injuries: HashMap<FramePath, Vec<i64>>,
    deaths: HashMap<FramePath, Vec<PendingDeath>>,
    completed_death_event_groups: Vec<PendingDeath>,
    target_modifiers: HashMap<FramePath, i32>,
    budget: DrainBudget,
    context: TargetContext,
    depth: usize,
    death_settlement_depth: usize,
    defender_card_cleanup_owner: DefenderCardCleanupOwner,
}

impl DrainState {
    pub(super) fn new(context: TargetContext) -> Self {
        Self {
            context,
            ..Self::default()
        }
    }

    pub(super) fn with_defender_card_cleanup_owner(
        mut self,
        owner: DefenderCardCleanupOwner,
    ) -> Self {
        self.defender_card_cleanup_owner = owner;
        self
    }

    pub(super) fn defender_card_cleanup_owner(&self) -> DefenderCardCleanupOwner {
        self.defender_card_cleanup_owner
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

    pub(super) fn defer_after_hit_skills(
        &mut self,
        action_path: Option<&[usize]>,
        queued: Vec<QueuedOp>,
    ) {
        defer(&mut self.after_hit_skills, action_path, queued);
    }

    pub(super) fn defer_after_hit_shell(
        &mut self,
        action_path: Option<&[usize]>,
        queued: Vec<QueuedOp>,
    ) {
        defer(&mut self.after_hit_shell, action_path, queued);
    }

    pub(super) fn defer_before_after_hit_observers(
        &mut self,
        action_path: Option<&[usize]>,
        queued: Vec<QueuedOp>,
    ) {
        defer(&mut self.before_after_hit_observers, action_path, queued);
    }

    pub(super) fn defer_hit_skills(
        &mut self,
        action_path: Option<&[usize]>,
        queued: Vec<QueuedOp>,
    ) {
        defer(&mut self.hit_skills, action_path, queued);
    }

    pub(super) fn defer_hit_passive_skills(
        &mut self,
        action_path: Option<&[usize]>,
        queued: Vec<QueuedOp>,
    ) {
        defer(&mut self.hit_passive_skills, action_path, queued);
    }

    pub(super) fn defer_hp_thresholds(
        &mut self,
        action_path: Option<&[usize]>,
        queued: Vec<QueuedOp>,
    ) {
        defer(&mut self.hp_thresholds, action_path, queued);
    }

    pub(super) fn defer_attacked_skills(
        &mut self,
        action_path: Option<&[usize]>,
        queued: Vec<QueuedOp>,
    ) {
        defer(&mut self.attacked_skills, action_path, queued);
    }

    pub(super) fn take_attacked_skills(
        &mut self,
        action_path: Option<&FramePath>,
    ) -> Option<Vec<QueuedOp>> {
        action_path.and_then(|path| self.attacked_skills.remove(path))
    }

    pub(super) fn take_hp_thresholds(
        &mut self,
        action_path: Option<&FramePath>,
    ) -> Option<Vec<QueuedOp>> {
        action_path.and_then(|path| self.hp_thresholds.remove(path))
    }

    pub(super) fn take_hit_passive_skills(
        &mut self,
        action_path: Option<&FramePath>,
    ) -> Option<Vec<QueuedOp>> {
        action_path.and_then(|path| self.hit_passive_skills.remove(path))
    }

    pub(super) fn take_hit_skills(
        &mut self,
        action_path: Option<&FramePath>,
    ) -> Option<Vec<QueuedOp>> {
        action_path.and_then(|path| self.hit_skills.remove(path))
    }

    pub(super) fn take_after_hit_skills(
        &mut self,
        action_path: Option<&FramePath>,
    ) -> Option<Vec<QueuedOp>> {
        action_path.and_then(|path| self.after_hit_skills.remove(path))
    }

    pub(super) fn take_after_hit_shell(
        &mut self,
        action_path: Option<&FramePath>,
    ) -> Option<Vec<QueuedOp>> {
        action_path.and_then(|path| self.after_hit_shell.remove(path))
    }

    pub(super) fn take_before_after_hit_observers(
        &mut self,
        action_path: Option<&FramePath>,
    ) -> Option<Vec<QueuedOp>> {
        action_path.and_then(|path| self.before_after_hit_observers.remove(path))
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
        self.action_reactions.remove(action_path);
        self.action_shared_damage.remove(action_path);
        self.after_action_casts
            .remove(action_path)
            .unwrap_or_default()
    }

    pub(super) fn action_skill_reaction_frame(
        &mut self,
        action_path: &[usize],
        source_uid: i64,
        skill_id: i32,
        current: &[usize],
    ) -> FramePath {
        self.action_skill_reaction_frames
            .entry((action_path.to_vec(), source_uid, skill_id))
            .or_insert_with(|| current.to_vec())
            .clone()
    }

    pub(super) fn retire_replaced_subscriber_casts(
        &mut self,
        owner_uid: i64,
        active_skills: impl IntoIterator<Item = i32>,
    ) {
        let active_skills = active_skills.into_iter().collect::<HashSet<_>>();
        for casts in self.after_action_casts.values_mut() {
            casts.retain(|(_, queued)| {
                !queued.subscriber.as_ref().is_some_and(|subscriber| {
                    subscriber.owner_uid == owner_uid
                        && subscriber
                            .skill_id
                            .is_some_and(|skill_id| !active_skills.contains(&skill_id))
                })
            });
        }
    }

    pub(super) fn cancel_action(&mut self, action_path: &FramePath) -> Vec<QueuedOp> {
        self.open_actions.remove(action_path);
        self.completed_actions.insert(action_path.clone());
        self.injuries.remove(action_path);
        self.deaths.remove(action_path);
        self.target_modifiers.remove(action_path);
        self.action_reactions.remove(action_path);
        self.action_shared_damage.remove(action_path);
        let mut abandoned = self.after_hit.remove(action_path).unwrap_or_default();
        abandoned.extend(self.attacked_skills.remove(action_path).unwrap_or_default());
        abandoned.extend(self.hp_thresholds.remove(action_path).unwrap_or_default());
        abandoned.extend(
            self.hit_passive_skills
                .remove(action_path)
                .unwrap_or_default(),
        );
        abandoned.extend(self.hit_skills.remove(action_path).unwrap_or_default());
        abandoned.extend(
            self.before_after_hit_observers
                .remove(action_path)
                .unwrap_or_default(),
        );
        abandoned.extend(
            self.after_hit_skills
                .remove(action_path)
                .unwrap_or_default(),
        );
        abandoned.extend(self.after_hit_shell.remove(action_path).unwrap_or_default());
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

    pub(super) fn action_reactions(
        &mut self,
        action_path: &[usize],
    ) -> &mut HashSet<ActionReactionKey> {
        self.action_reactions
            .entry(action_path.to_vec())
            .or_default()
    }

    pub(super) fn record_action_shared_damage(
        &mut self,
        action_path: Option<&[usize]>,
        events: &mut [BattleEvent],
    ) {
        let mut local = HashMap::new();
        let counts = if let Some(action_path) = action_path {
            self.action_shared_damage
                .entry(action_path.to_vec())
                .or_default()
        } else {
            &mut local
        };
        for event in events.iter() {
            let BattleEvent::DamageShared {
                participant_uids,
                share_count,
                damage_from,
                ..
            } = event
            else {
                continue;
            };
            let lane = match damage_from {
                HurtDamageFromType::Skill => 0,
                HurtDamageFromType::SkillEffect => 1,
                _ => continue,
            };
            for owner_uid in participant_uids {
                let owner = counts.entry(*owner_uid).or_insert([0, 0]);
                owner[lane] = owner[lane].max(*share_count);
            }
        }
        for event in events {
            let BattleEvent::DamageShared {
                participant_uids,
                action_share_count,
                ..
            } = event
            else {
                continue;
            };
            *action_share_count = participant_uids
                .iter()
                .filter_map(|owner_uid| counts.get(owner_uid))
                .map(|lanes| lanes.iter().sum())
                .max()
                .unwrap_or(*action_share_count);
        }
    }

    pub(super) fn next_event_batch_id(&mut self) -> u64 {
        let id = self.next_event_batch_id;
        self.next_event_batch_id += 1;
        id
    }

    pub(super) fn event_batch_reactions(
        &mut self,
        batch_id: u64,
    ) -> &mut HashSet<ActionReactionKey> {
        self.event_batch_reactions.entry(batch_id).or_default()
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
        deaths: impl IntoIterator<Item = PendingDeath>,
    ) {
        self.deaths.entry(action_path).or_default().extend(deaths);
    }

    pub(super) fn take_deaths(&mut self, action_path: &FramePath) -> Option<Vec<PendingDeath>> {
        self.deaths.remove(action_path)
    }

    pub(super) fn complete_death_event_group(&mut self, death: PendingDeath) {
        self.completed_death_event_groups.push(death);
    }

    pub(super) fn take_completed_death_event_groups(
        &mut self,
        pending: &[PendingDeath],
    ) -> Vec<PendingDeath> {
        let mut completed = Vec::new();
        self.completed_death_event_groups.retain(|death| {
            if pending.contains(death) {
                completed.push(*death);
                false
            } else {
                true
            }
        });
        completed
    }

    pub(super) fn record_subscriber_deaths(
        &mut self,
        group: &Rc<RefCell<SubscriberDeathGroup>>,
        parent_path: &FramePath,
        deaths: impl IntoIterator<Item = PendingDeath>,
    ) {
        let mut group = group.borrow_mut();
        for death in deaths {
            let current = (death, parent_path.clone());
            if let Some(existing) = group.deaths.iter_mut().find(|(recorded, _)| {
                recorded.transition().target_uid == death.transition().target_uid
            }) {
                *existing = current;
            } else {
                group.deaths.push(current);
            }
        }
    }

    pub(super) fn take_subscriber_deaths(
        &mut self,
        group: &Rc<RefCell<SubscriberDeathGroup>>,
    ) -> Vec<(PendingDeath, FramePath)> {
        std::mem::take(&mut group.borrow_mut().deaths)
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
    assign_subscriber_death_groups(&mut [&mut queued]);
    *existing = queued;
}
