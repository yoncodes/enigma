pub mod attribute;
pub mod battle_rule;
pub mod buff;
pub mod card;
pub mod conduit;
mod configured_rule_progress;
pub mod contract;
pub mod emanation;
pub mod emitter;
pub mod entity;
pub mod eureka;
pub mod ex_point;
pub mod field;
pub mod fight_progress;
pub mod gauge;
pub mod hp;
pub mod indicator;
pub mod injury;
pub mod revive;
pub mod shield;
pub mod summon;
pub mod toughness;
pub mod upgrade;
pub mod wave;

use sonettobuf::{
    EnhanceInfoBox, Fight, FightEntityInfo, FightExPointInfo, FightHeroSpAttributeInfo,
    HeroSpAttribute,
};
use std::collections::HashMap;

use crate::catalog::MonsterResistances;

use self::{
    attribute::AttributeManager,
    buff::{
        BuffChanges, BuffCommand, BuffCommandError, BuffGrant, BuffManager, BuffPlan, BuffRemove,
        BuffRemoveSelector, BuffSetState,
    },
    card::CardManager,
    conduit::ConduitManager,
    emanation::EmanationManager,
    emitter::EmitterManager,
    eureka::{EurekaCommand, EurekaManager},
    ex_point::{ExPointKind, ExPointManager},
    hp::HpManager,
    summon::SummonManager,
    upgrade::UpgradeManager,
};

/// Central ownership boundary for all durable battle state.
///
/// Skill handlers emit typed commands; only the relevant manager commits the
/// mutation and returns semantic changes for events and packet projection.
#[derive(Debug, Clone, Default)]
pub struct BattleManagers {
    catalog_data: Option<crate::catalog::BattleCatalog>,
    fight_version: i32,
    terminal_outcome: Option<crate::engine::round::outcome::BattleOutcome>,
    pub attribute: AttributeManager,
    pub battle_rule: battle_rule::BattleRuleManager,
    pub(crate) buff: BuffManager,
    pub card: CardManager,
    pub conduit: ConduitManager,
    pub contract: contract::ContractManager,
    pub emitter: EmitterManager,
    pub entity: entity::EntityManager,
    pub emanation: EmanationManager,
    pub hp: HpManager,
    pub indicator: indicator::IndicatorManager,
    pub injury: injury::InjuryManager,
    pub ex_point: ExPointManager,
    pub eureka: EurekaManager,
    pub gauge: gauge::GaugeManager,
    pub nuo_di_ka: crate::engine::mechanic::nuo_di_ka::NuoDiKa,
    pub field: field::FieldManager,
    pub fight_progress: fight_progress::FightProgressManager,
    pub upgrade: UpgradeManager,
    pub summon: SummonManager,
    pub toughness: toughness::ToughnessManager,
    pub wave: wave::WaveManager,
    configured_rule_progress: configured_rule_progress::ConfiguredRuleProgressManager,
    rule_fires: HashMap<(i64, i32, crate::engine::skill::effect::RuleLimitOwner), i32>,
    round_rule_fires: HashMap<(i64, i32, crate::engine::skill::effect::RuleLimitOwner), i32>,
    buff_act_fires: HashMap<(i64, i64, crate::engine::skill::rule::DefinitionKey), i32>,
    rule_progress: HashMap<(i64, i64, crate::engine::skill::rule::DefinitionKey, i32), i32>,
}

struct HpPlan {
    command: hp::HpCommand,
    fake_hp_consumed: Option<hp::FakeHpConsumption>,
    team_shared: Option<hp::TeamSharedShieldPlan>,
    team_shared_buff: Option<BuffPlan>,
    share: Option<ShareHurtPlan>,
    hp_loss_state: Vec<HpLossStatePlan>,
}

struct ShareHurtPlan {
    consume: BuffPlan,
    shares: Vec<hp::HpCommand>,
}

struct HpLossStatePlan {
    buff: BuffPlan,
    marker: buff::BuffActInfoMarkerResult,
}

pub(crate) fn persistent_attribute_delta(
    buffs: &BuffManager,
    hp: &HpManager,
    uid: i64,
    attr_id: crate::engine::entity::attr::AttrId,
) -> i32 {
    use crate::engine::skill::buff_act::{self, registry::BuffActKind};

    let active_features = buffs.active_features(hp);
    buffs.attribute_delta(uid, attr_id)
        + active_features
            .iter()
            .filter_map(|feature| {
                if feature.owner_uid != uid {
                    return None;
                }
                let scope = if buff_act::is_kind(feature, BuffActKind::AddAttrByOtherBuffLayer) {
                    buff_act::add_attr_by_other_buff_layer::LayerScope::SourceOrOwner
                } else if buff_act::is_kind(feature, BuffActKind::AddAttrBySourceBuffLayer) {
                    buff_act::add_attr_by_other_buff_layer::LayerScope::Source
                } else {
                    return None;
                };
                Some(buff_act::add_attr_by_other_buff_layer::attribute_delta(
                    feature, attr_id, buffs, scope,
                ))
            })
            .sum::<i32>()
        + active_features
            .iter()
            .filter(|feature| {
                feature.owner_uid == uid
                    && buff_act::is_kind(feature, BuffActKind::FixAttrBySubBuffLayer)
            })
            .map(|feature| {
                buff_act::fix_attr_by_sub_buff_layer::attribute_delta(feature, attr_id, buffs)
            })
            .sum::<i32>()
        + active_features
            .iter()
            .filter(|feature| feature.owner_uid == uid)
            .map(|feature| buff_act::dynamic_attribute_delta(feature, attr_id, buffs, hp, true))
            .sum::<i32>()
        + buff_act::raspberry::attribute_delta(buffs, uid, attr_id)
}

pub(crate) fn snapshotted_attribute_delta(
    buffs: &BuffManager,
    hp: &HpManager,
    uid: i64,
    attr_id: crate::engine::entity::attr::AttrId,
) -> i32 {
    buffs
        .active_features(hp)
        .iter()
        .filter(|feature| feature.owner_uid == uid)
        .map(|feature| {
            crate::engine::skill::buff_act::injury_bank::snapshotted_attribute_delta(
                feature, attr_id, buffs,
            )
        })
        .sum()
}

pub(crate) struct HpExecution {
    pub changes: hp::HpChanges,
    pub before_hp_markers: Vec<buff::BuffActInfoMarkerResult>,
    pub indicator: Option<crate::engine::skill::rule::output::EffectMarker>,
}

struct HpCommit {
    changes: hp::HpChanges,
    before_hp_markers: Vec<buff::BuffActInfoMarkerResult>,
}

fn hp_loss(changes: &hp::HpChanges) -> Option<(i64, i32)> {
    if changes.kill.is_some() {
        return None;
    }
    let change = changes.hp.filter(|change| change.delta < 0)?;
    Some((change.target_uid, change.delta.saturating_abs()))
}

impl BattleManagers {
    pub(crate) fn try_catalog(&self) -> Option<crate::catalog::BattleCatalog> {
        self.catalog_data
    }

    pub(crate) fn catalog(&self) -> crate::catalog::BattleCatalog {
        if let Some(catalog) = self.try_catalog() {
            return catalog;
        }
        #[cfg(test)]
        return crate::catalog::BattleCatalog::new(crate::test_support::game_data());
        #[cfg(not(test))]
        panic!("battle managers were not constructed with a catalog")
    }

    pub(crate) fn fight_version(&self) -> i32 {
        self.fight_version
    }

    pub(crate) fn terminal_outcome(&self) -> Option<crate::engine::round::outcome::BattleOutcome> {
        self.terminal_outcome
    }

