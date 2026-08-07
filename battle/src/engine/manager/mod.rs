pub mod attribute;
pub mod battle_rule;
pub mod buff;
pub mod card;
pub mod conduit;
pub mod contract;
pub mod emanation;
pub mod emitter;
pub mod entity;
pub mod eureka;
pub mod ex_point;
pub mod field;
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
    pub upgrade: UpgradeManager,
    pub summon: SummonManager,
    pub toughness: toughness::ToughnessManager,
    pub wave: wave::WaveManager,
    rule_fires: HashMap<(i64, i32, usize, crate::engine::skill::rule::DefinitionKey), i32>,
    round_rule_fires: HashMap<(i64, i32, usize, crate::engine::skill::rule::DefinitionKey), i32>,
    buff_act_fires: HashMap<(i64, i64, crate::engine::skill::rule::DefinitionKey), i32>,
    rule_progress: HashMap<(i64, i64, crate::engine::skill::rule::DefinitionKey, i32), i32>,
}

struct HpPlan {
    command: hp::HpCommand,
    team_shared: Option<hp::TeamSharedShieldPlan>,
    team_shared_buff: Option<BuffPlan>,
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
            .filter(|feature| {
                feature.owner_uid == uid
                    && buff_act::is_kind(feature, BuffActKind::AddAttrByOtherBuffLayer)
            })
            .map(|feature| {
                buff_act::add_attr_by_other_buff_layer::attribute_delta(feature, attr_id, buffs)
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

pub(crate) struct HpExecution {
    pub changes: hp::HpChanges,
    pub indicator: Option<crate::engine::skill::rule::output::EffectMarker>,
}

impl BattleManagers {
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
                let delta = i64::from(
                    self.attribute.get(uid, attr_id)
                        + self.persistent_attribute_delta(uid, attr_id),
                );
                (base + base * delta / 1000).clamp(0, i64::from(i32::MAX)) as i32
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

    pub(crate) fn plan_buff(&self, command: BuffCommand) -> Result<BuffPlan, BuffCommandError> {
        let source_uid = match &command {
            BuffCommand::Grant(grant)
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
        let source_attack = source_uid.map(|uid| {
            let base = self
                .attribute
                .base(uid, crate::engine::entity::attr::AttrId::Attack);
            let active_features = self.buff.active_features(&self.hp);
            let dynamic = active_features
                .iter()
                .filter(|feature| feature.owner_uid == uid)
                .map(|feature| {
                    crate::engine::skill::buff_act::attack_attribute_delta(
                        feature,
                        crate::engine::entity::attr::AttrId::Attack,
                        &self.buff,
                        &self.hp,
                    )
                })
                .sum::<i32>();
            let flat = active_features
                .iter()
                .filter(|feature| feature.owner_uid == uid)
                .map(|feature| {
                    crate::engine::skill::buff_act::injury_bank::snapshotted_attribute_delta(
                        feature,
                        crate::engine::entity::attr::AttrId::Attack,
                        &self.buff,
                    )
                })
                .sum::<i32>();
            let rate = 1000
                + self
                    .attribute
                    .get(uid, crate::engine::entity::attr::AttrId::Attack)
                + self
                    .buff
                    .attribute_delta(uid, crate::engine::entity::attr::AttrId::Attack)
                + dynamic;
            base * rate.max(0) / 1000 + flat
        });
        self.buff
            .plan_with_source_attack(&self.hp, command, source_attack)
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
                        (buff::configured_status(buff_id)? == buff::BuffStatus::Shield)
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
        self.execute_hp_with_target_count(command, 1)
    }

    pub(crate) fn execute_rule_hp(
        &mut self,
        command: hp::HpCommand,
    ) -> Result<HpExecution, hp::HpCommandError> {
        let changes = self.execute_hp(command)?;
        let indicator = self
            .indicator
            .record_damage(changes.target_uid, changes.applied_damage());
        Ok(HpExecution { changes, indicator })
    }

    pub(crate) fn execute_hp_batch(
        &mut self,
        commands: Vec<hp::HpCommand>,
    ) -> Result<Vec<hp::HpChanges>, hp::HpCommandError> {
        let mut targets_by_team =
            std::collections::HashMap::<i32, std::collections::HashSet<i64>>::new();
        for command in &commands {
            if let hp::HpCommand::Damage(damage) = command
                && damage.amount > 0
                && let Some(team) = self.entity.team_type(damage.target_uid)
            {
                targets_by_team
                    .entry(team)
                    .or_default()
                    .insert(damage.target_uid);
            }
        }
        let mut staged_hp = self.hp.clone();
        let mut shared_values = HashMap::new();
        let mut plans = Vec::with_capacity(commands.len());
        for command in commands {
            let target_count = match command {
                hp::HpCommand::Damage(damage) => self
                    .entity
                    .team_type(damage.target_uid)
                    .and_then(|team| targets_by_team.get(&team))
                    .map_or(1, std::collections::HashSet::len),
                _ => 1,
            };
            let plan = self.plan_hp(command, target_count, &staged_hp, &mut shared_values)?;
            staged_hp.commit_validated_command_with_team_shared(plan.command, plan.team_shared);
            plans.push(plan);
        }
        Ok(plans.into_iter().map(|plan| self.commit_hp(plan)).collect())
    }

    pub(crate) fn execute_rule_hp_batch(
        &mut self,
        commands: Vec<hp::HpCommand>,
    ) -> Result<Vec<HpExecution>, hp::HpCommandError> {
        let changes = self.execute_hp_batch(commands)?;
        Ok(changes
            .into_iter()
            .map(|change| {
                let indicator = self
                    .indicator
                    .record_damage(change.target_uid, change.applied_damage());
                HpExecution {
                    changes: change,
                    indicator,
                }
            })
            .collect())
    }

    fn execute_hp_with_target_count(
        &mut self,
        command: hp::HpCommand,
        target_count: usize,
    ) -> Result<hp::HpChanges, hp::HpCommandError> {
        let plan = self.plan_hp(command, target_count, &self.hp, &mut HashMap::new())?;
        Ok(self.commit_hp(plan))
    }

    fn plan_hp(
        &self,
        mut command: hp::HpCommand,
        target_count: usize,
        hp: &HpManager,
        shared_values: &mut HashMap<i64, i32>,
    ) -> Result<HpPlan, hp::HpCommandError> {
        if let hp::HpCommand::Damage(damage) = &mut command
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
        hp.validate_command(command)?;
        let team_shared = match command {
            hp::HpCommand::Damage(damage) if damage.amount > 0 => self
                .team_shared_shield_plans(hp, damage.target_uid, target_count)
                .into_iter()
                .find_map(|mut plan| {
                    plan.current = *shared_values.entry(plan.buff_uid).or_insert(plan.current);
                    (plan.current > 0).then_some(plan)
                }),
            _ => None,
        };
        let team_shared_buff = if let hp::HpCommand::Damage(damage) = command
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
            team_shared,
            team_shared_buff,
        })
    }

    fn commit_hp(&mut self, plan: HpPlan) -> hp::HpChanges {
        let toughness = match plan.command {
            hp::HpCommand::Damage(damage)
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
        changes.toughness = toughness;
        changes.team_shared_shield_removed = team_shared_shield_removed;
        if let Some(shield) = &mut changes.shield_absorbed {
            shield.buff_uid = self
                .buff
                .shield_carrier_uid(changes.target_uid)
                .unwrap_or_default();
        }
        changes
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
        let target_uid = match command {
            ex_point::ExPointCommand::Change(change) => Some(change.target_uid),
            ex_point::ExPointCommand::Spend(change) => Some(change.target_uid),
            ex_point::ExPointCommand::Set(change) => Some(change.target_uid),
            ex_point::ExPointCommand::ChangeMax(_)
            | ex_point::ExPointCommand::ConfigureSynchronization(_)
            | ex_point::ExPointCommand::RecordSynchronizationAction(_) => None,
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
        let mut changes = self.entity.execute_command(command, &self.hp)?;
        if matches!(changes.operation, entity::EntityOperation::Transform { .. }) {
            changes.entity.ex_point = Some(self.ex_point.get(changes.target_uid));
            changes.entity.shield_value = Some(self.hp.shield(changes.target_uid));
            self.eureka
                .sync_entity(changes.target_uid, &mut changes.entity);
            self.buff.sync_entity(&mut changes.entity);
            self.entity.update(changes.entity.clone());
        }
        self.register_entity(&changes.entity);
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
        self.entity.first_open_combat_position(source_uid, &self.hp)
    }

    pub(crate) fn execute_gauge(
        &mut self,
        command: gauge::GaugeCommand,
    ) -> Result<gauge::GaugeChange, gauge::GaugeCommandError> {
        self.gauge.execute_command(command)
    }

    pub(crate) fn execute_emitter(
        &mut self,
        command: emitter::EmitterCommand,
    ) -> emitter::EmitterChange {
        self.emitter.execute_command(command)
    }

    pub(crate) fn execute_card(
        &mut self,
        command: card::CardCommand,
    ) -> Result<card::CardChanges, card::CardCommandError> {
        let mut changes = self.card.execute_command(command)?;
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
        self.upgrade.execute_command(command)
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
        self.entity.register(entity);
        self.register_entity_state(entity);
    }

    fn register_entity_state(&mut self, entity: &FightEntityInfo) {
        let team_type = entity.team_type.unwrap_or_default();
        self.attribute.register(entity);
        self.hp.register(entity);
        self.toughness.register(entity);
        self.ex_point.register(entity);
        self.eureka.register(entity);
        self.buff.register_entity(entity, team_type);
    }

    pub(crate) fn promote_reserves(
        &mut self,
        fight: &mut Fight,
    ) -> Vec<crate::engine::fight::reserve::Promotion> {
        let mut promotions = self.entity.promote_reserves(&self.hp);
        for promotion in &mut promotions {
            if let Some(entering) = self.entity_snapshot(promotion.entering_uid) {
                promotion.entering = entering;
            }
        }
        self.sync_entities(fight);
        promotions
    }

    pub(crate) fn advance_wave(
        &mut self,
        fight: &mut Fight,
    ) -> anyhow::Result<Option<wave::WaveAdvanced>> {
        self.sync_entities(fight);
        let Some(roster) = self.wave.advance()? else {
            return Ok(None);
        };
        self.entity
            .replace_team_roster(2, &roster.entitys, &roster.sub_entitys);
        for entity in roster.entitys.iter().chain(&roster.sub_entitys) {
            self.register_entity_state(entity);
        }
        fight.cur_wave = Some(roster.wave);
        self.sync_entities(fight);
        Ok(Some(wave::WaveAdvanced {
            wave: roster.wave,
            entering_uids: roster.entering_uids,
            fight: fight.clone(),
        }))
    }

    pub fn can_fire_rule(
        &self,
        owner_uid: i64,
        skill_id: i32,
        slot_index: usize,
        condition_key: crate::engine::skill::rule::DefinitionKey,
        limit: i32,
        round_limit: i32,
    ) -> bool {
        let key = (owner_uid, skill_id, slot_index, condition_key);
        (limit <= 0 || self.rule_fires.get(&key).copied().unwrap_or_default() < limit)
            && (round_limit <= 0
                || self.round_rule_fires.get(&key).copied().unwrap_or_default() < round_limit)
    }

    pub fn mark_rule_fired(
        &mut self,
        owner_uid: i64,
        skill_id: i32,
        slot_index: usize,
        condition_key: crate::engine::skill::rule::DefinitionKey,
    ) {
        let key = (owner_uid, skill_id, slot_index, condition_key);
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
        self.round_rule_fires.clear();
        self.buff_act_fires.clear();
        self.card.begin_round();
        self.hp.begin_round();
        self.injury.begin_round();
        self.gauge.begin_combat_round();
        self.field.begin_round();
        self.conduit.begin_round();
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
    pub fn seeded(fight: &Fight) -> Self {
        let mut managers = Self {
            fight_version: fight.version.unwrap_or_default(),
            ..Self::default()
        };
        managers.attribute.seed(fight);
        managers.battle_rule = battle_rule::BattleRuleManager::seed(fight);
        managers.hp.seed(fight);
        managers.toughness.seed(fight);
        managers.ex_point.seed(fight);
        managers.eureka.seed(fight);
        managers.buff.seed(fight);
        managers.card.seed(fight);
        managers.conduit = ConduitManager::seed(fight);
        managers.entity = entity::EntityManager::seed(fight);
        managers.wave = wave::WaveManager::seed(fight);
        managers
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
        self.ex_point.sync_entity(entity);
        entity.ex_skill_point_change =
            Some(crate::engine::mechanic::card::CardMechanic.ultimate_cost_offset(self, uid));
        if let Some(progress) = self.ex_point.synchronization_progress(uid) {
            self.buff.project_synchronization(entity, progress);
        }
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
    }

    pub fn ex_point_info(&self, fight: &Fight) -> Vec<FightExPointInfo> {
        let current_uids = entities(fight)
            .filter_map(|entity| entity.uid)
            .collect::<Vec<_>>();
        self.entity
            .ordered_uids()
            .filter(|uid| current_uids.contains(uid))
            .map(|uid| FightExPointInfo {
                uid: Some(uid),
                ex_point: Some(self.ex_point.get(uid)),
                power_infos: self.eureka.power_infos(uid),
                current_hp: Some(self.hp.current(uid)),
                ex_point_type: Some(self.ex_point.kind(uid)),
            })
            .collect()
    }

    pub fn hero_sp_attributes(&self, fight: &Fight) -> Vec<FightHeroSpAttributeInfo> {
        let fight_version = fight.version.unwrap_or_default();
        entities(fight)
            .filter(|entity| entity.team_type == Some(2))
            .filter(|entity| entity.uid.is_some_and(|uid| self.hp.current(uid) > 0))
            .filter_map(|entity| {
                Some(FightHeroSpAttributeInfo {
                    uid: entity.uid,
                    attribute: Some(monster_sp_attribute(entity.model_id?, fight_version)),
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

fn monster_sp_attribute(model_id: i32, fight_version: i32) -> HeroSpAttribute {
    let Some(db) = config::try_get() else {
        return base_hero_sp_attribute(fight_version);
    };
    let Some(monster) = db.monster.get(model_id) else {
        return base_hero_sp_attribute(fight_version);
    };
    let Some(template) = db.monster_skill_template.get(monster.skill_template) else {
        return base_hero_sp_attribute(fight_version);
    };
    let Some(resistance) = db.resistances_attribute.get(template.resistance) else {
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
