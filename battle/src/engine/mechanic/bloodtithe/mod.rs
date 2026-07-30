use std::collections::{HashMap, HashSet};

use crate::engine::{
    manager::buff::ActiveBuffFeature,
    skill::buff_act::{is_kind, registry::BuffActKind},
};

pub mod hp_loss;
pub mod rule;
pub mod spend;

pub use hp_loss::threshold as hp_loss_threshold;

const BASE_MAX: i32 = 24;
const PER_ENABLER_MAX_BONUS: i32 = 16;

#[derive(Debug, Clone, Default)]
pub struct Bloodtithe {
    values: HashMap<i32, i32>,
    max_values: HashMap<i32, i32>,
    pending_max_deltas: HashMap<i32, i32>,
    seeded_teams: HashSet<i32>,
    initialized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BloodtitheApplyResult {
    pub source_uid: i64,
    pub team: i32,
    pub before: i32,
    pub requested_delta: i32,
    pub applied_delta: i32,
    pub after: i32,
    pub changes_max: bool,
    pub config_effect: i32,
}

impl Bloodtithe {
    pub fn initialized(&self) -> bool {
        self.initialized
    }

    pub fn team_initialized(&self, team: i32) -> bool {
        self.seeded_teams.contains(&team)
    }

    pub fn value(&self, team: i32) -> i32 {
        self.values.get(&team).copied().unwrap_or_default()
    }

    pub fn max(&self, team: i32) -> i32 {
        self.max_values.get(&team).copied().unwrap_or_default()
    }

    pub fn seed_max_once(&mut self, team: i32, enabler_count: i32) -> i32 {
        if enabler_count <= 0 {
            return 0;
        }
        if !self.seeded_teams.insert(team) {
            return 0;
        }
        self.initialized = true;
        let before = self.max(team);
        let after = BASE_MAX + enabler_count.max(0) * PER_ENABLER_MAX_BONUS;
        self.max_values.insert(team, after.max(0));
        if self.value(team) > after {
            self.values.insert(team, after);
        }
        self.max(team) - before
    }

    pub fn init_team_from_features(
        &mut self,
        team: i32,
        features: &[ActiveBuffFeature],
    ) -> Option<i32> {
        let enabler_count = shared_blood_pool_tag_owner_count(team, features);
        (enabler_count > 0).then(|| self.seed_max_once(team, enabler_count))
    }

    pub fn seed_max_from_features(&mut self, team: i32, features: &[ActiveBuffFeature]) -> i32 {
        self.init_team_from_features(team, features)
            .unwrap_or_default()
    }

    pub fn apply_max(&mut self, team: i32, delta: i32) -> i32 {
        if !self.team_initialized(team) {
            return 0;
        }
        let before = self.max(team);
        let after = (before + delta).max(0);
        self.max_values.insert(team, after);
        if self.value(team) > after {
            self.values.insert(team, after);
        }
        let applied = after - before;
        if applied != 0 {
            *self.pending_max_deltas.entry(team).or_default() += applied;
        }
        applied
    }

    pub fn apply_value(&mut self, team: i32, delta: i32) -> i32 {
        if !self.team_initialized(team) {
            return 0;
        }
        let before = self.value(team);
        let after = (before + delta).clamp(0, self.max(team).max(0));
        self.values.insert(team, after);
        after - before
    }

    pub fn take_pending_max_delta(&mut self, team: i32) -> i32 {
        self.pending_max_deltas.remove(&team).unwrap_or_default()
    }
}

fn shared_blood_pool_tag_owner_count(team: i32, features: &[ActiveBuffFeature]) -> i32 {
    let mut tagged: Vec<(i64, Vec<i32>)> = Vec::new();
    for feature in features.iter().filter(|feature| {
        feature.team_type == team
            && feature.owner_alive
            && is_kind(feature, BuffActKind::BloodPoolTag)
    }) {
        if let Some((_, buff_ids)) = tagged
            .iter_mut()
            .find(|(owner_uid, _)| *owner_uid == feature.owner_uid)
        {
            if !buff_ids.contains(&feature.buff_id) {
                buff_ids.push(feature.buff_id);
            }
        } else {
            tagged.push((feature.owner_uid, vec![feature.buff_id]));
        }
    }

    tagged
        .iter()
        .enumerate()
        .filter(|(_, (_, current))| !current.is_empty())
        .filter(|(index, (_, current))| {
            tagged.iter().enumerate().any(|(other_index, (_, other))| {
                other_index != *index && current.iter().any(|buff_id| other.contains(buff_id))
            })
        })
        .count() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recalculates_blood_pool_max_from_configured_enablers() {
        let mut bloodtithe = Bloodtithe::default();

        assert_eq!(bloodtithe.seed_max_once(1, 2), 56);
        assert_eq!(bloodtithe.seed_max_once(1, 4), 0);
        assert_eq!(bloodtithe.seed_max_once(2, 0), 0);
        assert_eq!(bloodtithe.max(1), 56);
        assert_eq!(bloodtithe.max(2), 0);
        assert_eq!(bloodtithe.take_pending_max_delta(1), 0);
    }

    #[test]
    fn seeds_max_from_shared_blood_pool_tag_features() {
        let mut bloodtithe = Bloodtithe::default();
        let features = vec![
            feature(10, 1, 6270501, vec![953]),
            feature(11, 1, 6270501, vec![953]),
            feature(12, 1, 31250131, vec![953]),
        ];

        assert_eq!(bloodtithe.seed_max_from_features(1, &features), 56);
    }

    fn feature(
        owner_uid: i64,
        team_type: i32,
        buff_id: i32,
        values: Vec<i32>,
    ) -> ActiveBuffFeature {
        ActiveBuffFeature {
            owner_uid,
            source_uid: owner_uid,
            buff_uid: owner_uid,
            buff_id,
            amount: 1,
            team_type,
            owner_alive: true,
            act_type: "BloodPoolTag".to_owned(),
            effect_time: 0,
            effect_condition: 0,
            raw: values
                .iter()
                .map(i32::to_string)
                .collect::<Vec<_>>()
                .join("#"),
            values,
        }
    }

    #[test]
    fn ignores_pool_mutations_until_team_is_enabled() {
        let mut bloodtithe = Bloodtithe::default();

        assert_eq!(bloodtithe.apply_max(1, 16), 0);
        assert_eq!(bloodtithe.apply_value(1, 16), 0);
        assert_eq!(bloodtithe.max(1), 0);
        assert_eq!(bloodtithe.value(1), 0);
    }
}