    pub(crate) fn commit_terminal(
        &mut self,
        outcome: crate::engine::round::outcome::BattleOutcome,
    ) -> bool {
        if self.terminal_outcome.is_some() {
            return false;
        }
        self.terminal_outcome = Some(outcome);
        true
    }
    pub(crate) fn origin_attribute(
        &self,
        uid: i64,
        attr_id: crate::engine::entity::attr::AttrId,
    ) -> i32 {
        use crate::engine::entity::attr::AttrId;

        let hp = self.hp.get(uid);
        match attr_id {
            AttrId::LostHp => (hp.max - hp.current).max(0),
            AttrId::CurrentHp => hp.current,
            AttrId::Hp => hp.max,
            AttrId::Attack | AttrId::RealityDef | AttrId::MentalDef | AttrId::CriticalTechnique => {
                let base = i64::from(self.attribute.base(uid, attr_id));
                let flat = i64::from(self.buff.fixed_attribute_delta(uid, attr_id))
                    + i64::from(snapshotted_attribute_delta(
                        &self.buff, &self.hp, uid, attr_id,
                    ));
                let delta = i64::from(
                    self.attribute.get(uid, attr_id)
                        + self.persistent_attribute_delta(uid, attr_id),
                );
                (base + flat + base * delta / 1000).clamp(0, i64::from(i32::MAX)) as i32
            }
            AttrId::CriticalDmg => {
                self.attribute.get(uid, attr_id)
                    + self.persistent_attribute_delta(uid, attr_id)
                    + self.buff.fixed_attribute_delta(uid, attr_id)
            }
            _ => self.attribute.get(uid, attr_id) + self.persistent_attribute_delta(uid, attr_id),
        }
    }

    pub(crate) fn persistent_attribute_delta(
        &self,
        uid: i64,
        attr_id: crate::engine::entity::attr::AttrId,
    ) -> i32 {
        persistent_attribute_delta(&self.buff, &self.hp, uid, attr_id)
    }

    /// Commits one buff transaction and returns its ordered semantic changes.
    pub(crate) fn execute_buff(
        &mut self,
        command: BuffCommand,
    ) -> Result<BuffChanges, BuffCommandError> {
        let plan = self.plan_buff(command)?;
        Ok(self.commit_buff(plan))
    }

    // A grant the target partly resists rolls its resistance like a crit.
    pub(crate) fn execute_buff_rolled(
        &mut self,
        command: BuffCommand,
        determinism: &mut crate::engine::runtime::determinism::RoundDeterminism,
    ) -> Result<BuffChanges, BuffCommandError> {
        let resist_roll = match &command {
            BuffCommand::Grant(grant)
            | BuffCommand::GrantOnDeath(grant)
            | BuffCommand::GrantRelated(buff::RelatedBuffGrant { grant, .. })
            | BuffCommand::GrantIndependent(grant)
            | BuffCommand::Accumulate(grant)
            | BuffCommand::GrantUsingChildUid(grant)
            | BuffCommand::GrantUsingNormalUid(grant) => {
                let chance = self.buff.buff_resistance(grant.target_uid, grant.buff_id);
                (chance > 0 && chance < 1000).then(|| buff::ResistRoll {
                    target_uid: grant.target_uid,
                    buff_id: grant.buff_id,
                    resisted: determinism.roll_crit(
                        grant.buff_id,
                        grant.source_uid,
                        grant.target_uid,
                        chance,
                    ),
                })
            }
            BuffCommand::GrantSourceRelative(stateful) => {
                let grant = &stateful.grant;
                let chance = self.buff.buff_resistance(grant.target_uid, grant.buff_id);
                (chance > 0 && chance < 1000).then(|| buff::ResistRoll {
                    target_uid: grant.target_uid,
                    buff_id: grant.buff_id,
                    resisted: determinism.roll_crit(
                        grant.buff_id,
                        grant.source_uid,
                        grant.target_uid,
                        chance,
                    ),
                })
            }
            _ => None,
        };
        let plan = self.plan_buff_with(command, resist_roll)?;
        Ok(self.commit_buff(plan))
    }

    pub(crate) fn plan_buff(&self, command: BuffCommand) -> Result<BuffPlan, BuffCommandError> {
        self.plan_buff_with(command, None)
    }

    fn plan_buff_with(
        &self,
        command: BuffCommand,
        resist_roll: Option<buff::ResistRoll>,
    ) -> Result<BuffPlan, BuffCommandError> {
        let source_uid = match &command {
            BuffCommand::Grant(grant)
            | BuffCommand::GrantOnDeath(grant)
            | BuffCommand::GrantRelated(buff::RelatedBuffGrant { grant, .. })
            | BuffCommand::GrantIndependent(grant)
            | BuffCommand::Accumulate(grant)
            | BuffCommand::GrantUsingChildUid(grant)
            | BuffCommand::GrantUsingNormalUid(grant) => Some(if grant.source_uid != 0 {
                grant.source_uid
            } else {
                grant.target_uid
            }),
            BuffCommand::GrantStateful(grant)
            | BuffCommand::GrantChild(grant)
            | BuffCommand::GrantInternalChild(grant) => Some(if grant.source_uid != 0 {
                grant.source_uid
            } else {
                grant.target_uid
            }),
            BuffCommand::GrantSourceRelative(stateful) => Some(if stateful.grant.source_uid != 0 {
                stateful.grant.source_uid
            } else {
                stateful.grant.target_uid
            }),
            BuffCommand::Convert(convert) => Some(convert.source_uid),
            BuffCommand::Replace(replace) => Some(if replace.source_uid != 0 {
                replace.source_uid
            } else {
                replace.target_uid
            }),
            BuffCommand::ReserveChildUids(reservation) => Some(reservation.target_uid),
            BuffCommand::ReserveGrantUid(reservation) => Some(reservation.target_uid),
            BuffCommand::ChangeDuration(change) => Some(change.target_uid),
            _ => None,
        };
        let source_attack = source_uid.map(|uid| self.attack(uid));
        let mut plan = self.buff.plan_with_source_attack(
            &self.hp,
            command,
            buff::GrantInputs {
                source_attack,
                resist_roll,
            },
        )?;
        let fanout_attacks = plan
            .fanout_source_uids()
            .into_iter()
            .map(|uid| (uid, self.attack(uid)))
            .collect();
        plan.initialize_fanout_source_attacks(&fanout_attacks);
        if let Some((source_uid, buff_id)) = plan.source_relative_attribute_source()
            && let Some(act_info) =
                crate::engine::skill::buff_act::each_change_attr_one_way::definition_snapshot(
                    self, buff_id, source_uid,
                )
        {
            plan.initialize_added_act_info_without_markers(act_info);
        }
        Ok(plan)
    }

    fn attack(&self, uid: i64) -> i32 {
        let attack = crate::engine::entity::attr::AttrId::Attack;
        let base = self.attribute.base(uid, attack);
        let active_features = self.buff.active_features(&self.hp);
        let dynamic = active_features
            .iter()
            .filter(|feature| feature.owner_uid == uid)
            .map(|feature| {
                crate::engine::skill::buff_act::attack_attribute_delta(
                    feature, attack, &self.buff, &self.hp,
                )
            })
            .sum::<i32>();
        let flat = snapshotted_attribute_delta(&self.buff, &self.hp, uid, attack);
        let rate = 1000
            + self.attribute.get(uid, attack)
            + self.buff.attribute_delta(uid, attack)
            + dynamic;
        base * rate.max(0) / 1000 + flat
    }

    pub(crate) fn commit_buff(&mut self, plan: BuffPlan) -> BuffChanges {
        let mut changes = self.buff.commit(&self.hp, plan);
        for target_uid in changes
            .change
            .removed
            .iter()
            .map(|removed| removed.target_uid)
            .collect::<std::collections::HashSet<_>>()
        {
            if !self.buff.has_status(target_uid, buff::BuffStatus::Shield) {
                let value = self.hp.shield(target_uid);
                if value > 0
                    && let Some(buff_uid) = changes.change.removed.iter().find_map(|removed| {
                        if removed.target_uid != target_uid {
                            return None;
                        }
                        let buff_id = removed.buff.buff_id?;
                        (self.catalog().buff_status(buff_id)? == buff::BuffStatus::Shield)
                            .then_some(removed.buff.uid)
                            .flatten()
                    })
                {
                    changes.shield_removed.push(buff::BuffShieldRemoveResult {
                        target_uid,
                        buff_uid,
                        value,
                    });
                }
                self.hp.set_shield(target_uid, 0);
            }
        }
        changes
    }

