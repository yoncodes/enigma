use std::collections::{HashMap, HashSet};

use crate::engine::skill::rule::ConfiguredRuleKey;

type CastKey = (i64, ConfiguredRuleKey);

#[derive(Debug, Clone, Default)]
pub(super) struct ConfiguredRuleProgressManager {
    progress: HashMap<(i64, ConfiguredRuleKey, i32), i32>,
    held: HashSet<CastKey>,
    round_start_casts: HashSet<CastKey>,
    action_casts: HashSet<CastKey>,
    in_round_start: bool,
    in_cloth_sequence: bool,
}

impl ConfiguredRuleProgressManager {
    pub(super) fn begin_round_start(&mut self) {
        self.round_start_casts.clear();
        self.in_round_start = true;
        self.in_cloth_sequence = false;
    }

    pub(super) fn begin_action_round(&mut self) {
        self.clear_completed_round_start_casts();
        self.in_round_start = false;
        self.in_cloth_sequence = false;
    }

    pub(super) fn begin_cloth_sequence(&mut self) {
        if self.in_cloth_sequence {
            return;
        }
        self.clear_completed_round_start_casts();
        let completed = self
            .action_casts
            .iter()
            .copied()
            .filter(|cast| !self.held.contains(cast))
            .collect::<Vec<_>>();
        for cast in completed {
            self.action_casts.remove(&cast);
            self.clear(cast);
        }
        self.in_round_start = false;
        self.in_cloth_sequence = true;
    }

    pub(super) fn advance_until_cast(
        &mut self,
        owner_uid: i64,
        key: ConfiguredRuleKey,
        threshold: i32,
        delta: i32,
    ) -> bool {
        let cast = (owner_uid, key);
        if threshold <= 0 || delta <= 0 || self.held.contains(&cast) {
            return false;
        }
        let progress = self
            .progress
            .entry((owner_uid, key, threshold))
            .or_default();
        *progress = progress.saturating_add(delta);
        if *progress < threshold {
            return false;
        }
        *progress = 0;
        self.held.insert(cast);
        if self.in_round_start {
            self.round_start_casts.insert(cast);
        } else if !self.in_cloth_sequence {
            self.action_casts.insert(cast);
        }
        true
    }

    pub(super) fn release(&mut self, owner_uid: i64, key: ConfiguredRuleKey) {
        let cast = (owner_uid, key);
        self.held.remove(&cast);
        let crossed_phase = (!self.in_round_start && self.round_start_casts.remove(&cast))
            || (self.in_cloth_sequence && self.action_casts.remove(&cast));
        if crossed_phase {
            self.clear(cast);
        }
    }

    fn clear_completed_round_start_casts(&mut self) {
        let completed = self
            .round_start_casts
            .iter()
            .copied()
            .filter(|cast| !self.held.contains(cast))
            .collect::<Vec<_>>();
        for cast in completed {
            self.round_start_casts.remove(&cast);
            self.clear(cast);
        }
    }

    fn clear(&mut self, (owner_uid, key): CastKey) {
        self.progress
            .retain(|(owner, progress_key, _), _| *owner != owner_uid || *progress_key != key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::skill::rule::DefinitionKey;

    const RULE: ConfiguredRuleKey =
        ConfiguredRuleKey::new(31090144, 2, DefinitionKey::new(60135, "ShellUseSkill"));

    #[test]
    fn completed_cast_progress_resets_at_its_next_phase_but_untouched_progress_carries() {
        let advance = |manager: &mut ConfiguredRuleProgressManager, amount| {
            manager.advance_until_cast(10, RULE, 7, amount)
        };

        let mut completed = ConfiguredRuleProgressManager::default();
        assert!(!advance(&mut completed, 6));
        completed.begin_round_start();
        assert!(advance(&mut completed, 1));
        completed.release(10, RULE);
        assert!(!advance(&mut completed, 2));
        completed.begin_action_round();
        assert!(!advance(&mut completed, 6));
        assert!(advance(&mut completed, 1));

        let mut untouched = ConfiguredRuleProgressManager::default();
        assert!(!advance(&mut untouched, 6));
        untouched.begin_round_start();
        untouched.begin_action_round();
        assert!(advance(&mut untouched, 1));

        let mut pending = ConfiguredRuleProgressManager::default();
        assert!(!advance(&mut pending, 6));
        pending.begin_round_start();
        assert!(advance(&mut pending, 1));
        pending.begin_action_round();
        assert!(!advance(&mut pending, 7));
        pending.release(10, RULE);
        assert!(!advance(&mut pending, 6));
        assert!(advance(&mut pending, 1));

        let mut cloth = ConfiguredRuleProgressManager::default();
        cloth.begin_action_round();
        assert!(advance(&mut cloth, 7));
        cloth.release(10, RULE);
        assert!(!advance(&mut cloth, 2));
        cloth.begin_round_start();
        cloth.begin_cloth_sequence();
        assert!(!advance(&mut cloth, 6));
        cloth.begin_cloth_sequence();
        assert!(advance(&mut cloth, 1));
        cloth.release(10, RULE);
        assert!(!advance(&mut cloth, 2));
        cloth.begin_action_round();
        assert!(!advance(&mut cloth, 4));
        assert!(advance(&mut cloth, 1));
    }
}
