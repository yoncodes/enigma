use std::collections::HashMap;

use sonettobuf::{Fight, FightEntityInfo};

use crate::engine::{
    event::payload::{BattleEvent, EntityDiedEvent, HitEvent},
    skill::rule::CommandOrigin,
};

use super::entities;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HpState {
    pub current: i32,
    pub max: i32,
    pub base_max: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HurtDamageFromType {
    None,
    Skill,
    SkillEffect,
    Buff,
    Additional,
    AbsorbHurt,
    ShareHurt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HurtInfoData {
    pub from_uid: i64,
    pub is_crit: bool,
    pub career_restraint: bool,
    pub reduce_hp: i32,
    pub effect_id: i32,
    pub skill_id: i32,
    pub damage_from: HurtDamageFromType,
    pub buff_act_id: i32,
    pub buff_uid: i64,
    pub hurt_effect_type: i32,
    pub display_amount: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HpChange {
    pub target_uid: i64,
    pub before: i32,
    pub delta: i32,
    pub after: i32,
    pub max: i32,
    pub config_effect: i32,
    pub hurt: Option<HurtInfoData>,
    pub assassinate: bool,
    pub effect_type: i32,
    pub display_amount: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShieldChange {
    pub target_uid: i64,
    pub buff_uid: i64,
    pub before: i32,
    pub absorbed: i32,
    pub after: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShieldGain {
    pub target_uid: i64,
    pub before: i32,
    pub added: i32,
    pub after: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageEffectKind {
    Normal,
    Critical,
    Genesis,
    Avoided,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HpDamage {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub amount: i32,
    pub config_effect: i32,
    pub effect_kind: DamageEffectKind,
    pub assassinate: bool,
    pub hurt: HurtInfoData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HpLoss {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub amount: i32,
    pub config_effect: i32,
    pub hurt: Option<HurtInfoData>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HpKill {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub config_effect: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HpHeal {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub amount: i32,
    pub config_effect: i32,
    pub kind: HpHealKind,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HpHealKind {
    #[default]
    Normal,
    Critical,
    Bloodlust,
    Revive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurrentHpSet {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub value: i32,
    pub config_effect: i32,
    pub effect_type: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShieldGrant {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub amount: i32,
    pub max: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TeamSharedShieldGain {
    pub buff_uid: i64,
    pub owner_uid: i64,
    pub buff_act_id: i32,
    pub before: i32,
    pub added: i32,
    pub after: i32,
    pub max: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TeamSharedShieldAbsorption {
    pub buff_uid: i64,
    pub owner_uid: i64,
    pub buff_act_id: i32,
    pub before: i32,
    pub consumed: i32,
    pub absorbed: i32,
    pub after: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TeamSharedShieldPlan {
    pub buff_uid: i64,
    pub owner_uid: i64,
    pub buff_act_id: i32,
    pub current: i32,
    pub block_rate: i32,
}

impl TeamSharedShieldPlan {
    pub(crate) fn absorption(self, damage: i32) -> Option<TeamSharedShieldAbsorption> {
        if self.current <= 0 || damage <= 0 || self.block_rate <= 0 {
            return None;
        }
        let required = (i64::from(damage) * 1000 + i64::from(self.block_rate) - 1)
            / i64::from(self.block_rate);
        let consumed = self
            .current
            .min(required.clamp(0, i64::from(i32::MAX)) as i32);
        let absorbed = damage.min(
            (i64::from(consumed) * i64::from(self.block_rate) / 1000).clamp(0, i64::from(i32::MAX))
                as i32,
        );
        (absorbed > 0).then_some(TeamSharedShieldAbsorption {
            buff_uid: self.buff_uid,
            owner_uid: self.owner_uid,
            buff_act_id: self.buff_act_id,
            before: self.current,
            consumed,
            absorbed,
            after: self.current - consumed,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaxHpAdjust {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub delta: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HpCommand {
    Damage(HpDamage),
    Lose(HpLoss),
    Kill(HpKill),
    Heal(HpHeal),
    SetCurrent(CurrentHpSet),
    GrantShield(ShieldGrant),
    AdjustMax(MaxHpAdjust),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeathTransition {
    pub source_uid: i64,
    pub target_uid: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaxHpChange {
    pub target_uid: i64,
    pub before_current: i32,
    pub before_max: i32,
    pub delta: i32,
    pub after_current: i32,
    pub after_max: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HpChanges {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub damage: Option<DamageRecord>,
    pub team_shared_shield_absorbed: Option<TeamSharedShieldAbsorption>,
    pub team_shared_shield_removed: Option<crate::engine::manager::buff::BuffChanges>,
    pub shield_absorbed: Option<ShieldChange>,
    pub shield_granted: Option<ShieldGain>,
    pub max_hp: Option<MaxHpChange>,
    pub hp: Option<HpChange>,
    pub kill: Option<i32>,
    pub death: Option<DeathTransition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageRecord {
    pub amount: i32,
    pub config_effect: i32,
    pub effect_kind: DamageEffectKind,
    pub assassinate: bool,
    pub hurt: HurtInfoData,
}

impl HpChanges {
    pub fn applied_damage(&self) -> i32 {
        if self.kill.is_some() {
            return 0;
        }
        self.shield_absorbed
            .map(|change| change.absorbed)
            .unwrap_or_default()
            + self
                .team_shared_shield_absorbed
                .map(|change| change.absorbed)
                .unwrap_or_default()
            + self
                .hp
                .filter(|change| change.delta < 0)
                .map(|change| change.delta.saturating_abs())
                .unwrap_or_default()
    }

    pub fn events(&self) -> Vec<BattleEvent> {
        let mut events = Vec::with_capacity(3);
        if self.kill.is_none()
            && let Some(change) = self.hp.filter(|change| change.delta < 0)
        {
            events.push(BattleEvent::HpLost {
                origin: self.origin,
                source_uid: self.source_uid,
                skill_id: change.hurt.map(|hurt| hurt.skill_id).unwrap_or_default(),
                target_uid: self.target_uid,
                amount: change.delta.saturating_abs(),
                buff_uid: change
                    .hurt
                    .map(|hurt| hurt.buff_uid)
                    .filter(|buff_uid| *buff_uid != 0),
            });
        }
        if let Some(change) = self.hp.filter(|change| change.delta > 0) {
            events.push(BattleEvent::HpHealed {
                origin: self.origin,
                source_uid: self.source_uid,
                target_uid: self.target_uid,
                amount: change.delta,
            });
        }
        if let Some(damage) = self.damage.filter(|damage| {
            damage.effect_kind != DamageEffectKind::Avoided
                && damage.hurt.damage_from != HurtDamageFromType::Buff
        }) {
            events.push(BattleEvent::Hit(HitEvent {
                origin: self.origin,
                source_uid: self.source_uid,
                target_uid: self.target_uid,
                skill_id: damage.hurt.skill_id,
                amount: self
                    .hp
                    .filter(|change| change.delta < 0)
                    .map(|change| change.delta.saturating_abs())
                    .unwrap_or_default(),
                shield_absorbed: self
                    .shield_absorbed
                    .map(|change| change.absorbed)
                    .unwrap_or_default()
                    + self
                        .team_shared_shield_absorbed
                        .map(|change| change.absorbed)
                        .unwrap_or_default(),
                damage_from: damage.hurt.damage_from,
                assassinate: damage.assassinate,
            }));
        }
        if let Some(removed) = &self.team_shared_shield_removed {
            events.extend(removed.events());
        }
        if let Some(death) = self.death {
            events.push(BattleEvent::EntityDied(EntityDiedEvent {
                source_uid: death.source_uid,
                target_uid: death.target_uid,
            }));
        }
        events
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HpCommandError {
    InvalidCommand,
    MissingTarget(i64),
    InvalidTeamSharedState,
}

#[derive(Debug, Clone, Default)]
pub struct HpManager {
    states: HashMap<i64, HpState>,
    action_start: HashMap<(i64, i32), HpState>,
    shields: HashMap<i64, i32>,
    damage_taken: HashMap<i64, i32>,
    skill_damage_taken: HashMap<(i64, i64), i32>,
    total_damage_dealt: HashMap<i64, i64>,
    total_damage_taken: HashMap<i64, i64>,
    total_healing_done: HashMap<i64, i64>,
    order: Vec<i64>,
}

impl HpManager {
    pub fn seed(&mut self, fight: &Fight) {
        self.states.clear();
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
        self.states.insert(
            uid,
            HpState {
                current,
                max,
                base_max: max,
            },
        );
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

    fn absorb_team_shared_shield(
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
        if state.base_max == 0 {
            state.base_max = max.max(0);
        }
        state.max = max.max(0);
        state.current = state.current.min(state.max);
    }

    pub fn add_max_snapshot(&mut self, uid: i64, delta: i32) -> HpState {
        let state = self.states.entry(uid).or_default();
        if state.base_max == 0 {
            state.base_max = state.max;
        }
        state.max = (state.max + delta).max(0);
        state.current = (state.current + delta).clamp(0, state.max);
        *state
    }

    pub fn sync_entity(&self, entity: &mut FightEntityInfo) {
        let Some(uid) = entity.uid else { return };
        let state = self.get(uid);
        entity.current_hp = Some(state.current);
        if let Some(attr) = entity.attr.as_mut() {
            attr.hp = Some(state.max);
        }
        entity.shield_value = Some(self.shield(uid));
    }

    #[cfg(test)]
    pub(crate) fn execute_command(
        &mut self,
        command: HpCommand,
    ) -> Result<HpChanges, HpCommandError> {
        self.validate_command(command)?;
        Ok(self.commit_validated_command_with_team_shared(command, None))
    }

    pub(crate) fn commit_validated_command_with_team_shared(
        &mut self,
        command: HpCommand,
        team_shared: Option<TeamSharedShieldPlan>,
    ) -> HpChanges {
        debug_assert!(self.validate_command(command).is_ok());
        let (origin, source_uid, target_uid, _) = Self::command_context(command);
        let before = self.current(target_uid);
        let mut changes = HpChanges {
            origin,
            source_uid,
            target_uid,
            damage: None,
            team_shared_shield_absorbed: None,
            team_shared_shield_removed: None,
            shield_absorbed: None,
            shield_granted: None,
            max_hp: None,
            hp: None,
            kill: None,
            death: None,
        };
        match command {
            HpCommand::Damage(value) => {
                changes.damage = Some(DamageRecord {
                    amount: value.amount,
                    config_effect: value.config_effect,
                    effect_kind: value.effect_kind,
                    assassinate: value.assassinate,
                    hurt: value.hurt,
                });
                changes.team_shared_shield_absorbed = team_shared.and_then(|plan| {
                    self.absorb_team_shared_shield(plan, target_uid, value.amount)
                });
                let after_team_shared = value.amount
                    - changes
                        .team_shared_shield_absorbed
                        .map(|change| change.absorbed)
                        .unwrap_or_default();
                changes.shield_absorbed = self.absorb_shield(target_uid, after_team_shared);
                let remaining = value.amount
                    - changes
                        .team_shared_shield_absorbed
                        .map(|change| change.absorbed)
                        .unwrap_or_default()
                    - changes
                        .shield_absorbed
                        .map(|change| change.absorbed)
                        .unwrap_or_default();
                if remaining > 0 {
                    let mut hurt = value.hurt;
                    hurt.from_uid = source_uid;
                    hurt.is_crit = value.effect_kind == DamageEffectKind::Critical;
                    changes.hp =
                        self.lose_with_hurt(target_uid, remaining, value.config_effect, Some(hurt));
                    if let Some(change) = &mut changes.hp {
                        change.assassinate = value.assassinate;
                    }
                }
                if value.hurt.damage_from == HurtDamageFromType::Skill {
                    *self
                        .skill_damage_taken
                        .entry((source_uid, target_uid))
                        .or_default() += changes.applied_damage();
                }
            }
            HpCommand::Lose(value) => {
                changes.hp =
                    self.lose_with_hurt(target_uid, value.amount, value.config_effect, value.hurt);
            }
            HpCommand::Kill(value) => {
                changes.hp = self.lose(target_uid, before, value.config_effect);
                changes.kill = Some(value.config_effect);
            }
            HpCommand::Heal(value) => {
                changes.hp = self.heal(target_uid, value.amount, value.config_effect);
                if let Some(change) = &mut changes.hp {
                    change.effect_type = match value.kind {
                        HpHealKind::Normal => 0,
                        HpHealKind::Critical => {
                            sonettobuf::effect_type_enum::EffectType::Healcrit as i32
                        }
                        HpHealKind::Bloodlust => {
                            sonettobuf::effect_type_enum::EffectType::Bloodlust as i32
                        }
                        HpHealKind::Revive => sonettobuf::effect_type_enum::EffectType::Cure as i32,
                    };
                }
            }
            HpCommand::SetCurrent(value) => {
                changes.hp =
                    self.apply_delta(target_uid, value.value - before, value.config_effect, None);
                if let Some(change) = &mut changes.hp {
                    change.effect_type = value.effect_type;
                    change.display_amount = Some(change.after);
                }
            }
            HpCommand::GrantShield(value) => {
                changes.shield_granted = Some(self.add_shield(target_uid, value.amount, value.max));
            }
            HpCommand::AdjustMax(value) => {
                let before = self.get(target_uid);
                let after = self.add_max_snapshot(target_uid, value.delta);
                if crate::engine::diagnostics::enabled(
                    crate::engine::diagnostics::TraceArea::Damage,
                ) {
                    eprintln!(
                        "max-hp source={} target={target_uid} requested={} before={}/{} after={}/{} origin={origin:?}",
                        source_uid,
                        value.delta,
                        before.current,
                        before.max,
                        after.current,
                        after.max,
                    );
                }
                changes.max_hp = Some(MaxHpChange {
                    target_uid,
                    before_current: before.current,
                    before_max: before.max,
                    delta: after.max - before.max,
                    after_current: after.current,
                    after_max: after.max,
                });
            }
        }
        if before > 0 && self.current(target_uid) == 0 {
            changes.death = Some(DeathTransition {
                source_uid,
                target_uid,
            });
        }

        let applied_damage = i64::from(changes.applied_damage());
        if applied_damage > 0 {
            *self.total_damage_taken.entry(target_uid).or_default() += applied_damage;
            if source_uid != target_uid
                && (changes.damage.is_some() || changes.hp.and_then(|change| change.hurt).is_some())
            {
                *self.total_damage_dealt.entry(source_uid).or_default() += applied_damage;
            }
        }
        if matches!(command, HpCommand::Heal(_))
            && let Some(healed) = changes.hp.filter(|change| change.delta > 0)
        {
            *self.total_healing_done.entry(source_uid).or_default() += i64::from(healed.delta);
        }
        changes
    }

    fn command_context(command: HpCommand) -> (CommandOrigin, i64, i64, bool) {
        match command {
            HpCommand::Damage(value) => (
                value.origin,
                value.source_uid,
                value.target_uid,
                value.amount > 0
                    || (value.amount == 0 && value.effect_kind == DamageEffectKind::Avoided),
            ),
            HpCommand::Lose(value) => (
                value.origin,
                value.source_uid,
                value.target_uid,
                value.amount > 0,
            ),
            HpCommand::Kill(value) => (
                value.origin,
                value.source_uid,
                value.target_uid,
                value.config_effect > 0,
            ),
            HpCommand::Heal(value) => (
                value.origin,
                value.source_uid,
                value.target_uid,
                value.amount > 0,
            ),
            HpCommand::SetCurrent(value) => (
                value.origin,
                value.source_uid,
                value.target_uid,
                value.value >= 0 && value.effect_type != 0,
            ),
            HpCommand::GrantShield(value) => (
                value.origin,
                value.source_uid,
                value.target_uid,
                value.amount > 0 && value.max > 0,
            ),
            HpCommand::AdjustMax(value) => (
                value.origin,
                value.source_uid,
                value.target_uid,
                value.delta != 0,
            ),
        }
    }

    pub(crate) fn validate_command(&self, command: HpCommand) -> Result<(), HpCommandError> {
        let (_, _, target_uid, valid) = Self::command_context(command);
        if target_uid == 0 || !valid {
            return Err(HpCommandError::InvalidCommand);
        }
        if !self.states.contains_key(&target_uid) {
            return Err(HpCommandError::MissingTarget(target_uid));
        }
        Ok(())
    }

    fn apply_delta(
        &mut self,
        uid: i64,
        delta: i32,
        config_effect: i32,
        mut hurt: Option<HurtInfoData>,
    ) -> Option<HpChange> {
        if delta < 0
            && let Some(hurt) = &mut hurt
            && hurt.display_amount.is_none()
        {
            hurt.display_amount = Some(delta.saturating_abs());
        }
        let state = self.states.entry(uid).or_default();
        let before = state.current;
        let after = (before + delta).clamp(0, state.max.max(0));
        let applied = after - before;
        state.current = after;
        if applied < 0 {
            *self.damage_taken.entry(uid).or_default() += -applied;
            if let Some(hurt) = &mut hurt {
                // The client uses damage for the floating number and reduce_hp
                // for the health-bar delta. Keep the latter tied to the amount
                // the HP manager actually committed, including overkill.
                hurt.reduce_hp = applied;
            }
        }

        Some(HpChange {
            target_uid: uid,
            before,
            delta: applied,
            after,
            max: state.max,
            config_effect,
            hurt,
            assassinate: false,
            effect_type: 0,
            display_amount: Some(delta.saturating_abs()),
        })
    }
}

#[cfg(test)]
mod test;