    pub(crate) fn execute_hp(
        &mut self,
        command: hp::HpCommand,
    ) -> Result<hp::HpChanges, hp::HpCommandError> {
        self.execute_hp_commit_with_target_count(command, 1)
            .map(|commit| commit.changes)
    }

    pub(crate) fn execute_rule_hp(
        &mut self,
        command: hp::HpCommand,
    ) -> Result<HpExecution, hp::HpCommandError> {
        let commit = self.execute_hp_commit_with_target_count(command, 1)?;
        let indicator = self
            .indicator
            .record_damage(commit.changes.target_uid, commit.changes.applied_damage());
        Ok(HpExecution {
            changes: commit.changes,
            before_hp_markers: commit.before_hp_markers,
            indicator,
        })
    }

    pub(crate) fn execute_hp_batch(
        &mut self,
        commands: Vec<hp::HpCommand>,
    ) -> Result<Vec<hp::HpChanges>, hp::HpCommandError> {
        self.execute_hp_batch_commits(commands)
            .map(|commits| commits.into_iter().map(|commit| commit.changes).collect())
    }

    fn execute_hp_batch_commits(
        &mut self,
        commands: Vec<hp::HpCommand>,
    ) -> Result<Vec<HpCommit>, hp::HpCommandError> {
        let mut targets_by_team =
            std::collections::HashMap::<i32, std::collections::HashSet<i64>>::new();
        for command in &commands {
            if let hp::HpCommand::Damage(damage) | hp::HpCommand::SettleAccumulatedDamage(damage) =
                command
                && damage.amount > 0
                && let Some(team) = self.entity.team_type(damage.target_uid)
            {
                targets_by_team
                    .entry(team)
                    .or_default()
                    .insert(damage.target_uid);
            }
        }
        let mut staged = self.clone();
        let mut shared_values = HashMap::new();
        let mut share_spent = HashMap::new();
        let mut plans = Vec::with_capacity(commands.len());
        for command in commands {
            let target_count = match command {
                hp::HpCommand::Damage(damage) | hp::HpCommand::SettleAccumulatedDamage(damage) => {
                    self.entity
                        .team_type(damage.target_uid)
                        .and_then(|team| targets_by_team.get(&team))
                        .map_or(1, std::collections::HashSet::len)
                }
                _ => 1,
            };
            let mut plan = staged.plan_hp(
                command,
                target_count,
                &staged.hp,
                &mut shared_values,
                &mut share_spent,
            )?;
            staged.prepare_hp_loss_state(&mut plan)?;
            staged.commit_hp_preview(&plan);
            plans.push(plan);
        }
        Ok(plans.into_iter().map(|plan| self.commit_hp(plan)).collect())
    }

    pub(crate) fn execute_rule_hp_batch(
        &mut self,
        commands: Vec<hp::HpCommand>,
    ) -> Result<Vec<HpExecution>, hp::HpCommandError> {
        let commits = self.execute_hp_batch_commits(commands)?;
        Ok(commits
            .into_iter()
            .map(|commit| {
                let indicator = self
                    .indicator
                    .record_damage(commit.changes.target_uid, commit.changes.applied_damage());
                HpExecution {
                    changes: commit.changes,
                    before_hp_markers: commit.before_hp_markers,
                    indicator,
                }
            })
            .collect())
    }

    fn execute_hp_commit_with_target_count(
        &mut self,
        command: hp::HpCommand,
        target_count: usize,
    ) -> Result<HpCommit, hp::HpCommandError> {
        let mut plan = self.plan_hp(
            command,
            target_count,
            &self.hp,
            &mut HashMap::new(),
            &mut HashMap::new(),
        )?;
        let mut staged = self.clone();
        staged.prepare_hp_loss_state(&mut plan)?;
        Ok(self.commit_hp(plan))
    }

    fn plan_hp(
        &self,
        mut command: hp::HpCommand,
        target_count: usize,
        hp: &HpManager,
        shared_values: &mut HashMap<i64, i32>,
        share_spent: &mut HashMap<i64, i32>,
    ) -> Result<HpPlan, hp::HpCommandError> {
        let share = if let hp::HpCommand::Damage(damage)
        | hp::HpCommand::SettleAccumulatedDamage(damage) = &mut command
            && let Some(split) = crate::engine::skill::buff_act::share_hurt::plan(
                &self.buff,
                hp,
                &self.entity,
                damage,
                share_spent,
            ) {
            damage.amount = split.holder_amount;
            *share_spent.entry(split.buff_uid).or_default() += 1;
            let shares = split
                .shares
                .into_iter()
                .map(hp::HpCommand::Lose)
                .collect::<Vec<_>>();
            for share in &shares {
                hp.validate_command(*share)?;
            }
            Some(ShareHurtPlan {
                consume: self
                    .plan_buff(split.consume)
                    .map_err(|_| hp::HpCommandError::InvalidShareHurtState)?,
                shares,
            })
        } else {
            None
        };
        if let hp::HpCommand::Damage(damage) | hp::HpCommand::SettleAccumulatedDamage(damage) =
            &mut command
            && let Some(cap) = self
                .buff
                .active_features(hp)
                .into_iter()
                .filter(|feature| feature.owner_uid == damage.target_uid)
                .filter_map(|feature| {
                    crate::engine::skill::buff_act::damage_not_more_than::cap(&feature, hp)
                })
                .min()
        {
            damage.amount = damage.amount.min(cap);
        }
        match &mut command {
            hp::HpCommand::Damage(damage) | hp::HpCommand::SettleAccumulatedDamage(damage) => {
                if let Some(cap) = crate::engine::skill::buff_act::lock_hp::max_loss(
                    &self.buff,
                    hp,
                    damage.target_uid,
                ) {
                    damage.amount = damage.amount.min(cap);
                }
            }
            hp::HpCommand::Lose(loss) => {
                if let Some(cap) = crate::engine::skill::buff_act::lock_hp::max_loss(
                    &self.buff,
                    hp,
                    loss.target_uid,
                ) {
                    loss.amount = loss.amount.min(cap);
                }
            }
            _ => {}
        }
        hp.validate_command(command)?;
        let fake_hp_consumed = match command {
            hp::HpCommand::Damage(damage) | hp::HpCommand::SettleAccumulatedDamage(damage)
                if damage.amount > 0 && damage.effect_kind != hp::DamageEffectKind::Avoided =>
            {
                crate::engine::skill::buff_act::lost_hp_to_fake_hp::damage_consumption(
                    self,
                    damage.target_uid,
                )
                .map_err(|_| hp::HpCommandError::InvalidHpLossState)?
            }
            _ => None,
        };
        let team_shared = match command {
            hp::HpCommand::Damage(damage) | hp::HpCommand::SettleAccumulatedDamage(damage)
                if damage.amount > 0 =>
            {
                self.team_shared_shield_plans(hp, damage.target_uid, target_count)
                    .into_iter()
                    .find_map(|mut plan| {
                        plan.current = *shared_values.entry(plan.buff_uid).or_insert(plan.current);
                        (plan.current > 0).then_some(plan)
                    })
            }
            _ => None,
        };
        let team_shared_buff = if let hp::HpCommand::Damage(damage)
        | hp::HpCommand::SettleAccumulatedDamage(damage) = command
            && let Some(absorption) = team_shared.and_then(|plan| plan.absorption(damage.amount))
        {
            let plan = self.plan_team_shared_shield_absorption(absorption, damage.origin)?;
            shared_values.insert(absorption.buff_uid, absorption.after);
            plan
        } else {
            None
        };
        Ok(HpPlan {
            command,
            fake_hp_consumed,
            team_shared,
            team_shared_buff,
            share,
            hp_loss_state: Vec::new(),
        })
    }

