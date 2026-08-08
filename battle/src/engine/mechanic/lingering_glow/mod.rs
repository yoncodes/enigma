use std::collections::HashMap;

use sonettobuf::BuffActInfo;

use crate::engine::{
    manager::{
        buff::{
            ActiveBuffFeature, BuffActInfoMarkerResult, BuffCommand, BuffConsume, BuffGrantChild,
            BuffManager, BuffSelector, BuffSetState, DepletedBuff,
        },
        emanation::EmanationManager,
        gauge::{
            GaugeChange, GaugeChangeKind, GaugeCommand, GaugeKey, GaugeKind, GaugeManager,
            GaugeOperation, GaugeOwner,
        },
    },
    mechanic::{
        bloodtithe,
        heat_scale::{
            self, HeatScaleCast, HeatScaleCreate, HeatScaleUseSkillInfo, ready_cast_selection,
        },
    },
    skill::{
        action::{SkillExecutionMode, SkillInvocation, SkillRequest},
        buff_act::{self, registry::BuffActKind},
        condition::extra::skill_kind_from_is_extra,
        effect::SkillEffectCatalog,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RawState {
    current: i32,
    max: i32,
    source_buff_id: i32,
}

#[derive(Debug, Clone, Default)]
pub struct LingeringGlowRuntime {
    raw: HashMap<i32, RawState>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LingeringGlowEnable {
    pub create: HeatScaleCreate,
    pub output: RuleOp,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LingeringGlowInput {
    pub raw_delta: i32,
    pub output: RuleOp,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LingeringGlowCast {
    pub cast: HeatScaleCast,
    pub outputs: Vec<RuleOp>,
}

pub const fn key(team: i32) -> GaugeKey {
    GaugeKey {
        kind: GaugeKind::LingeringGlow,
        owner: GaugeOwner::Team(team),
    }
}

pub fn enable_rule_ops(
    gauges: &GaugeManager,
    features: &[ActiveBuffFeature],
    catalog: &SkillEffectCatalog,
) -> Vec<LingeringGlowEnable> {
    heat_scale::creation_specs(features, catalog)
        .into_iter()
        .filter(|create| gauges.get(key(create.team)).is_none())
        .filter(|create| gauges.get(bloodtithe::rule::key(create.team)).is_none())
        .filter_map(|create| {
            let tag = features.iter().find(|feature| {
                feature.team_type == create.team
                    && feature.buff_id == create.source_buff_id
                    && buff_act::is_kind(feature, BuffActKind::HeatScaleTag)
            })?;
            Some(LingeringGlowEnable {
                create,
                output: RuleOp::Command(BattleCommand::Gauge(
                    GaugeCommand::new(
                        buff_act::feature_command_origin(tag)?,
                        key(create.team),
                        GaugeOperation::Enable {
                            max: Some(create.amount),
                        },
                    )
                    .attributed_to(
                        tag.owner_uid,
                        GaugeKind::LingeringGlow.shared_pool_config_effect(),
                    ),
                )),
            })
        })
        .collect()
}

pub fn round_start_attribute_rule_ops_for_team(
    managers: &crate::engine::manager::BattleManagers,
    catalog: &SkillEffectCatalog,
    team: i32,
) -> Vec<RuleOp> {
    let features = managers.buff.active_features(&managers.hp);
    let Some(tag) = features.iter().find(|feature| {
        feature.team_type == team && buff_act::is_kind(feature, BuffActKind::HeatScaleTag)
    }) else {
        return Vec::new();
    };
    let Some(origin) = buff_act::feature_command_origin(tag) else {
        return Vec::new();
    };
    let mut outputs = Vec::new();
    for create in heat_scale::creation_specs(&features, catalog)
        .into_iter()
        .filter(|create| create.team == team)
    {
        let Some(state) = managers.gauge.get(key(create.team)) else {
            continue;
        };
        let Some((buff_id, act_id)) = heat_scale::attribute_buff() else {
            continue;
        };
        if state.current <= 0 {
            continue;
        }
        let Some(buff_origin) =
            buff_act::configured_command_origin(act_id, BuffActKind::AttrByHeatScale)
        else {
            continue;
        };
        let remaining = state.current / 2;
        let depleted = state.current - remaining;
        let targets = managers
            .buff
            .alive_team_uids(create.team, &managers.hp)
            .into_iter()
            .filter(|target_uid| !managers.buff.has_buff_id(*target_uid, buff_id))
            .collect::<Vec<_>>();
        if targets.is_empty() {
            continue;
        }
        outputs.extend(targets.into_iter().map(|target_uid| {
            RuleOp::Command(BattleCommand::Buff(BuffCommand::GrantChild(
                BuffGrantChild {
                    origin: buff_origin,
                    source_uid: 0,
                    target_uid,
                    buff_id,
                    amount: Some(0),
                    params: None,
                    act_info: Some(vec![BuffActInfo {
                        act_id: Some(act_id),
                        param: vec![remaining],
                        str_param: Some(String::new()),
                    }]),
                },
            )))
        }));
        if depleted == 0 {
            continue;
        }
        let raw_current = managers
            .gauge
            .raw_value(key(team))
            .unwrap_or_else(|| state.current.saturating_mul(1000));
        let depleted_raw = raw_current - raw_current / 2;
        outputs.push(RuleOp::Command(BattleCommand::Gauge(
            GaugeCommand::new(
                origin,
                key(team),
                GaugeOperation::ChangeValue { delta: -depleted },
            )
            .attributed_to(0, GaugeKind::LingeringGlow.shared_pool_config_effect())
            .with_raw_delta(-depleted_raw),
        )));
        for counter in heat_scale::decr_counter_infos(depleted_raw, &features, team) {
            let Some(counter_origin) = buff_act::configured_command_origin(
                counter.act_id,
                BuffActKind::HeatScaleDecrCounter,
            ) else {
                continue;
            };
            let mut act_info = managers
                .buff
                .snapshot(counter.owner_uid, counter.buff_uid)
                .map(|buff| buff.act_info)
                .unwrap_or_default();
            let previous = act_info
                .iter()
                .find(|info| info.act_id == Some(counter.act_id))
                .and_then(|info| info.param.first())
                .copied()
                .unwrap_or_default();
            let counter_value = previous.saturating_add(counter.value);
            if let Some(info) = act_info
                .iter_mut()
                .find(|info| info.act_id == Some(counter.act_id))
            {
                info.param = vec![counter_value];
            } else {
                act_info.push(BuffActInfo {
                    act_id: Some(counter.act_id),
                    param: vec![counter_value],
                    str_param: Some(String::new()),
                });
            }
            outputs.push(RuleOp::Command(BattleCommand::Buff(
                BuffCommand::SetInternalState(BuffSetState {
                    origin: counter_origin,
                    target_uid: counter.owner_uid,
                    buff_uid: counter.buff_uid,
                    ex_info: None,
                    params: None,
                    act_info: Some(act_info),
                }),
            )));
            outputs.push(RuleOp::BuffActInfoMarker(BuffActInfoMarkerResult {
                target_uid: counter.owner_uid,
                buff_uid: counter.buff_uid,
                act_id: counter.act_id,
                params: vec![counter_value],
                str_param: Some(String::new()),
                team_type: 0,
            }));
        }
    }
    outputs
}

pub fn burn_or_halo_rule_op(
    gauges: &GaugeManager,
    features: &[ActiveBuffFeature],
    added: heat_scale::BurnOrHaloAdded,
) -> Option<LingeringGlowInput> {
    let heat_scale::BurnOrHaloAdded {
        source_team,
        target_uid,
        buff_uid,
        ..
    } = added;
    let key = key(source_team);
    gauges.get(key)?;
    let trigger = features
        .iter()
        .find(|feature| feature.owner_uid == target_uid && feature.buff_uid == buff_uid)?;
    let gain = heat_scale::burn_or_halo_gain(features, added)?;
    Some(LingeringGlowInput {
        raw_delta: gain.raw_amount,
        output: RuleOp::Command(BattleCommand::Gauge(
            GaugeCommand::new(
                buff_act::feature_command_origin(trigger)?,
                key,
                GaugeOperation::AccumulateRawValue {
                    amount: gain.raw_amount,
                    stream: trigger.act_id()?,
                },
            )
            .attributed_to(
                target_uid,
                GaugeKind::LingeringGlow.shared_pool_config_effect(),
            )
            .with_raw_delta(gain.raw_amount),
        )),
    })
}

pub fn value_change_rule_ops(
    managers: &crate::engine::manager::BattleManagers,
    mut command: GaugeCommand,
) -> Vec<RuleOp> {
    if let GaugeOperation::ChangeValue { delta } = command.operation
        && delta >= 0
        && let Some(raw_delta) = command.raw_delta
        && raw_delta > 0
    {
        let GaugeOwner::Team(team) = command.key.owner else {
            return Vec::new();
        };
        let modifier = heat_scale::lingering_glow_gain_modifier(
            &managers.buff.active_features(&managers.hp),
            team,
        );
        let adjusted_raw = (i64::from(raw_delta)
            * i64::from(1_000_i32.saturating_add(modifier.max(-1_000)))
            / 1_000)
            .clamp(0, i64::from(i32::MAX)) as i32;
        command.operation = GaugeOperation::AccumulateRawValue {
            amount: adjusted_raw,
            stream: command.origin.key.opcode.max(1),
        };
        command.raw_delta = Some(adjusted_raw);
        if command.progress_raw_delta.is_some() {
            command.progress_raw_delta = Some(adjusted_raw);
        }
    }
    vec![RuleOp::Command(BattleCommand::Gauge(command))]
}

pub fn visible_counter_info(
    gauges: &GaugeManager,
    features: &[ActiveBuffFeature],
    team: i32,
) -> Option<HeatScaleUseSkillInfo> {
    let listener = heat_scale::use_skill_info(0, features, team)?;
    let raw = gauges.accumulated_raw_value(key(team), listener.buff_uid, listener.act_id)?;
    let current = raw / 1000;
    heat_scale::use_skill_info(current, features, team)
}

pub fn ready_cast_rule_ops(
    gauges: &GaugeManager,
    buffs: &BuffManager,
    emanation: &EmanationManager,
    catalog: &SkillEffectCatalog,
    subscriber: &BuffActSubscriber,
) -> Option<LingeringGlowCast> {
    if !subscriber.owner_alive
        || !buff_act::subscriber_is_kind(
            subscriber,
            buff_act::registry::BuffActKind::HeatScaleUseSkill,
        )
    {
        return None;
    }
    let origin = buff_act::command_origin(subscriber)?;
    let gauge_key = key(subscriber.team_type);
    let current_raw = gauges.accumulated_raw_value(
        gauge_key,
        subscriber.buff_uid,
        subscriber.key.definition.opcode,
    )?;
    let current = current_raw / 1000;
    let selected = ready_cast_selection(
        &subscriber.raw,
        emanation.count(
            subscriber.owner_uid,
            crate::engine::manager::emanation::EmanationKind::Green,
        ),
        current,
        |buff_id| buffs.has_buff_id_or_type(subscriber.owner_uid, buff_id),
    )?;
    let after = current - selected.threshold;
    let cast = HeatScaleCast {
        owner_uid: subscriber.owner_uid,
        buff_uid: subscriber.buff_uid,
        buff_id: subscriber.buff_id,
        act_id: subscriber.key.definition.opcode,
        skill_id: selected.skill_id,
        trigger_value: current,
        current: after,
        consume_buff_id: selected.consume_buff_id,
    };
    let mut outputs = Vec::new();
    if let Some(buff_id) = selected.consume_buff_id {
        outputs.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(
            BuffConsume {
                origin,
                target_uid: subscriber.owner_uid,
                selector: BuffSelector::IdOrType(buff_id),
                amount: 1,
                depleted: DepletedBuff::Remove,
            },
        ))));
    }
    outputs.extend([
        RuleOp::Command(BattleCommand::Gauge(GaugeCommand::new(
            origin,
            key(subscriber.team_type),
            GaugeOperation::ConsumeAccumulated {
                listener_uid: subscriber.buff_uid,
                listener_opcode: subscriber.key.definition.opcode,
                amount: selected.threshold,
            },
        ))),
        RuleOp::BuffActInfoMarker(BuffActInfoMarkerResult {
            target_uid: subscriber.owner_uid,
            buff_uid: subscriber.buff_uid,
            act_id: subscriber.key.definition.opcode,
            params: vec![after],
            str_param: Some(String::new()),
            team_type: 0,
        }),
    ]);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: subscriber.owner_uid,
        skill_id: selected.skill_id,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    invocation.extra_skill_kind = skill_kind_from_is_extra(catalog.extra_kind(selected.skill_id));
    outputs.push(RuleOp::Skill(invocation));
    Some(LingeringGlowCast { cast, outputs })
}

impl LingeringGlowRuntime {
    pub fn register(&mut self, gauges: &GaugeManager, create: HeatScaleCreate) -> bool {
        if gauges.get(key(create.team)).is_none() || self.raw.contains_key(&create.team) {
            return false;
        }
        self.raw.insert(
            create.team,
            RawState {
                current: 0,
                max: if create.raw_amount != 0 {
                    create.raw_amount
                } else {
                    create.amount * 1000
                },
                source_buff_id: create.source_buff_id,
            },
        );
        true
    }

    pub fn apply_change(&mut self, change: GaugeChange, raw_delta: i32) -> bool {
        if change.key.kind != GaugeKind::LingeringGlow || change.kind != GaugeChangeKind::Value {
            return false;
        }
        let GaugeOwner::Team(team) = change.key.owner else {
            return false;
        };
        let Some(raw) = self.raw.get_mut(&team) else {
            return false;
        };
        raw.current = raw.current.saturating_add(raw_delta).clamp(0, raw.max);
        true
    }

    pub fn raw_value(&self, team: i32) -> i32 {
        self.raw
            .get(&team)
            .map(|state| state.current)
            .unwrap_or_default()
    }

    pub fn source_buff_id(&self, team: i32) -> i32 {
        self.raw
            .get(&team)
            .map(|state| state.source_buff_id)
            .unwrap_or_default()
    }

    pub fn decrement_counter_infos(
        &self,
        features: &[ActiveBuffFeature],
        team: i32,
    ) -> Vec<heat_scale::HeatScaleCounterInfo> {
        heat_scale::decr_counter_infos(self.raw_value(team), features, team)
    }
}

#[cfg(test)]
mod test;
