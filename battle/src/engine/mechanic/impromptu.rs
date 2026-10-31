use sonettobuf::CardInfo;

use crate::engine::{
    manager::{
        BattleManagers,
        buff::ActiveBuffFeature,
        card::{CardCommand, CardEnergyAllocation, CardQueueUse},
        gauge::{GaugeCommand, GaugeKey, GaugeKind, GaugeManager, GaugeOperation, GaugeOwner},
    },
    skill::{
        buff_act::{self, registry::BuffActKind},
        effect::SkillEffectCatalog,
        rule::output::{BattleCommand, RuleOp},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImpromptuDefinition {
    skill_id: i32,
    damage_up_act_id: i32,
    damage_rate_per_inspiration: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImpromptuError {
    MissingTeamEnergy(i32),
    InvalidEmitterTag(i64),
}

impl ImpromptuDefinition {
    pub(crate) const fn new(
        skill_id: i32,
        damage_up_act_id: i32,
        damage_rate_per_inspiration: i32,
    ) -> Self {
        Self {
            skill_id,
            damage_up_act_id,
            damage_rate_per_inspiration,
        }
    }

    pub const fn skill_id(self) -> i32 {
        self.skill_id
    }

    pub const fn with_skill(mut self, skill_id: i32) -> Self {
        self.skill_id = skill_id;
        self
    }

    pub const fn damage_up_act_id(self) -> i32 {
        self.damage_up_act_id
    }

    pub const fn damage_rate(self, inspiration: i32) -> i32 {
        inspiration.saturating_mul(self.damage_rate_per_inspiration)
    }

    pub fn card(self, emitter_uid: i64) -> CardInfo {
        CardInfo {
            uid: Some(emitter_uid),
            skill_id: Some(self.skill_id),
            temp_card: Some(false),
            card_type: Some(0),
            hero_id: Some(0),
            status: Some(0),
            target_uid: Some(0),
            energy: Some(0),
            area_red_or_blue: Some(0),
            heat_id: Some(0),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImpromptuPlan {
    pub source_uid: i64,
    pub skill_id: i32,
    pub inspiration: i32,
    pub attack_count: i32,
    pub damage_rate_opcode: i32,
    pub damage_rate: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImpromptuEnable {
    pub team: i32,
    pub emitter_uid: i64,
    pub emitter: RuleOp,
    pub team_energy: RuleOp,
    pub inspiration: RuleOp,
}

pub const fn team_energy_key(team: i32) -> GaugeKey {
    GaugeKey {
        kind: GaugeKind::TeamEnergy,
        owner: GaugeOwner::Team(team),
    }
}

pub const fn inspiration_key(emitter_uid: i64) -> GaugeKey {
    GaugeKey {
        kind: GaugeKind::ImpromptuInspiration,
        owner: GaugeOwner::Entity(emitter_uid),
    }
}

pub fn enable_rule_ops(
    definition: Option<ImpromptuDefinition>,
    gauges: &GaugeManager,
    features: &[ActiveBuffFeature],
    emitter_uid: i64,
) -> Vec<ImpromptuEnable> {
    if emitter_uid == 0 {
        return Vec::new();
    }
    emitter_tags(features)
        .into_iter()
        .filter(|feature| {
            gauges.get(team_energy_key(feature.team_type)).is_none()
                || gauges.get(inspiration_key(emitter_uid)).is_none()
        })
        .filter_map(|feature| {
            let origin = buff_act::feature_command_origin(feature)?;
            let definition = definition?;
            Some(ImpromptuEnable {
                team: feature.team_type,
                emitter_uid,
                emitter: RuleOp::Command(BattleCommand::Emitter(
                    crate::engine::manager::emitter::EmitterCommand {
                        origin,
                        operation: crate::engine::manager::emitter::EmitterOperation::Enable(
                            definition,
                        ),
                    },
                )),
                team_energy: enable(origin, team_energy_key(feature.team_type)),
                inspiration: enable(origin, inspiration_key(emitter_uid)),
            })
        })
        .collect()
}

pub fn team_energy_gain_rule_op(
    gauges: &GaugeManager,
    feature: &ActiveBuffFeature,
    delta: i32,
) -> Option<RuleOp> {
    if !buff_act::is_kind(feature, BuffActKind::UseSkillTeamAddEmitterEnergy) || delta <= 0 {
        return None;
    }
    change(
        gauges,
        buff_act::feature_command_origin(feature)?,
        team_energy_key(feature.team_type),
        delta,
    )
}

pub fn eureka_spent_rule_op(
    managers: &BattleManagers,
    owner_uid: i64,
    amount: i32,
) -> Option<RuleOp> {
    if amount <= 0 {
        return None;
    }
    let team = managers.entity.team_type(owner_uid)?;
    let features = managers.buff.active_features(&managers.hp);
    let tag = emitter_tags(&features)
        .into_iter()
        .find(|feature| feature.team_type == team)?;
    change(
        &managers.gauge,
        buff_act::feature_command_origin(tag)?,
        team_energy_key(team),
        amount,
    )
}

pub fn spend_team_energy_rule_op(
    gauges: &GaugeManager,
    emitter_tag: &ActiveBuffFeature,
    amount: i32,
) -> Option<RuleOp> {
    if !buff_act::is_kind(emitter_tag, BuffActKind::EmitterTag) || amount <= 0 {
        return None;
    }
    change(
        gauges,
        buff_act::feature_command_origin(emitter_tag)?,
        team_energy_key(emitter_tag.team_type),
        -amount,
    )
}

pub fn allocate_team_energy_rule_ops(
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut crate::engine::runtime::determinism::RoundDeterminism,
    team: i32,
) -> Result<Option<Vec<RuleOp>>, ImpromptuError> {
    let features = managers.buff.active_features(&managers.hp);
    let Some(tag) = emitter_tags(&features)
        .into_iter()
        .find(|feature| feature.team_type == team)
    else {
        return Ok(None);
    };
    let limit = buff_act::emitter_card_energy_limit::resolve(
        crate::engine::manager::card::energy::BASE_LIMIT,
        &features,
        crate::engine::manager::emitter::UID,
    );
    let available = managers
        .gauge
        .get(team_energy_key(team))
        .ok_or(ImpromptuError::MissingTeamEnergy(team))?
        .current;
    let Some(cards) = crate::engine::manager::card::energy::allocate(
        &managers.buff,
        &managers.hp,
        catalog,
        managers.card.hand(),
        available,
        limit,
        determinism,
    ) else {
        return Ok(Some(Vec::new()));
    };
    let allocated = cards
        .iter()
        .zip(managers.card.hand())
        .map(|(after, before)| after.energy.unwrap_or_default() - before.energy.unwrap_or_default())
        .sum::<i32>();
    let origin = buff_act::feature_command_origin(tag)
        .ok_or(ImpromptuError::InvalidEmitterTag(tag.buff_uid))?;
    Ok(Some(vec![
        spend_team_energy_rule_op(&managers.gauge, tag, allocated)
            .ok_or(ImpromptuError::InvalidEmitterTag(tag.buff_uid))?,
        RuleOp::Command(BattleCommand::Card(CardCommand::AllocateEnergy(
            CardEnergyAllocation {
                origin,
                energies: cards
                    .into_iter()
                    .map(|card| card.energy.unwrap_or_default())
                    .collect(),
                limit,
            },
        ))),
    ]))
}

fn emitter_tags(features: &[ActiveBuffFeature]) -> Vec<&ActiveBuffFeature> {
    let mut tags = features
        .iter()
        .filter(|feature| {
            feature.owner_alive
                && feature.team_type != 0
                && buff_act::is_kind(feature, BuffActKind::EmitterTag)
        })
        .collect::<Vec<_>>();
    tags.sort_by_key(|feature| (feature.team_type, feature.owner_uid, feature.buff_uid));
    tags.dedup_by_key(|feature| feature.team_type);
    tags
}

pub fn collect_inspiration_rule_op(
    gauges: &GaugeManager,
    emitter_tag: &ActiveBuffFeature,
    emitter_uid: i64,
    cards: &[CardInfo],
) -> Option<RuleOp> {
    if !buff_act::is_kind(emitter_tag, BuffActKind::EmitterTag) {
        return None;
    }
    let delta = cards
        .iter()
        .map(|card| card.energy.unwrap_or_default())
        .fold(0_i32, i32::saturating_add);
    if delta <= 0 {
        return None;
    }
    change(
        gauges,
        buff_act::feature_command_origin(emitter_tag)?,
        inspiration_key(emitter_uid),
        delta,
    )
}

pub fn action_queue_committed_rule_ops(
    managers: &BattleManagers,
    team: i32,
    emitter_uid: i64,
    cards: &[CardInfo],
) -> Vec<RuleOp> {
    let features = managers.buff.active_features(&managers.hp);
    let Some(tag) = emitter_tags(&features)
        .into_iter()
        .find(|feature| feature.team_type == team)
    else {
        return Vec::new();
    };
    let current = managers
        .gauge
        .get(inspiration_key(emitter_uid))
        .map(|state| state.current)
        .unwrap_or_default();
    let gained = cards
        .iter()
        .map(|card| card.energy.unwrap_or_default())
        .fold(0_i32, i32::saturating_add);
    let mut ops = collect_inspiration_rule_op(&managers.gauge, tag, emitter_uid, cards)
        .into_iter()
        .collect::<Vec<_>>();
    if current.saturating_add(gained) > 0
        && let Some(definition) = buff_act::emitter_skill::resolve(
            managers.catalog().impromptu_definition(),
            &features,
            emitter_uid,
        )
        && let Some(origin) = buff_act::feature_command_origin(tag)
    {
        ops.push(RuleOp::Command(BattleCommand::Card(
            CardCommand::QueueUseCard(CardQueueUse {
                origin,
                card_index: cards.len() as i32 + 1,
                card: definition.card(emitter_uid),
                team_type: team,
                source_skill_id: 0,
                action: None,
            }),
        )));
    }
    ops
}

pub fn finalize_action_queue_rule_ops(
    managers: &BattleManagers,
    team: i32,
    emitter_uid: i64,
) -> Vec<RuleOp> {
    let Some(origin) = action_queue_finalization_origin(managers, team, emitter_uid) else {
        return Vec::new();
    };
    let key = inspiration_key(emitter_uid);
    vec![
        RuleOp::Command(BattleCommand::Card(CardCommand::ClearEnergy { origin })),
        RuleOp::Command(BattleCommand::Gauge(GaugeCommand::new(
            origin,
            key,
            GaugeOperation::Snapshot,
        ))),
    ]
}

fn action_queue_finalization_origin(
    managers: &BattleManagers,
    team: i32,
    emitter_uid: i64,
) -> Option<crate::engine::skill::rule::CommandOrigin> {
    let features = managers.buff.active_features(&managers.hp);
    let tag = emitter_tags(&features)
        .into_iter()
        .find(|feature| feature.team_type == team)?;
    managers.gauge.get(inspiration_key(emitter_uid))?;
    buff_act::feature_command_origin(tag)
}

pub fn build_plan(managers: &BattleManagers, team: i32, emitter_uid: i64) -> Option<ImpromptuPlan> {
    let features = managers.buff.active_features(&managers.hp);
    emitter_tags(&features)
        .into_iter()
        .any(|feature| feature.team_type == team)
        .then_some(())?;
    let inspiration = managers.gauge.get(inspiration_key(emitter_uid))?.current;
    if inspiration <= 0 {
        return None;
    }
    let definition = buff_act::emitter_skill::resolve(
        managers.catalog().impromptu_definition(),
        &features,
        emitter_uid,
    )?;
    let attacks = crate::engine::skill::buff_act::emitter_num_change::resolve(
        &managers.buff,
        &managers.hp,
        emitter_uid,
    );
    Some(ImpromptuPlan {
        source_uid: emitter_uid,
        skill_id: definition.skill_id(),
        inspiration,
        attack_count: attacks.count,
        damage_rate_opcode: definition.damage_up_act_id(),
        damage_rate: definition
            .damage_rate(inspiration)
            .saturating_add(attacks.damage_rate),
    })
}

pub fn resolved_rule_ops(managers: &BattleManagers, team: i32, emitter_uid: i64) -> Vec<RuleOp> {
    let features = managers.buff.active_features(&managers.hp);
    let Some(tag) = emitter_tags(&features)
        .into_iter()
        .find(|feature| feature.team_type == team)
    else {
        return Vec::new();
    };
    reset_inspiration_rule_op(&managers.gauge, tag, emitter_uid)
        .into_iter()
        .collect()
}

pub fn reset_inspiration_rule_op(
    gauges: &GaugeManager,
    emitter_tag: &ActiveBuffFeature,
    emitter_uid: i64,
) -> Option<RuleOp> {
    if !buff_act::is_kind(emitter_tag, BuffActKind::EmitterTag) {
        return None;
    }
    let current = gauges.get(inspiration_key(emitter_uid))?.current;
    change(
        gauges,
        buff_act::feature_command_origin(emitter_tag)?,
        inspiration_key(emitter_uid),
        -current,
    )
}

fn enable(origin: crate::engine::skill::rule::CommandOrigin, key: GaugeKey) -> RuleOp {
    RuleOp::Command(BattleCommand::Gauge(GaugeCommand::new(
        origin,
        key,
        GaugeOperation::Enable { max: None },
    )))
}

fn change(
    gauges: &GaugeManager,
    origin: crate::engine::skill::rule::CommandOrigin,
    key: GaugeKey,
    delta: i32,
) -> Option<RuleOp> {
    (delta != 0 && gauges.get(key).is_some()).then_some(RuleOp::Command(BattleCommand::Gauge(
        GaugeCommand::new(origin, key, GaugeOperation::ChangeValue { delta }),
    )))
}

#[cfg(test)]
mod tests;