    fn prepare_hp_loss_state(&mut self, plan: &mut HpPlan) -> Result<(), hp::HpCommandError> {
        let hp_losses = self.preview_hp_losses(plan);
        for (target_uid, amount) in hp_losses {
            let updates =
                crate::engine::skill::buff_act::hp_loss_state_updates(self, target_uid, amount)
                    .ok_or(hp::HpCommandError::InvalidHpLossState)?;
            for update in updates {
                let buff = self
                    .plan_buff(update.command)
                    .map_err(|_| hp::HpCommandError::InvalidHpLossState)?;
                self.commit_buff(buff.clone());
                plan.hp_loss_state.push(HpLossStatePlan {
                    buff,
                    marker: update.marker,
                });
            }
        }
        Ok(())
    }

    fn commit_hp_preview(&mut self, plan: &HpPlan) {
        for share in plan.share.iter().flat_map(|share| &share.shares) {
            self.hp
                .commit_validated_command_with_team_shared(*share, None);
        }
        self.hp
            .commit_validated_command_with_team_shared(plan.command, plan.team_shared);
    }

    fn commit_hp(&mut self, plan: HpPlan) -> HpCommit {
        let before_hp_markers = plan
            .hp_loss_state
            .into_iter()
            .map(|state| {
                self.commit_buff(state.buff);
                state.marker
            })
            .collect();
        let shared_hurt = plan.share.map(|share| {
            let consumed = self.commit_buff(share.consume);
            let shares = share
                .shares
                .into_iter()
                .map(|share| {
                    self.hp
                        .commit_validated_command_with_team_shared(share, None)
                })
                .collect();
            Box::new(hp::SharedHurt { consumed, shares })
        });
        let toughness = match plan.command {
            hp::HpCommand::Damage(damage) | hp::HpCommand::SettleAccumulatedDamage(damage)
                if damage.effect_kind != hp::DamageEffectKind::Avoided
                    && damage.hurt.damage_from == hp::HurtDamageFromType::Skill =>
            {
                self.toughness.reduce(
                    damage.target_uid,
                    damage.amount,
                    damage.hurt.career_restraint || self.conduit.is_running(damage.source_uid),
                )
            }
            _ => None,
        };
        let team_shared_shield_removed = plan.team_shared_buff.map(|plan| self.commit_buff(plan));
        let mut changes = self
            .hp
            .commit_validated_command_with_team_shared(plan.command, plan.team_shared);
        changes.fake_hp_consumed = plan.fake_hp_consumed;
        changes.toughness = toughness;
        changes.team_shared_shield_removed = team_shared_shield_removed;
        changes.shared_hurt = shared_hurt;
        if let Some(shield) = &mut changes.shield_absorbed {
            shield.buff_uid = self
                .buff
                .shield_carrier_uid(changes.target_uid)
                .unwrap_or_default();
        }
        HpCommit {
            changes,
            before_hp_markers,
        }
    }

    fn preview_hp_losses(&self, plan: &HpPlan) -> Vec<(i64, i32)> {
        let mut hp = self.hp.clone();
        let mut losses = Vec::new();
        if let Some(share) = &plan.share {
            for command in &share.shares {
                let changes = hp.commit_validated_command_with_team_shared(*command, None);
                if let Some(loss) = hp_loss(&changes) {
                    losses.push(loss);
                }
            }
        }
        let changes = hp.commit_validated_command_with_team_shared(plan.command, plan.team_shared);
        if let Some(loss) = hp_loss(&changes) {
            losses.push(loss);
        }
        losses
    }

    fn team_shared_shield_plans(
        &self,
        hp: &HpManager,
        target_uid: i64,
        target_count: usize,
    ) -> Vec<hp::TeamSharedShieldPlan> {
        let Some(team) = self.entity.team_type(target_uid) else {
            return Vec::new();
        };
        let mut plans = self
            .buff
            .active_features(hp)
            .into_iter()
            .filter(|feature| {
                feature.team_type == team
                    && feature.owner_alive
                    && crate::engine::skill::buff_act::is_kind(
                        feature,
                        crate::engine::skill::buff_act::registry::BuffActKind::TeamShareShield,
                    )
            })
            .filter_map(|feature| {
                let block_rate = crate::engine::skill::buff_act::team_share_shield::block_rate(
                    feature.values.get(1..)?,
                    target_count,
                )?;
                let buff_act_id = feature.act_id()?;
                let current = self
                    .buff
                    .snapshot(feature.owner_uid, feature.buff_uid)?
                    .act_info
                    .iter()
                    .find(|info| info.act_id == Some(buff_act_id))?
                    .param
                    .first()
                    .copied()?;
                (current > 0).then_some(hp::TeamSharedShieldPlan {
                    buff_uid: feature.buff_uid,
                    owner_uid: feature.owner_uid,
                    buff_act_id,
                    current,
                    block_rate,
                })
            })
            .collect::<Vec<_>>();
        plans.sort_by_key(|plan| plan.buff_uid);
        plans
    }

    fn plan_team_shared_shield_absorption(
        &self,
        absorption: hp::TeamSharedShieldAbsorption,
        origin: crate::engine::skill::rule::CommandOrigin,
    ) -> Result<Option<BuffPlan>, hp::HpCommandError> {
        if absorption.after == 0 {
            return self
                .plan_buff(BuffCommand::Remove(BuffRemove {
                    origin,
                    target_uid: absorption.owner_uid,
                    selector: BuffRemoveSelector::Uid(absorption.buff_uid),
                }))
                .map(Some)
                .map_err(|_| hp::HpCommandError::InvalidTeamSharedState);
        }
        let mut buff = self
            .buff
            .snapshot(absorption.owner_uid, absorption.buff_uid)
            .ok_or(hp::HpCommandError::InvalidTeamSharedState)?;
        let info = buff
            .act_info
            .iter_mut()
            .find(|info| info.act_id == Some(absorption.buff_act_id))
            .ok_or(hp::HpCommandError::InvalidTeamSharedState)?;
        info.param = vec![absorption.after];
        info.str_param = Some(String::new());
        self.plan_buff(BuffCommand::SetInternalState(BuffSetState {
            origin,
            target_uid: absorption.owner_uid,
            buff_uid: absorption.buff_uid,
            ex_info: None,
            params: None,
            act_info: Some(buff.act_info),
        }))
        .map(Some)
        .map_err(|_| hp::HpCommandError::InvalidTeamSharedState)
    }

    pub(crate) fn execute_ex_point(
        &mut self,
        command: ex_point::ExPointCommand,
    ) -> Result<ex_point::ExPointChanges, ex_point::ExPointCommandError> {
        self.execute_ex_point_with(command, false)
    }

    pub(crate) fn execute_ex_point_rolled(
        &mut self,
        command: ex_point::ExPointCommand,
        determinism: &mut crate::engine::runtime::determinism::RoundDeterminism,
    ) -> Result<ex_point::ExPointChanges, ex_point::ExPointCommandError> {
        let reduction_allowed = match command {
            ex_point::ExPointCommand::Reduce(change) => !self.buff.has_buff_act_kind(
                change.target_uid,
                crate::engine::skill::buff_act::registry::BuffActKind::MoxieReductionImmunity,
            ),
            _ => true,
        };
        let resisted = match command {
            ex_point::ExPointCommand::Reduce(change) if reduction_allowed => {
                let resistance_id = crate::engine::entity::attr::AttrId::MoxieDownResistance;
                let base = self
                    .entity
                    .model_id(change.target_uid)
                    .and_then(|model_id| self.catalog().monster_resistances(model_id))
                    .map(|resistance| resistance.del_ex_point)
                    .unwrap_or_default();
                let chance = base.saturating_add(
                    self.persistent_attribute_delta(change.target_uid, resistance_id),
                );
                chance >= 1000
                    || (chance > 0
                        && determinism.roll_moxie_reduction_resistance(
                            change.origin.key.opcode,
                            change.source_uid,
                            change.target_uid,
                            chance,
                        ))
            }
            _ => false,
        };
        self.execute_ex_point_with(command, resisted)
    }

