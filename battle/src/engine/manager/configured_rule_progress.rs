use std::collections::{HashMap, HashSet};

use crate::engine::skill::rule::ConfiguredRuleKey;

type CastKey = (i64, ConfiguredRuleKey);

#[derive(Debug, Clone, Default)]
pub(super) struct ConfiguredRuleProgressManager {
    progress: HashMap<(i64, ConfiguredRuleKey, i32), i32>,
    held: HashSet<CastKey>,
    round_start_casts: HashSet<CastKey>,
    in_round_start: bool,
}

impl ConfiguredRuleProgressManager {
    pub(super) fn begin_round_start(&mut self) {
        self.round_start_casts.clear();
        self.in_round_start = true;
    }

    pub(super) fn begin_action_round(&mut self) {
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
        self.in_round_start = false;
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
        }
        true
    }

    pub(super) fn release(&mut self, owner_uid: i64, key: ConfiguredRuleKey) {
        let cast = (owner_uid, key);
        self.held.remove(&cast);
        if !self.in_round_start && self.round_start_casts.remove(&cast) {
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
    fn round_start_cast_resets_before_actions_but_untouched_progress_carries() {
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
    }
}
