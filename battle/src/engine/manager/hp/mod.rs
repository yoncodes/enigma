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
    pub ignore_riposte: bool,
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
pub struct FakeHpConsumption {
    pub buff_uid: i64,
    pub consumed: i32,
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
    SettleAccumulatedDamage(HpDamage),
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
pub struct PendingDeath {
    transition: DeathTransition,
    revision: u64,
}

impl PendingDeath {
    pub(crate) fn transition(self) -> DeathTransition {
        self.transition
    }
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

/// A hit split by ShareHurt before it landed: the consumed stack and each ally's share.
#[derive(Debug, Clone, PartialEq)]
pub struct SharedHurt {
    pub consumed: crate::engine::manager::buff::BuffChanges,
    pub shares: Vec<HpChanges>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HpChanges {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub damage: Option<DamageRecord>,
    pub publishes_hit: bool,
    pub fake_hp_consumed: Option<FakeHpConsumption>,
    pub team_shared_shield_absorbed: Option<TeamSharedShieldAbsorption>,
    pub team_shared_shield_removed: Option<crate::engine::manager::buff::BuffChanges>,
    pub shared_hurt: Option<Box<SharedHurt>>,
    pub shield_absorbed: Option<ShieldChange>,
    pub shield_granted: Option<ShieldGain>,
    pub max_hp: Option<MaxHpChange>,
    pub hp: Option<HpChange>,
    pub toughness: Option<super::toughness::ToughnessChange>,
    pub kill: Option<i32>,
    pub death: Option<PendingDeath>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageRecord {
    pub amount: i32,
    pub config_effect: i32,
    pub effect_kind: DamageEffectKind,
    pub assassinate: bool,
    pub ignore_riposte: bool,
    pub hurt: HurtInfoData,
}

impl HpChanges {
    /// Every HP change this commit made: ShareHurt shares first, then this change.
    pub fn with_shares(&self) -> impl Iterator<Item = &HpChanges> {
        self.shared_hurt
            .iter()
            .flat_map(|shared| shared.shares.iter())
            .chain(std::iter::once(self))
    }

    pub fn take_deaths_with_shares(&mut self) -> Vec<PendingDeath> {
        let mut deaths = self
            .shared_hurt
            .iter_mut()
            .flat_map(|shared| shared.shares.iter_mut())
            .filter_map(|share| share.death.take())
            .collect::<Vec<_>>();
        deaths.extend(self.death.take());
        deaths
    }

    pub fn caused_death(&self) -> bool {
        self.hp
            .is_some_and(|change| change.before > 0 && change.after == 0)
    }

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
        let share_count = self
            .shared_hurt
            .iter()
            .flat_map(|shared| &shared.shares)
            .filter(|share| share.hp.is_some_and(|change| change.delta < 0))
            .count() as i32;
        if let Some(shared) = &self.shared_hurt {
            events.extend(shared.consumed.events());
            for share in &shared.shares {
                events.extend(share.events());
            }
            let amount = shared
                .shares
                .iter()
                .filter_map(|share| share.hp.filter(|change| change.delta < 0))
                .map(|change| change.delta.saturating_abs())
                .sum();
            if amount > 0 {
                events.push(BattleEvent::DamageShared {
                    origin: shared
                        .shares
                        .first()
                        .map_or(self.origin, |share| share.origin),
                    source_uid: self.source_uid,
                    target_uid: self.target_uid,
                    participant_uids: shared
                        .shares
                        .iter()
                        .filter(|share| share.hp.is_some_and(|change| change.delta < 0))
                        .map(|share| share.target_uid)
                        .collect(),
                    amount,
                    share_count,
                    action_share_count: share_count,
                    damage_from: self
                        .damage
                        .map(|damage| damage.hurt.damage_from)
                        .expect("shared damage event requires parent damage provenance"),
                });
            }
        }
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
            self.publishes_hit
                && damage.effect_kind != DamageEffectKind::Avoided
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
                resolved_damage: damage.amount,
                shield_absorbed: self
                    .shield_absorbed
                    .map(|change| change.absorbed)
                    .unwrap_or_default()
                    .saturating_add(
                        self.team_shared_shield_absorbed
                            .map(|change| change.absorbed)
                            .unwrap_or_default(),
                    ),
                career_restraint: damage.hurt.career_restraint,
                damage_from: damage.hurt.damage_from,
                share_count,
                assassinate: damage.assassinate,
                ignore_riposte: damage.ignore_riposte,
            }));
        }
        if self.toughness.is_some_and(|change| change.broke) {
            events.push(BattleEvent::ToughnessBroken {
                source_uid: self.source_uid,
                target_uid: self.target_uid,
                skill_id: self
                    .damage
                    .map(|damage| damage.hurt.skill_id)
                    .unwrap_or_default(),
            });
        }
        if let Some(removed) = &self.team_shared_shield_removed {
            events.extend(removed.events());
        }
        if let Some(death) = self.death {
            let death = death.transition();
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
    InvalidFoundationState,
    UnsupportedFoundationInteraction,
    MissingTarget(i64),
    InvalidTeamSharedState,
    InvalidShareHurtState,
    InvalidHpLossState,
}

#[derive(Debug, Clone, Default)]
pub struct HpManager {
    states: HashMap<i64, HpState>,
    revisions: HashMap<i64, u64>,
    action_start: HashMap<(i64, i32), HpState>,
    shields: HashMap<i64, i32>,
    damage_taken: HashMap<i64, i32>,
    skill_damage_taken: HashMap<(i64, i64), i32>,
    total_damage_dealt: HashMap<i64, i64>,
    total_damage_taken: HashMap<i64, i64>,
    total_healing_done: HashMap<i64, i64>,
    order: Vec<i64>,
}

mod command;
mod state;

#[cfg(test)]
mod tests;