    fn execute_ex_point_with(
        &mut self,
        command: ex_point::ExPointCommand,
        resisted: bool,
    ) -> Result<ex_point::ExPointChanges, ex_point::ExPointCommandError> {
        if let ex_point::ExPointCommand::Reduce(change) = command
            && (change.target_uid == 0 || change.delta >= 0)
        {
            return Err(ex_point::ExPointCommandError::InvalidCommand);
        }
        if let ex_point::ExPointCommand::Reduce(change) = command
            && resisted
        {
            return Ok(ex_point::ExPointChanges::Resisted {
                target_uid: change.target_uid,
                resistance_id: crate::engine::entity::attr::AttrId::MoxieDownResistance.id(),
            });
        }
        let command = match command {
            ex_point::ExPointCommand::Change(mut change) if change.delta > 0 => {
                if let Some(source_uid) = self.buff.buff_act_source_uid(
                    change.target_uid,
                    crate::engine::skill::buff_act::registry::BuffActKind::TransferAddExPoint,
                ) {
                    change.target_uid = source_uid;
                }
                ex_point::ExPointCommand::Change(change)
            }
            ex_point::ExPointCommand::Reduce(change) => ex_point::ExPointCommand::Change(change),
            command => command,
        };
        let target_uid = match command {
            ex_point::ExPointCommand::Change(change) => Some(change.target_uid),
            ex_point::ExPointCommand::Reduce(change) => Some(change.target_uid),
            ex_point::ExPointCommand::Spend(change) => Some(change.target_uid),
            ex_point::ExPointCommand::Set(change) => Some(change.target_uid),
            ex_point::ExPointCommand::ChangeMax(_)
            | ex_point::ExPointCommand::ConfigureSynchronization(_)
            | ex_point::ExPointCommand::RestartSynchronization(_)
            | ex_point::ExPointCommand::CompleteSynchronization(_)
            | ex_point::ExPointCommand::RecordSynchronizationAction(_)
            | ex_point::ExPointCommand::RecordSynchronizationDamage(_)
            | ex_point::ExPointCommand::PublishSynchronizationHit(_) => None,
        };
        let gain_allowed = target_uid.is_none_or(|target_uid| {
            !self.buff.has_buff_act_kind(
                target_uid,
                crate::engine::skill::buff_act::registry::BuffActKind::ExPointCantAdd,
            )
        });
        let reduction_allowed = target_uid.is_none_or(|target_uid| {
            !self.buff.has_buff_act_kind(
                target_uid,
                crate::engine::skill::buff_act::registry::BuffActKind::MoxieReductionImmunity,
            )
        });
        self.ex_point
            .execute_command(command, gain_allowed, reduction_allowed)
    }

    pub(crate) fn execute_eureka(
        &mut self,
        command: EurekaCommand,
    ) -> Result<eureka::EurekaChanges, eureka::EurekaCommandError> {
        let EurekaCommand::ChangeByProgress {
            mut change,
            progress,
        } = command
        else {
            return self.eureka.execute_command(command);
        };
        let repeats = self.advance_rule_progress(
            progress.owner_uid,
            0,
            progress.key,
            progress.threshold,
            progress.amount,
        );
        if repeats == 0 {
            return Ok(eureka::EurekaChanges::Unchanged {
                origin: change.origin,
            });
        }
        change.delta = change.delta.saturating_mul(repeats);
        self.eureka.execute_command(EurekaCommand::Change(change))
    }

    pub(crate) fn execute_entity(
        &mut self,
        command: entity::EntityCommand,
    ) -> Result<entity::EntityChanges, entity::EntityCommandError> {
        let catalog = self.catalog();
        let combat_capacity = self.combat_capacity(command.source_uid);
        let mut changes = self.entity.execute_command_with_capacity(
            catalog,
            command,
            &self.hp,
            combat_capacity,
        )?;
        if matches!(
            changes.operation,
            entity::EntityOperation::SummonCombatant { .. }
                | entity::EntityOperation::SummonSpecial { .. }
                | entity::EntityOperation::Transform { .. }
        ) {
            changes.entity.ex_point_max = catalog.entity_ex_point_max(
                changes.entity.ex_point_max,
                changes.entity.model_id,
                changes.entity.level.unwrap_or(1),
            );
            self.entity.update(changes.entity.clone());
        }
        if matches!(changes.operation, entity::EntityOperation::Transform { .. }) {
            changes.entity.ex_point = Some(self.ex_point.get(changes.target_uid));
            changes.entity.shield_value = Some(self.hp.shield(changes.target_uid));
            self.eureka
                .sync_entity(changes.target_uid, &mut changes.entity);
            if !changes.operation.resets_state() {
                self.buff.sync_entity(&mut changes.entity);
            }
            self.entity.update(changes.entity.clone());
        }
        if !matches!(
            changes.operation,
            entity::EntityOperation::ChangeCareer { .. }
        ) {
            self.register_entity(&changes.entity);
        }
        if matches!(changes.operation, entity::EntityOperation::Transform { .. }) {
            self.project_primary_attributes(&mut changes.entity);
            self.entity.update(changes.entity.clone());
        }
        Ok(changes)
    }

    pub(crate) fn execute_entity_skill(
        &mut self,
        command: entity::EntitySkillCommand,
    ) -> Result<(), entity::EntityCommandError> {
        self.entity.execute_skill_command(command)
    }

    pub(crate) fn first_open_combat_position(&self, source_uid: i64) -> Option<i32> {
        self.entity.first_open_combat_position(
            source_uid,
            &self.hp,
            self.combat_capacity(source_uid),
        )
    }

    pub(crate) fn combat_position_available(&self, source_uid: i64, position: i32) -> bool {
        self.entity.combat_position_available(
            source_uid,
            position,
            &self.hp,
            self.combat_capacity(source_uid),
        )
    }

    fn combat_capacity(&self, source_uid: i64) -> i32 {
        match self.entity.team_type(source_uid) {
            Some(2) => match self.wave.combat_capacity() {
                Some(capacity) => capacity,
                None => entity::MAX_COMBAT_POSITION,
            },
            Some(1) => entity::MAX_COMBAT_POSITION,
            _ => 0,
        }
    }

    pub(crate) fn execute_gauge(
        &mut self,
        command: gauge::GaugeCommand,
    ) -> Result<gauge::GaugeChange, gauge::GaugeCommandError> {
        self.gauge.execute_command(command)
    }

    pub(crate) fn start_raw_gauge_batch(
        &mut self,
        id: gauge::GaugeBatchId,
        commands: Vec<gauge::GaugeCommand>,
    ) -> Result<gauge::GaugeChange, gauge::GaugeCommandError> {
        self.gauge.start_raw_batch(id, commands)
    }

    pub(crate) fn continue_raw_gauge_batch(
        &mut self,
        id: gauge::GaugeBatchId,
    ) -> Result<gauge::GaugeChange, gauge::GaugeCommandError> {
        self.gauge.continue_raw_batch(id)
    }

    pub(crate) fn commit_raw_gauge_batch(
        &mut self,
        id: gauge::GaugeBatchId,
    ) -> Result<gauge::GaugeBatchChanges, gauge::GaugeCommandError> {
        self.gauge.commit_raw_batch(id)
    }

    pub(crate) fn execute_emitter(
        &mut self,
        command: emitter::EmitterCommand,
    ) -> emitter::EmitterChange {
        let change = self.emitter.execute_command(command);
        if !change.enabled_before && change.enabled_after {
            self.hp.register(&emitter::activation_entity());
        }
        change
    }

