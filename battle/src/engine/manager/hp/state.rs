use super::*;

impl HpManager {
    pub fn seed(&mut self, fight: &Fight) {
        self.states.clear();
        self.revisions.clear();
        self.action_start.clear();
        self.shields.clear();
        self.damage_taken.clear();
        self.skill_damage_taken.clear();
        self.total_damage_dealt.clear();
        self.total_damage_taken.clear();
        self.total_healing_done.clear();
        self.order.clear();
        for entity in entities(fight).chain(
            fight
                .attacker
                .iter()
                .chain(fight.defender.iter())
                .filter_map(|team| team.assist_boss.as_ref()),
        ) {
            self.register(entity);
        }
    }

    pub fn register(&mut self, entity: &FightEntityInfo) {
        let Some(uid) = entity.uid else { return };
        let current = entity.current_hp.unwrap_or_default();
        let max = entity
            .attr
            .as_ref()
            .and_then(|attr| attr.hp)
            .unwrap_or(current);
        let replaced = self
            .states
            .insert(
                uid,
                HpState {
                    current,
                    max,
                    base_max: max,
                },
            )
            .is_some();
        let revision = self.revisions.entry(uid).or_default();
        if replaced {
            *revision += 1;
        }
        self.shields
            .insert(uid, entity.shield_value.unwrap_or_default().max(0));
        if !self.order.contains(&uid) {
            self.order.push(uid);
        }
    }

    pub fn get(&self, uid: i64) -> HpState {
        self.states.get(&uid).copied().unwrap_or_default()
    }

    pub fn capture_action_start(&mut self, uid: i64, skill_id: i32) {
        self.action_start.insert((uid, skill_id), self.get(uid));
    }

    pub fn action_start(&self, uid: i64, skill_id: i32) -> Option<HpState> {
        self.action_start.get(&(uid, skill_id)).copied()
    }

    pub fn current(&self, uid: i64) -> i32 {
        self.get(uid).current
    }

    pub(crate) fn is_current_death(&self, death: PendingDeath) -> bool {
        self.current(death.transition.target_uid) == 0
            && self
                .revisions
                .get(&death.transition.target_uid)
                .copied()
                .unwrap_or_default()
                == death.revision
    }

    pub fn max(&self, uid: i64) -> i32 {
        self.get(uid).max
    }

    pub fn base_max(&self, uid: i64) -> i32 {
        let state = self.get(uid);
        if state.base_max > 0 {
            state.base_max
        } else {
            state.max
        }
    }

    pub(crate) fn resync_current(&mut self, uid: i64, current: i32) {
        if let Some(state) = self.states.get_mut(&uid) {
            let before = state.current;
            state.current = current.clamp(0, state.max);
            if before != state.current {
                *self.revisions.entry(uid).or_default() += 1;
            }
        }
    }

    pub fn shield(&self, uid: i64) -> i32 {
        self.shields.get(&uid).copied().unwrap_or_default()
    }

    pub fn set_shield(&mut self, uid: i64, value: i32) {
        self.shields.insert(uid, value.max(0));
    }

    pub fn add_shield(&mut self, uid: i64, amount: i32, max: i32) -> ShieldGain {
        let shield = self.shields.entry(uid).or_default();
        let before = *shield;
        *shield = (before + amount.max(0)).min(max.max(0));
        ShieldGain {
            target_uid: uid,
            before,
            added: *shield - before,
            after: *shield,
        }
    }

    pub fn absorb_shield(&mut self, uid: i64, damage: i32) -> Option<ShieldChange> {
        let shield = self.shields.entry(uid).or_default();
        let before = *shield;
        let absorbed = damage.max(0).min(before);
        if absorbed == 0 {
            return None;
        }
        *shield -= absorbed;
        *self.damage_taken.entry(uid).or_default() += absorbed;
        Some(ShieldChange {
            target_uid: uid,
            buff_uid: 0,
            before,
            absorbed,
            after: *shield,
        })
    }

    pub(super) fn absorb_team_shared_shield(
        &mut self,
        plan: TeamSharedShieldPlan,
        target_uid: i64,
        damage: i32,
    ) -> Option<TeamSharedShieldAbsorption> {
        let absorption = plan.absorption(damage)?;
        *self.damage_taken.entry(target_uid).or_default() += absorption.absorbed;
        Some(absorption)
    }

    pub fn lose(&mut self, uid: i64, amount: i32, config_effect: i32) -> Option<HpChange> {
        self.lose_with_hurt(uid, amount, config_effect, None)
    }

    pub fn lose_with_hurt(
        &mut self,
        uid: i64,
        amount: i32,
        config_effect: i32,
        hurt: Option<HurtInfoData>,
    ) -> Option<HpChange> {
        if amount <= 0 {
            return None;
        }

        self.apply_delta(uid, -amount, config_effect, hurt)
    }

    pub fn heal(&mut self, uid: i64, amount: i32, config_effect: i32) -> Option<HpChange> {
        if amount <= 0 {
            return None;
        }

        self.apply_delta(uid, amount, config_effect, None)
    }

    pub fn take_damage_taken(&mut self) -> Vec<(i64, i32)> {
        let mut damage = std::mem::take(&mut self.damage_taken);
        self.order
            .iter()
            .filter_map(|uid| damage.remove(uid).map(|amount| (*uid, amount)))
            .collect()
    }

    pub fn begin_round(&mut self) {
        self.skill_damage_taken.clear();
    }

    pub fn skill_damage_from_sources(&self, target_uid: i64, source_uids: &[i64]) -> i32 {
        source_uids
            .iter()
            .map(|source_uid| {
                self.skill_damage_taken
                    .get(&(*source_uid, target_uid))
                    .copied()
                    .unwrap_or_default()
            })
            .sum()
    }

    pub fn total_damage_dealt(&self, uid: i64) -> i64 {
        self.total_damage_dealt
            .get(&uid)
            .copied()
            .unwrap_or_default()
    }

    pub fn total_damage_taken(&self, uid: i64) -> i64 {
        self.total_damage_taken
            .get(&uid)
            .copied()
            .unwrap_or_default()
    }

    pub fn total_healing_done(&self, uid: i64) -> i64 {
        self.total_healing_done
            .get(&uid)
            .copied()
            .unwrap_or_default()
    }

    pub fn set_max(&mut self, uid: i64, max: i32) {
        let state = self.states.entry(uid).or_default();
        let before = state.current;
        if state.base_max == 0 {
            state.base_max = max.max(0);
        }
        state.max = max.max(0);
        state.current = state.current.min(state.max);
        if before != state.current {
            *self.revisions.entry(uid).or_default() += 1;
        }
    }

    pub fn add_max_snapshot(&mut self, uid: i64, delta: i32) -> HpState {
        let state = self.states.entry(uid).or_default();
        let before = state.current;
        if state.base_max == 0 {
            state.base_max = state.max;
        }
        state.max = (state.max + delta).max(0);
        state.current = if delta >= 0 {
            (state.current + delta).min(state.max)
        } else {
            state.current.min(state.max)
        };
        if before != state.current {
            *self.revisions.entry(uid).or_default() += 1;
        }
        *state
    }
}
