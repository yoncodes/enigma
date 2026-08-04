use std::collections::HashMap;

use crate::engine::{
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffCommand, BuffSetState},
        card::CardCommand,
        gauge::{GaugeCommand, GaugeKind, GaugeOperation},
    },
    skill::{
        behavior::{BehaviorOpContext, classify::BehaviorKind, registry::BehaviorHandler},
        buff_act::{is_kind, registry::BuffActKind},
        effect::{SkillEffectCatalog, slot::ParsedBehavior},
        rule::output::{BattleCommand, RuleOp},
    },
};

const RAW_LINGERING_GLOW_PER_LAYER: i32 = 5_000;

pub(crate) struct Handler;

impl BehaviorHandler for Handler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        if behavior.spec.kind == BehaviorKind::AddHeatScaleFromBuff {
            let amount = context
                .managers
                .buff
                .active_features(&context.managers.hp)
                .into_iter()
                .filter(|feature| {
                    feature.owner_uid == context.target_uid
                        && is_kind(feature, BuffActKind::AttrByHeatScale)
                })
                .filter_map(|feature| {
                    let act_id = feature.act_id()?;
                    context
                        .managers
                        .buff
                        .snapshot(feature.owner_uid, feature.buff_uid)
                        .map(|buff| recorded_heat_scale_amount(&buff, act_id))
                })
                .max()
                .unwrap_or_default();
            if context.source_team == 0 || amount <= 0 {
                return Some(Vec::new());
            }
            let command = GaugeCommand::new(
                super::super::skill::behavior::command_origin(behavior)?,
                super::lingering_glow::key(context.source_team),
                GaugeOperation::ChangeValue { delta: amount },
            )
            .attributed_to(
                context.source_uid,
                GaugeKind::LingeringGlow.shared_pool_config_effect(),
            )
            .with_raw_delta(amount.saturating_mul(1000));
            return Some(super::lingering_glow::value_change_rule_ops(
                context.managers,
                command,
            ));
        }
        if matches!(behavior.spec.kind, BehaviorKind::AddCardRankNext) {
            let count = behavior.arg(0)?;
            let levels = behavior.arg(1)?;
            if context.target.active_card_index <= 0 || count <= 0 || levels <= 0 {
                return Some(Vec::new());
            }
            return Some(vec![RuleOp::Command(BattleCommand::Card(
                CardCommand::RankUpQueued {
                    origin: super::super::skill::behavior::command_origin(behavior)?,
                    after_card_index: context.target.active_card_index,
                    count,
                    levels,
                },
            ))]);
        }
        let raw_delta = behavior.arg(0)?;
        let delta = normalize_skill_gain(raw_delta);
        if context.source_team == 0 || delta == 0 {
            return Some(Vec::new());
        }
        let raw_delta = if raw_delta.abs() >= 1000 {
            raw_delta
        } else {
            delta.saturating_mul(1000)
        };
        let command = GaugeCommand::new(
            super::super::skill::behavior::command_origin(behavior)?,
            super::lingering_glow::key(context.source_team),
            GaugeOperation::AccumulateProgress {
                raw_amount: raw_delta,
            },
        )
        .attributed_to(
            context.source_uid,
            GaugeKind::LingeringGlow.shared_pool_config_effect(),
        );
        Some(super::lingering_glow::value_change_rule_ops(
            context.managers,
            command,
        ))
    }
}

fn recorded_heat_scale_amount(buff: &sonettobuf::BuffInfo, act_id: i32) -> i32 {
    buff.act_info
        .iter()
        .find(|info| info.act_id == Some(act_id))
        .and_then(|info| info.param.first())
        .copied()
        .unwrap_or_default()
        .max(0)
}