    pub(crate) fn execute_card(
        &mut self,
        command: card::CardCommand,
    ) -> Result<card::CardChanges, card::CardCommandError> {
        let skill_group_replacement = match &command {
            card::CardCommand::ReplaceOwnerSkills(replace) => self
                .entity
                .snapshot(replace.owner_uid)
                .filter(|entity| {
                    (!replace.replacement_group1.is_empty()
                        || !replace.replacement_group2.is_empty())
                        && (replace.replacement_group1.is_empty()
                            || entity.skill_group1 == replace.base_group1)
                        && (replace.replacement_group2.is_empty()
                            || entity.skill_group2 == replace.base_group2)
                })
                .map(|mut entity| {
                    if !replace.replacement_group1.is_empty() {
                        entity.skill_group1 = replace.replacement_group1.clone();
                    }
                    if !replace.replacement_group2.is_empty() {
                        entity.skill_group2 = replace.replacement_group2.clone();
                    }
                    entity
                }),
            _ => None,
        };
        let mut changes = self.card.execute_command(command)?;
        if let Some(entity) = skill_group_replacement {
            let owner_uid = entity.uid.unwrap_or_default();
            self.entity.update(entity);
            changes.entity = self.entity_snapshot(owner_uid);
        }
        if changes.kind == card::CardChangeKind::HandRankChanged
            && let Some(owner_uid) = changes.rank_results.iter().find_map(|result| match result {
                card::CardRankResult::Changed(change) => Some(change.owner_uid),
                card::CardRankResult::Failed(_) => None,
            })
        {
            changes.entity = self.entity_snapshot(owner_uid);
        }
        Ok(changes)
    }

    pub(crate) fn execute_field(
        &mut self,
        command: field::FieldCommand,
    ) -> Result<field::FieldChange, field::FieldCommandError> {
        self.field.execute_command(command)
    }

    pub(crate) fn execute_summon(
        &mut self,
        command: summon::SummonCommand,
    ) -> Result<summon::SummonChanges, summon::SummonCommandError> {
        self.summon.execute_command(command)
    }

    pub(crate) fn execute_upgrade(
        &mut self,
        command: upgrade::UpgradeCommand,
    ) -> Result<upgrade::UpgradeChange, upgrade::UpgradeCommandError> {
        self.upgrade.execute_command(self.catalog(), command)
    }

    pub fn select_upgrade(
        &mut self,
        fight: &mut Fight,
        owner_uid: i64,
        upgrade_id: i32,
        option_id: i32,
    ) -> Option<upgrade::UpgradeApplied> {
        let mut managers = self.clone();
        let applied = managers.apply_upgrade(owner_uid, upgrade_id, option_id)?;
        *self = managers;
        self.sync_entities(fight);
        Some(applied)
    }

    fn apply_upgrade(
        &mut self,
        owner_uid: i64,
        upgrade_id: i32,
        option_id: i32,
    ) -> Option<upgrade::UpgradeApplied> {
        let selection_change = self
            .execute_upgrade(upgrade::UpgradeCommand {
                owner_uid,
                operation: upgrade::UpgradeOperation::Select {
                    upgrade_id,
                    option_id,
                },
            })
            .ok()?;
        let origin = selection_change.offer_origin?;
        let selection = selection_change.selection.clone()?;
        let (entity, buff_changes, card_changes) =
            self.apply_upgrade_selection(owner_uid, origin, selection)?;
        Some(upgrade::UpgradeApplied {
            change: selection_change,
            entity,
            buff_changes,
            card_changes,
        })
    }

    fn apply_upgrade_selection(
        &mut self,
        owner_uid: i64,
        origin: crate::engine::skill::rule::CommandOrigin,
        selection: upgrade::UpgradeSelection,
    ) -> Option<(FightEntityInfo, Vec<BuffChanges>, card::CardChanges)> {
        let card_changes = self.apply_upgrade_identity(owner_uid, origin, &selection)?;
        let mut buff_changes =
            Vec::with_capacity(selection.del_buff_ids.len() + selection.add_buff_ids.len());
        for buff_id in selection.del_buff_ids {
            buff_changes.push(
                self.execute_buff(BuffCommand::Remove(BuffRemove {
                    origin,
                    target_uid: owner_uid,
                    selector: BuffRemoveSelector::ExactId(buff_id),
                }))
                .ok()?,
            );
        }
        for buff_id in selection.add_buff_ids {
            buff_changes.push(
                self.execute_buff(BuffCommand::GrantUsingChildUid(BuffGrant {
                    origin,
                    source_uid: owner_uid,
                    target_uid: owner_uid,
                    buff_id,
                    amount: None,
                    occurrences: 1,
                    child_uid_reservations: 0,
                }))
                .ok()?,
            );
        }
        Some((self.entity_snapshot(owner_uid)?, buff_changes, card_changes))
    }

    fn apply_upgrade_identity(
        &mut self,
        owner_uid: i64,
        origin: crate::engine::skill::rule::CommandOrigin,
        selection: &upgrade::UpgradeSelection,
    ) -> Option<card::CardChanges> {
        let mut entity = self.entity.snapshot(owner_uid)?;
        let base_group1 = entity.skill_group1.clone();
        let base_group2 = entity.skill_group2.clone();
        let enhance = entity
            .enhance_info_box
            .get_or_insert_with(|| EnhanceInfoBox {
                uid: Some(owner_uid),
                ..Default::default()
            });
        enhance
            .can_upgrade_ids
            .retain(|id| *id != selection.upgrade_id);
        if !enhance.upgraded_options.contains(&selection.option_id) {
            enhance.upgraded_options.push(selection.option_id);
        }
        for (from, to) in &selection.replace_passive_skills {
            if let Some(index) = entity.passive_skill.iter().position(|id| id == from) {
                entity.passive_skill[index] = *to;
            } else if !entity.passive_skill.contains(to) {
                entity.passive_skill.push(*to);
            }
        }
        for skill_id in &selection.add_passive_skill_ids {
            if !entity.passive_skill.contains(skill_id) {
                entity.passive_skill.push(*skill_id);
            }
        }
        if selection.replace_big_skill > 0 {
            entity.ex_skill = Some(selection.replace_big_skill);
        }
        if !selection.replace_skill_group1.is_empty() {
            entity.skill_group1 = selection.replace_skill_group1.clone();
        }
        if !selection.replace_skill_group2.is_empty() {
            entity.skill_group2 = selection.replace_skill_group2.clone();
        }
        self.entity.update(entity);

        self.execute_card(card::CardCommand::ReplaceOwnerSkills(
            card::CardReplaceOwnerSkills {
                origin,
                owner_uid,
                base_group1,
                base_group2,
                replacement_group1: selection.replace_skill_group1.clone(),
                replacement_group2: selection.replace_skill_group2.clone(),
            },
        ))
        .ok()
    }

    pub fn register_entity(&mut self, entity: &FightEntityInfo) {
        let catalog = self.catalog();
        self.entity.register(catalog, entity);
        self.register_entity_state(entity);
    }

    fn register_entity_state(&mut self, entity: &FightEntityInfo) {
        let catalog = self.catalog();
        let team_type = entity.team_type.unwrap_or_default();
        self.attribute.register_with_catalog(catalog, entity);
        self.hp.register(entity);
        self.toughness.register_configured(catalog, entity);
        self.ex_point.register_configured(catalog, entity);
        self.eureka.register(entity);
        self.buff.register_entity(entity, team_type);
        let entity_order = self.entity.ordered_uids().collect::<Vec<_>>();
        self.buff.sync_entity_order(&entity_order);
    }

    pub(crate) fn promote_reserves(
        &mut self,
        fight: &mut Fight,
    ) -> Vec<crate::engine::fight::reserve::Promotion> {
        let mut promotions = self.entity.promote_reserves(&self.hp);
        self.complete_promotions(&mut promotions);
        self.sync_entities(fight);
        promotions
    }

    pub(crate) fn promote_immediate_reserves(
        &mut self,
    ) -> Vec<crate::engine::fight::reserve::Promotion> {
        let battle_rule = &self.battle_rule;
        let mut promotions = self
            .entity
            .promote_immediate_reserves(&self.hp, |team_type| {
                battle_rule.promotes_reserves_immediately(team_type)
            });
        self.complete_promotions(&mut promotions);
        promotions
    }

    fn complete_promotions(&self, promotions: &mut [crate::engine::fight::reserve::Promotion]) {
        for promotion in promotions {
            if let Some(entering) = self.entity_snapshot(promotion.entering_uid) {
                promotion.entering = entering;
            }
        }
    }

    pub(crate) fn advance_wave(
        &mut self,
        fight: &mut Fight,
    ) -> anyhow::Result<Option<wave::WaveAdvanced>> {
        self.sync_entities(fight);
        let catalog = self.catalog();
        let Some(roster) = self.wave.advance(catalog)? else {
            return Ok(None);
        };
        self.entity
            .replace_team_roster(catalog, 2, &roster.entitys, &roster.sub_entitys);
        for entity in roster.entitys.iter().chain(&roster.sub_entitys) {
            self.register_entity_state(entity);
        }
        fight.cur_wave = Some(roster.wave);
        fight.is_record = Some(false);
        self.sync_entities(fight);
        Ok(Some(wave::WaveAdvanced {
            wave: roster.wave,
            entering_uids: roster.entering_uids,
            fight: fight.clone(),
        }))
    }

    pub(crate) fn can_fire_rule(
        &self,
        owner_uid: i64,
        skill_id: i32,
        limit_owner: crate::engine::skill::effect::RuleLimitOwner,
        limit: i32,
        round_limit: i32,
    ) -> bool {
        let key = (owner_uid, skill_id, limit_owner);
        (limit <= 0 || self.rule_fires.get(&key).copied().unwrap_or_default() < limit)
            && (round_limit <= 0
                || self.round_rule_fires.get(&key).copied().unwrap_or_default() < round_limit)
    }

    pub(crate) fn mark_rule_fired(
        &mut self,
        owner_uid: i64,
        skill_id: i32,
        limit_owner: crate::engine::skill::effect::RuleLimitOwner,
    ) {
        let key = (owner_uid, skill_id, limit_owner);
        *self.rule_fires.entry(key).or_default() += 1;
        *self.round_rule_fires.entry(key).or_default() += 1;
    }

    pub fn can_fire_buff_act(
        &self,
        owner_uid: i64,
        buff_uid: i64,
        key: crate::engine::skill::rule::DefinitionKey,
        limit: i32,
    ) -> bool {
        self.buff_act_fires
            .get(&(owner_uid, buff_uid, key))
            .copied()
            .unwrap_or_default()
            < limit
    }

    pub fn mark_buff_act_fired(
        &mut self,
        owner_uid: i64,
        buff_uid: i64,
        key: crate::engine::skill::rule::DefinitionKey,
    ) {
        *self
            .buff_act_fires
            .entry((owner_uid, buff_uid, key))
            .or_default() += 1;
    }

    pub fn begin_round(&mut self) {
        self.begin_reaction_round();
        self.begin_action_round();
    }

    pub(crate) fn begin_reaction_round(&mut self) {
        self.round_rule_fires.clear();
        self.buff_act_fires.clear();
        self.configured_rule_progress.begin_round_start();
    }

    pub(crate) fn begin_action_round(&mut self) {
        self.configured_rule_progress.begin_action_round();
        self.card.begin_round();
        self.hp.begin_round();
        self.injury.begin_round();
        self.gauge.begin_combat_round();
        self.field.begin_round();
        self.conduit.begin_round();
    }

    pub fn advance_configured_rule_progress_until_cast(
        &mut self,
        owner_uid: i64,
        key: crate::engine::skill::rule::ConfiguredRuleKey,
        threshold: i32,
        delta: i32,
    ) -> bool {
        self.configured_rule_progress
            .advance_until_cast(owner_uid, key, threshold, delta)
    }

    pub fn release_held_rule_progress(
        &mut self,
        owner_uid: i64,
        key: crate::engine::skill::rule::ConfiguredRuleKey,
    ) {
        self.configured_rule_progress.release(owner_uid, key);
    }

    pub fn advance_rule_progress(
        &mut self,
        owner_uid: i64,
        instance_uid: i64,
        key: crate::engine::skill::rule::DefinitionKey,
        threshold: i32,
        delta: i32,
    ) -> i32 {
        if threshold <= 0 || delta <= 0 {
            return 0;
        }
        let progress = self
            .rule_progress
            .entry((owner_uid, instance_uid, key, threshold))
            .or_default();
        let before = *progress;
        *progress = progress.saturating_add(delta);
        if *progress < threshold {
            return 0;
        }
        let repeats = *progress / threshold;
        *progress %= threshold;
        if crate::engine::diagnostics::enabled(crate::engine::diagnostics::TraceArea::Gauge) {
            eprintln!(
                "rule progress owner={owner_uid} key={key:?} delta={delta} threshold={threshold} progress={before}->{} repeats={repeats}",
                *progress,
            );
        }
        repeats
    }

    /// Seeds every manager from the initial fight snapshot exactly once.
    pub fn seeded_with_catalog(catalog: crate::catalog::BattleCatalog, fight: &Fight) -> Self {
        let mut managers = Self {
            catalog_data: Some(catalog),
            fight_version: fight.version.unwrap_or_default(),
            ..Self::default()
        };
        managers.attribute.seed_with_catalog(catalog, fight);
        managers.battle_rule = battle_rule::BattleRuleManager::seed_with_catalog(catalog, fight);
        managers.hp.seed(fight);
        managers.toughness.seed_configured(catalog, fight);
        managers.ex_point.seed_configured(catalog, fight);
        managers.eureka.seed(fight);
        managers.buff.set_catalog(catalog);
        managers.buff.seed(fight);
        managers.card.set_catalog(catalog);
        managers.card.seed(fight);
        managers.conduit = ConduitManager::configured(catalog, fight);
        managers.entity = entity::EntityManager::configured(catalog, fight);
        managers.fight_progress.seed(fight);
        managers.wave = wave::WaveManager::seed_with_catalog(catalog, fight);
        managers
    }

    #[cfg(test)]
    pub fn seeded(fight: &Fight) -> Self {
        Self::seeded_with_catalog(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            fight,
        )
    }

    /// Projects manager-owned entity state into the response `Fight` snapshot.
    ///
    /// This is a serialization boundary, not a second mutation path.
    pub fn sync_entities(&self, fight: &mut Fight) {
        self.entity.sync_to_fight(fight);
        for entity in entities_mut(fight) {
            self.project_entity_state(entity);
        }
        for entity in fight
            .attacker
            .iter_mut()
            .chain(fight.defender.iter_mut())
            .filter_map(|team| team.assist_boss.as_mut())
        {
            self.project_entity_state(entity);
        }
        self.eureka.sync_fight(fight);
        self.fight_progress.sync_fight(fight);
        let features = self.buff.active_features(&self.hp);
        if let Some(attacker) = fight.attacker.as_mut() {
            attacker.heat_scale = self.heat_scale_snapshot(1, &features);
        }
        if let Some(defender) = fight.defender.as_mut() {
            defender.heat_scale = self.heat_scale_snapshot(2, &features);
        }
    }

    fn heat_scale_snapshot(
        &self,
        team_type: i32,
        features: &[buff::ActiveBuffFeature],
    ) -> Option<sonettobuf::FightHeatScale> {
        let state = self
            .gauge
            .get(crate::engine::mechanic::lingering_glow::key(team_type))?;
        let crystal = features
            .iter()
            .find(|feature| {
                feature.team_type == team_type
                    && crate::engine::skill::buff_act::is_kind(
                        feature,
                        crate::engine::skill::buff_act::registry::BuffActKind::CrystalNotifySelect,
                    )
            })
            .map(|feature| self.emanation.counts(feature.owner_uid))
            .map(|[blue, purple, green]| blue * 100 + purple * 10 + green)
            .unwrap_or_default();
        Some(sonettobuf::FightHeatScale {
            value: Some(i64::from(state.current)),
            max: Some(i64::from(state.max.unwrap_or_default())),
            crystal: Some(crystal),
        })
    }

    pub(crate) fn entity_snapshot(&self, uid: i64) -> Option<FightEntityInfo> {
        let mut entity = self.entity.snapshot(uid)?;
        self.project_entity_state(&mut entity);
        self.eureka.sync_entity(uid, &mut entity);
        Some(entity)
    }