fn normalize_skill_gain(raw: i32) -> i32 {
    if raw.abs() >= 1000 { raw / 1000 } else { raw }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeatScaleCreate {
    pub team: i32,
    pub amount: i32,
    pub raw_amount: i32,
    pub source_buff_id: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeatScaleUseSkillInfo {
    pub owner_uid: i64,
    pub buff_uid: i64,
    pub act_id: i32,
    pub current: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeatScaleBurnChange {
    pub target_uid: i64,
    pub team: i32,
    pub amount: i32,
    pub current: i32,
    pub use_skill: Option<HeatScaleUseSkillInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeatScaleGain {
    pub amount: i32,
    pub raw_amount: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BurnOrHaloAdded {
    pub source_team: i32,
    pub target_uid: i64,
    pub buff_uid: i64,
    pub added_layers: i32,
    pub alive_enemy_index: usize,
    pub alive_enemy_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeatScaleCounterInfo {
    pub owner_uid: i64,
    pub buff_uid: i64,
    pub act_id: i32,
    pub value: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeatScaleCast {
    pub owner_uid: i64,
    pub buff_uid: i64,
    pub buff_id: i32,
    pub act_id: i32,
    pub skill_id: i32,
    pub trigger_value: i32,
    pub current: i32,
    pub consume_buff_id: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeatScaleCastSelection {
    pub threshold: i32,
    pub skill_id: i32,
    pub consume_buff_id: Option<i32>,
}

pub fn referenced_skills(raw: &str) -> impl Iterator<Item = i32> + '_ {
    raw.split('#')
        .nth(2)
        .into_iter()
        .flat_map(|skills| skills.split(','))
        .filter_map(|skill| skill.parse().ok())
}

pub fn ready_cast_selection(
    raw: &str,
    green_count: i32,
    current: i32,
    has_buff: impl FnOnce(i32) -> bool,
) -> Option<HeatScaleCastSelection> {
    let parts = raw.split('#').collect::<Vec<_>>();
    let base_threshold = parts.get(1)?.parse::<i32>().ok()? / 1000;
    let skills = referenced_skills(raw).collect::<Vec<_>>();
    let threshold_buff_id = parts.get(5)?.parse::<i32>().ok()?;
    let has_threshold_buff = has_buff(threshold_buff_id);
    let threshold_delta = parts.get(6)?.parse::<i32>().ok()? / 1000;
    let threshold = (base_threshold
        + if has_threshold_buff {
            threshold_delta
        } else {
            0
        })
    .max(1);
    if current < threshold {
        return None;
    }
    let green_count = green_count.max(0) as usize;
    Some(HeatScaleCastSelection {
        threshold,
        skill_id: *skills.get(green_count.min(skills.len().saturating_sub(1)))?,
        consume_buff_id: has_threshold_buff.then_some(threshold_buff_id),
    })
}

pub struct HeatScaleCastRequest<'a> {
    pub owner_uid: i64,
    pub buff_uid: i64,
    pub buff_id: i32,
    pub act_id: i32,
    pub team: i32,
    pub raw: &'a str,
    pub has_threshold_buff: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HeatScaleState {
    max: i32,
    current: i32,
    raw_max: i32,
    raw_current: i32,
    source_buff_id: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CrystalSelectionChanges {
    pub state: crate::engine::manager::buff::BuffChanges,
    pub marker: crate::engine::manager::buff::BuffActInfoMarkerResult,
}

#[derive(Debug, Clone, Default)]
pub struct HeatScale {
    states: HashMap<i32, HeatScaleState>,
}

impl HeatScale {
    pub fn select_crystals_from_cloth(
        managers: &mut BattleManagers,
        owner_uid: i64,
        packed: i32,
    ) -> Option<CrystalSelectionChanges> {
        let feature = managers
            .buff
            .active_features(&managers.hp)
            .into_iter()
            .find(|feature| {
                feature.owner_uid == owner_uid && is_kind(feature, BuffActKind::CrystalNotifySelect)
            })?;
        let total = feature.values.get(1).copied()?;
        let per_crystal = feature.values.get(2).copied()?;
        let counts = [packed / 100, packed / 10 % 10, packed % 10];
        if counts.iter().sum::<i32>() != total
            || counts
                .iter()
                .any(|count| *count < 0 || *count > per_crystal)
            || !managers.emanation.select(owner_uid, packed)
        {
            return None;
        }
        let act_id = feature.act_id()?;
        let params = vec![total, per_crystal, 1];
        let state = managers
            .execute_buff(BuffCommand::SetState(BuffSetState {
                ex_info: None,
                origin: crate::engine::skill::buff_act::feature_command_origin(&feature)?,
                target_uid: owner_uid,
                buff_uid: feature.buff_uid,
                params: None,
                act_info: Some(vec![sonettobuf::BuffActInfo {
                    act_id: Some(act_id),
                    param: params.clone(),
                    str_param: Some(String::new()),
                }]),
            }))
            .ok()?;
        if state.change.refreshed.is_empty() {
            return None;
        }
        Some(CrystalSelectionChanges {
            state,
            marker: crate::engine::manager::buff::BuffActInfoMarkerResult {
                target_uid: owner_uid,
                buff_uid: feature.buff_uid,
                act_id,
                params,
                str_param: Some(String::new()),
                team_type: feature.team_type,
            },
        })
    }

    pub fn create_from_features(
        &mut self,
        features: &[ActiveBuffFeature],
        catalog: &SkillEffectCatalog,
    ) -> Vec<HeatScaleCreate> {
        creation_specs(features, catalog)
            .into_iter()
            .filter(|create| self.create(*create))
            .collect()
    }

    pub fn has_state(&self, team: i32) -> bool {
        self.states.contains_key(&team)
    }

    pub fn value(&self, team: i32) -> i32 {
        self.states
            .get(&team)
            .map(|state| state.current)
            .unwrap_or_default()
    }

    pub fn raw_value(&self, team: i32) -> i32 {
        self.states
            .get(&team)
            .map(|state| state.raw_current)
            .unwrap_or_default()
    }

    pub fn source_buff_id(&self, team: i32) -> i32 {
        self.states
            .get(&team)
            .map(|state| state.source_buff_id)
            .unwrap_or_default()
    }

    pub fn apply_value(&mut self, team: i32, delta: i32, raw_delta: i32) -> Option<i32> {
        let state = self.states.get_mut(&team)?;
        state.current = (state.current + delta).clamp(0, state.max.max(0));
        state.raw_current = (state.raw_current + raw_delta).clamp(0, state.raw_max.max(0));
        Some(state.current)
    }

    pub fn use_skill_info(
        &self,
        features: &[ActiveBuffFeature],
        team: i32,
    ) -> Option<HeatScaleUseSkillInfo> {
        use_skill_info(self.value(team), features, team)
    }

    pub fn on_burn_added(
        &mut self,
        features: &[ActiveBuffFeature],
        added: BurnOrHaloAdded,
    ) -> Option<HeatScaleBurnChange> {
        let BurnOrHaloAdded {
            source_team,
            target_uid,
            ..
        } = added;
        if source_team == 0 || !self.has_state(source_team) {
            return None;
        }
        let gain = burn_or_halo_gain(features, added)?;
        let current = self.apply_value(source_team, gain.amount, gain.raw_amount)?;
        Some(HeatScaleBurnChange {
            target_uid,
            team: source_team,
            amount: gain.amount,
            current,
            use_skill: self.use_skill_info(features, source_team),
        })
    }

    pub fn decr_counter_info(
        &self,
        features: &[ActiveBuffFeature],
        team: i32,
    ) -> Option<HeatScaleCounterInfo> {
        decr_counter_info(self.raw_value(team), features, team)
    }

    pub fn take_ready_cast(
        &mut self,
        emanation: &crate::engine::manager::emanation::EmanationManager,
        request: HeatScaleCastRequest<'_>,
    ) -> Option<HeatScaleCast> {
        let HeatScaleCastRequest {
            owner_uid,
            buff_uid,
            buff_id,
            act_id,
            team,
            raw,
            has_threshold_buff,
        } = request;
        let trigger_value = self.value(team);
        let selected = ready_cast_selection(
            raw,
            emanation.count(
                owner_uid,
                crate::engine::manager::emanation::EmanationKind::Green,
            ),
            trigger_value,
            |_| has_threshold_buff,
        )?;
        let current = self.apply_value(team, -selected.threshold, -selected.threshold * 1000)?;
        Some(HeatScaleCast {
            owner_uid,
            buff_uid,
            buff_id,
            act_id,
            skill_id: selected.skill_id,
            trigger_value,
            current,
            consume_buff_id: selected.consume_buff_id,
        })
    }

    fn create(&mut self, create: HeatScaleCreate) -> bool {
        if create.team == 0 || create.amount <= 0 || self.states.contains_key(&create.team) {
            return false;
        }
        self.states.insert(
            create.team,
            HeatScaleState {
                max: create.amount,
                current: 0,
                raw_max: raw_value(create.raw_amount, create.amount),
                raw_current: 0,
                source_buff_id: create.source_buff_id,
            },
        );
        true
    }
}

pub fn use_skill_info(
    current: i32,
    features: &[ActiveBuffFeature],
    team: i32,
) -> Option<HeatScaleUseSkillInfo> {
    features
        .iter()
        .filter(|feature| feature.team_type == team && feature.owner_alive)
        .find(|feature| is_kind(feature, BuffActKind::HeatScaleUseSkill))
        .map(|feature| HeatScaleUseSkillInfo {
            owner_uid: feature.owner_uid,
            buff_uid: feature.buff_uid,
            act_id: feature.act_id().unwrap_or_default(),
            current,
        })
}

pub fn decr_counter_info(
    raw_current: i32,
    features: &[ActiveBuffFeature],
    team: i32,
) -> Option<HeatScaleCounterInfo> {
    features
        .iter()
        .find(|feature| {
            feature.team_type == team && is_kind(feature, BuffActKind::HeatScaleDecrCounter)
        })
        .map(|feature| HeatScaleCounterInfo {
            owner_uid: feature.owner_uid,
            buff_uid: feature.buff_uid,
            act_id: feature.act_id().unwrap_or_default(),
            value: raw_current * feature.values.get(1).copied().unwrap_or_default().max(0) / 1000,
        })
}

pub fn creation_specs(
    features: &[ActiveBuffFeature],
    catalog: &SkillEffectCatalog,
) -> Vec<HeatScaleCreate> {
    let mut by_team = HashMap::<i32, HeatScaleCreate>::new();
    for tag in features {
        if tag.team_type == 0 || !tag.owner_alive || !is_kind(tag, BuffActKind::HeatScaleTag) {
            continue;
        }
        let Some((amount, raw_amount)) = features
            .iter()
            .filter(|feature| {
                feature.team_type == tag.team_type
                    && feature.owner_alive
                    && is_kind(feature, BuffActKind::CardNotCalSize)
            })
            .filter_map(|feature| linked_heat_scale_amount(feature, catalog))
            .max_by_key(|(amount, _)| *amount)
        else {
            continue;
        };
        let create = HeatScaleCreate {
            team: tag.team_type,
            amount,
            raw_amount,
            source_buff_id: tag.buff_id,
        };
        if by_team
            .get(&tag.team_type)
            .is_none_or(|current| create.amount > current.amount)
        {
            by_team.insert(tag.team_type, create);
        }
    }

    let mut creates: Vec<_> = by_team.into_values().collect();
    creates.sort_by_key(|create| create.team);
    creates
}

pub fn burn_or_halo_gain(
    features: &[ActiveBuffFeature],
    added: BurnOrHaloAdded,
) -> Option<HeatScaleGain> {
    let BurnOrHaloAdded {
        source_team,
        target_uid,
        buff_uid,
        added_layers,
        alive_enemy_index: _,
        alive_enemy_count,
    } = added;
    let trigger = features.iter().find(|feature| {
        feature.owner_uid == target_uid && feature.buff_uid == buff_uid && is_burn_or_halo(feature)
    })?;
    if source_team == 0
        || trigger.team_type == source_team
        || added_layers <= 0
        || alive_enemy_count == 0
    {
        return None;
    }
    let enemy_count = i32::try_from(alive_enemy_count).ok()?;
    let per_layer = RAW_LINGERING_GLOW_PER_LAYER / enemy_count;
    let base_raw = per_layer.saturating_mul(added_layers);
    let modifier = heat_scale_gain_modifier(features, source_team, trigger).max(-1_000);
    let raw_amount = (i64::from(base_raw) * i64::from(1_000 + modifier) / 1_000)
        .clamp(0, i64::from(i32::MAX)) as i32;
    if raw_amount == 0 {
        return None;
    }
    Some(HeatScaleGain {
        amount: raw_amount / 1000,
        raw_amount,
    })
}

pub fn lingering_glow_gain_modifier(features: &[ActiveBuffFeature], team: i32) -> i32 {
    features
        .iter()
        .filter(|feature| feature.team_type == team && feature.owner_alive)
        .filter_map(|feature| {
            is_kind(feature, BuffActKind::HeatScaleAddFix)
                .then(|| feature.values.get(1).copied())
                .flatten()
        })
        .fold(0, i32::saturating_add)
}

fn heat_scale_gain_modifier(
    features: &[ActiveBuffFeature],
    source_team: i32,
    trigger: &ActiveBuffFeature,
) -> i32 {
    let trigger_is_burn = is_kind(trigger, BuffActKind::Burn);
    lingering_glow_gain_modifier(features, source_team).saturating_add(
        features
            .iter()
            .filter(|feature| feature.team_type == source_team && feature.owner_alive)
            .filter_map(
                |feature| match crate::engine::skill::buff_act::feature_kind(feature) {
                    Some(BuffActKind::HeatScaleBurnAddFix) if trigger_is_burn => {
                        feature.values.get(1).copied()
                    }
                    _ => None,
                },
            )
            .fold(0, i32::saturating_add),
    )
}

fn linked_heat_scale_amount(
    feature: &ActiveBuffFeature,
    catalog: &SkillEffectCatalog,
) -> Option<(i32, i32)> {
    linked_skill_ids(&feature.raw)
        .into_iter()
        .filter_map(|skill_id| catalog.get(skill_id))
        .flat_map(|effect| &effect.slots)
        .filter_map(|slot| heat_scale_amount(&slot.behavior))
        .max_by_key(|(amount, _)| *amount)
}

fn heat_scale_amount(behavior: &ParsedBehavior) -> Option<(i32, i32)> {
    if !matches!(
        (behavior.spec.key.opcode, behavior.spec.kind),
        (60246, BehaviorKind::HeatScaleUseSkillAddCount)
    ) {
        return None;
    }
    let raw = behavior.arg(0)?;
    let amount = if raw.abs() >= 100 { raw / 100 } else { raw };
    (amount > 0).then_some((amount, raw))
}

fn linked_skill_ids(raw: &str) -> Vec<i32> {
    raw.split('#')
        .skip(1)
        .flat_map(|part| part.split(','))
        .filter_map(|part| part.trim().parse().ok())
        .collect()
}

fn raw_value(raw_amount: i32, amount: i32) -> i32 {
    if raw_amount != 0 {
        raw_amount
    } else {
        amount * 1000
    }
}

pub fn attribute_buff() -> Option<(i32, i32)> {
    const ATTRIBUTE_BUFF_CONFIG_ID: i32 = 2;
    let db = config::try_get()?;
    let buff_id = db
        .fight_jgz_const
        .get(ATTRIBUTE_BUFF_CONFIG_ID)?
        .value
        .parse()
        .ok()?;
    let buff = db.skill_buff.get(buff_id)?;
    attr_value(&buff.features).map(|(act_id, _)| (buff_id, act_id))
}

fn attr_value(features: &str) -> Option<(i32, i32)> {
    features.split('|').find_map(|feature| {
        let values: Vec<_> = feature
            .split('#')
            .filter_map(|part| part.trim().parse::<i32>().ok())
            .collect();
        let act_id = *values.first()?;
        let act = config::try_get()?.buff_act.get(act_id)?;
        if crate::engine::skill::buff_act::registry::kind(act_id, &act.r#type)
            != Some(crate::engine::skill::buff_act::registry::BuffActKind::AttrByHeatScale)
        {
            return None;
        }
        Some((act_id, *values.get(2)?))
    })
}

fn is_burn_or_halo(feature: &ActiveBuffFeature) -> bool {
    matches!(
        crate::engine::skill::buff_act::feature_kind(feature),
        Some(
            BuffActKind::Burn
                | BuffActKind::Radiance
                | BuffActKind::MasterHalo
                | BuffActKind::LayerMasterHalo
        )
    )
}

#[cfg(test)]
mod test;