    fn project_entity_state(&self, entity: &mut FightEntityInfo) {
        let Some(uid) = entity.uid else { return };
        self.buff.sync_entity(entity);
        for link in self.buff.passive_skill_links_for(uid) {
            if !entity.passive_skill.contains(&link.skill_id) {
                entity.passive_skill.push(link.skill_id);
            }
        }
        self.project_primary_attributes(entity);
        self.hp.sync_entity(entity);
        self.toughness.sync_entity(entity);
        self.ex_point.sync_entity_configured(self.catalog(), entity);
        entity.ex_skill_point_change =
            Some(crate::engine::mechanic::card::CardMechanic.ultimate_cost_offset(self, uid));
    }

    fn project_primary_attributes(&self, entity: &mut FightEntityInfo) {
        let (Some(uid), Some(attr)) = (entity.uid, entity.attr.as_mut()) else {
            return;
        };
        attr.attack = Some(self.origin_attribute(uid, crate::engine::entity::attr::AttrId::Attack));
        attr.defense =
            Some(self.origin_attribute(uid, crate::engine::entity::attr::AttrId::RealityDef));
        attr.mdefense =
            Some(self.origin_attribute(uid, crate::engine::entity::attr::AttrId::MentalDef));
        attr.technic = Some(
            self.origin_attribute(uid, crate::engine::entity::attr::AttrId::CriticalTechnique),
        );
    }

    pub(crate) fn sync_roster(&mut self, fight: &Fight) {
        self.buff.sync_roster(fight);
        let entity_order = self.entity.ordered_uids().collect::<Vec<_>>();
        self.buff.sync_entity_order(&entity_order);
    }

    pub fn ex_point_info(&self, _fight: &Fight) -> Vec<FightExPointInfo> {
        self.entity
            .ordered_uids()
            .filter(|uid| self.entity.roster_lane(*uid) != Some(entity::RosterLane::Inactive))
            .map(|uid| FightExPointInfo {
                uid: Some(uid),
                ex_point: Some(self.ex_point.get(uid)),
                power_infos: self.eureka.power_infos(uid),
                current_hp: Some(self.hp.current(uid)),
                ex_point_type: Some(self.ex_point.kind(uid)),
            })
            .collect()
    }

    pub(crate) fn hero_sp_attributes(&self, fight: &Fight) -> Vec<FightHeroSpAttributeInfo> {
        let fight_version = fight.version.unwrap_or_default();
        entities(fight)
            .filter(|entity| entity.team_type == Some(2))
            .filter(|entity| entity.uid.is_some_and(|uid| self.hp.current(uid) > 0))
            .filter_map(|entity| {
                let uid = entity.uid?;
                let mut attribute = monster_sp_attribute(
                    self.catalog().monster_resistances(entity.model_id?),
                    fight_version,
                );
                use crate::engine::entity::attr::AttrId;
                attribute.revive = Some(self.origin_attribute(uid, AttrId::DmgHeal));
                attribute.heal = Some(self.origin_attribute(uid, AttrId::HealingDone));
                attribute.absorb = Some(self.origin_attribute(uid, AttrId::LeechRate));
                attribute.defense_ignore = Some(self.origin_attribute(uid, AttrId::Penetration));
                attribute.clutch = Some(self.origin_attribute(uid, AttrId::UltimateMight));
                attribute.final_add_dmg = Some(self.origin_attribute(uid, AttrId::FinalDmgBonus));
                attribute.final_drop_dmg =
                    Some(self.origin_attribute(uid, AttrId::FinalDmgReduction));
                attribute.normal_skill_rate =
                    Some(self.origin_attribute(uid, AttrId::IncantationMight));
                attribute.play_add_rate =
                    Some(self.origin_attribute(uid, AttrId::PlaymodeDmgIncrease));
                attribute.play_drop_rate =
                    Some(self.origin_attribute(uid, AttrId::PlaymodeDmgImmunity));
                attribute.rebound_dmg = Some(self.origin_attribute(uid, AttrId::ReboundDmg));
                attribute.extra_dmg = Some(self.origin_attribute(uid, AttrId::ExtraDmg));
                attribute.reuse_dmg = Some(self.origin_attribute(uid, AttrId::ReuseDmg));
                attribute.device_skill_rate =
                    (fight_version == 7).then(|| self.origin_attribute(uid, AttrId::ConduitMight));
                Some(FightHeroSpAttributeInfo {
                    uid: Some(uid),
                    attribute: Some(attribute),
                })
            })
            .collect()
    }

    pub fn faith_ex_point_uids(&self, team_type: i32) -> Vec<i64> {
        self.buff
            .alive_team_uids(team_type, &self.hp)
            .into_iter()
            .filter(|uid| self.ex_point.kind(*uid) == ExPointKind::Faith.as_wire())
            .collect()
    }
}

fn monster_sp_attribute(
    resistance: Option<MonsterResistances>,
    fight_version: i32,
) -> HeroSpAttribute {
    let Some(resistance) = resistance else {
        return base_hero_sp_attribute(fight_version);
    };

    HeroSpAttribute {
        dizzy_resistances: Some(resistance.dizzy),
        sleep_resistances: Some(resistance.sleep),
        petrified_resistances: Some(resistance.petrified),
        frozen_resistances: Some(resistance.frozen),
        disarm_resistances: Some(resistance.disarm),
        forbid_resistances: Some(resistance.forbid),
        seal_resistances: Some(resistance.seal),
        cant_get_exskill_resistances: Some(resistance.cant_get_exskill),
        del_ex_point_resistances: Some(resistance.del_ex_point),
        stress_up_resistances: Some(resistance.stress_up),
        control_resilience: Some(resistance.control_resilience),
        del_ex_point_resilience: Some(resistance.del_ex_point_resilience),
        stress_up_resilience: Some(resistance.stress_up_resilience),
        charm_resistances: Some(resistance.charm),
        ..base_hero_sp_attribute(fight_version)
    }
}

fn base_hero_sp_attribute(fight_version: i32) -> HeroSpAttribute {
    let mut attribute = HeroSpAttribute {
        revive: Some(0),
        heal: Some(0),
        absorb: Some(0),
        defense_ignore: Some(0),
        clutch: Some(0),
        final_add_dmg: Some(0),
        final_drop_dmg: Some(0),
        normal_skill_rate: Some(0),
        play_add_rate: Some(0),
        play_drop_rate: Some(0),
        dizzy_resistances: Some(0),
        sleep_resistances: Some(0),
        petrified_resistances: Some(0),
        frozen_resistances: Some(0),
        disarm_resistances: Some(0),
        forbid_resistances: Some(0),
        seal_resistances: Some(0),
        cant_get_exskill_resistances: Some(0),
        del_ex_point_resistances: Some(0),
        stress_up_resistances: Some(0),
        control_resilience: Some(0),
        del_ex_point_resilience: Some(0),
        stress_up_resilience: Some(0),
        charm_resistances: Some(0),
        rebound_dmg: Some(0),
        extra_dmg: Some(0),
        reuse_dmg: Some(0),
        big_skill_rate: Some(0),
        clutch_dmg: Some(0),
        nowmal_dmg: Some(0),
        ..Default::default()
    };
    if fight_version == 7 {
        attribute.toughness_add = Some(0);
        attribute.toughness_drop = Some(0);
        attribute.multi_weak_dmg_add = Some(0);
        attribute.multi_weak_dmg_drop = Some(0);
        attribute.play_add_rate2 = Some(0);
        attribute.play_drop_rate2 = Some(0);
        attribute.device_skill_rate = Some(0);
    }
    attribute
}

pub fn entities(fight: &Fight) -> impl Iterator<Item = &FightEntityInfo> {
    fight
        .attacker
        .iter()
        .chain(fight.defender.iter())
        .flat_map(|team| {
            team.entitys
                .iter()
                .chain(&team.sub_entitys)
                .chain(&team.sp_entitys)
        })
}

pub fn entities_mut(fight: &mut Fight) -> impl Iterator<Item = &mut FightEntityInfo> {
    fight
        .attacker
        .iter_mut()
        .chain(fight.defender.iter_mut())
        .flat_map(|team| {
            team.entitys
                .iter_mut()
                .chain(&mut team.sub_entitys)
                .chain(&mut team.sp_entitys)
        })
}

#[cfg(test)]
mod test;
