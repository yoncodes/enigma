use sonettobuf::{ActEffect, FightStep, MagicCircleInfo};

use crate::engine::{
    fight::versions::{HurtInfoWireLayout, RedealWireLayout},
    manager::{
        card::{CARD_PLAY_ORIGIN, CardChangeKind},
        eureka::EurekaChanges,
        ex_point::ExPointChanges,
        field::{FieldChange, FieldChangeKind},
        gauge::{GaugeChangeKind, GaugeKind, GaugeOwner},
        summon::{SummonApplyResult, SummonOperation, summoned_lane},
        upgrade::UpgradeOperation,
    },
    runtime::{
        change::BattleChange,
        record::{FrameItem, FrameOwner, RoundCue, SemanticFrame},
    },
};

use super::{card::CardPacket, effect::EffectPacket, normalize_effect_tree, step::StepPacket};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionError {
    Card(CardChangeKind),
    Gauge(GaugeKind),
    Field(FieldChangeKind),
    Upgrade,
    FightVersion(i32),
}

#[cfg(test)]
pub fn project(frames: &[SemanticFrame]) -> Result<Vec<FightStep>, ProjectionError> {
    project_with_reduce_hp(frames, true)
}

/// Projects committed semantic frames using the requested wire-version layout.
/// Version gates may change fields, never state, targets, UIDs, timing, or ordering.
pub fn project_for_version(
    frames: &[SemanticFrame],
    fight_version: i32,
) -> Result<Vec<FightStep>, ProjectionError> {
    let hurt_info_layout = crate::engine::fight::versions::hurt_info_wire_layout(fight_version)
        .ok_or(ProjectionError::FightVersion(fight_version))?;
    let redeal_layout = crate::engine::fight::versions::redeal_wire_layout(fight_version)
        .ok_or(ProjectionError::FightVersion(fight_version))?;
    project_frames(
        frames,
        crate::engine::fight::versions::writes_reduce_hp(fight_version),
        hurt_info_layout,
        redeal_layout,
    )
}

#[cfg(test)]
fn project_with_reduce_hp(
    frames: &[SemanticFrame],
    writes_reduce_hp: bool,
) -> Result<Vec<FightStep>, ProjectionError> {
    project_frames(
        frames,
        writes_reduce_hp,
        HurtInfoWireLayout::Version6,
        RedealWireLayout::Version6,
    )
}

fn project_frames(
    frames: &[SemanticFrame],
    writes_reduce_hp: bool,
    hurt_info_layout: HurtInfoWireLayout,
    redeal_layout: RedealWireLayout,
) -> Result<Vec<FightStep>, ProjectionError> {
    let frames = frames
        .iter()
        .map(|frame| project_frame(frame, writes_reduce_hp, hurt_info_layout, redeal_layout))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(frames.into_iter().flatten().collect())
}

fn project_frame(
    frame: &SemanticFrame,
    writes_reduce_hp: bool,
    hurt_info_layout: HurtInfoWireLayout,
    redeal_layout: RedealWireLayout,
) -> Result<Option<FightStep>, ProjectionError> {
    let effects = project_frame_items(
        &frame.items,
        writes_reduce_hp,
        hurt_info_layout,
        redeal_layout,
    )?;
    if effects.is_empty() {
        return Ok(None);
    }
    match &frame.owner {
        FrameOwner::Skill {
            source_uid,
            skill_id,
            card_index,
            target_uid,
        } => Ok(Some(normalize_framed_step(
            EffectPacket::skill_fight_step_with_card_index(
                *skill_id,
                *source_uid,
                target_uid.unwrap_or_default(),
                *card_index,
                effects,
            ),
        ))),
        FrameOwner::ConduitAction {
            source_uid,
            group,
            skill_position,
            target_uid,
        } => Ok(Some(normalize_framed_step(
            EffectPacket::conduit_fight_step(
                *source_uid,
                target_uid.unwrap_or_default(),
                *group,
                *skill_position,
                effects,
            ),
        ))),
        FrameOwner::ConduitStopped { source_uid, group } => Ok(Some(normalize_framed_step(
            EffectPacket::conduit_fight_step(*source_uid, 0, *group, 1, effects),
        ))),
        FrameOwner::BuffAct {
            owner_uid,
            source_uid,
            buff_id,
            ..
        } => Ok(Some(normalize_framed_step(
            EffectPacket::effect_fight_step_action(*source_uid, *owner_uid, *buff_id, effects),
        ))),
        FrameOwner::BuffRule { emitter_uid, .. } => Ok(Some(normalize_framed_step(
            EffectPacket::effect_fight_step_action(*emitter_uid, 0, 0, effects),
        ))),
        FrameOwner::EventEffect {
            source_uid,
            target_uid,
        } => Ok(Some(normalize_framed_step(
            EffectPacket::effect_fight_step_action(*source_uid, *target_uid, 0, effects),
        ))),
        FrameOwner::SetupBuffAct { .. } => Ok(StepPacket::effect(effects)),
        FrameOwner::SetupMechanic => Ok(StepPacket::effect(effects).map(|mut step| {
            step.fake_timeline = Some(true);
            step
        })),
        FrameOwner::SetupSide(_)
        | FrameOwner::SetupEntity { .. }
        | FrameOwner::StageWave { .. }
        | FrameOwner::EventRule
        | FrameOwner::RoundPhase(_)
        | FrameOwner::Command => Ok(StepPacket::effect(effects)),
    }
}

fn project_frame_items(
    items: &[FrameItem],
    writes_reduce_hp: bool,
    hurt_info_layout: HurtInfoWireLayout,
    redeal_layout: RedealWireLayout,
) -> Result<Vec<ActEffect>, ProjectionError> {
    let mut effects = Vec::new();
    for item in items {
        match item {
            FrameItem::Change(change) => effects.extend(project_change(
                change.as_ref(),
                writes_reduce_hp,
                hurt_info_layout,
                redeal_layout,
            )?),
            FrameItem::Child(frame) => effects.extend(project_child(
                frame,
                writes_reduce_hp,
                hurt_info_layout,
                redeal_layout,
            )?),
            FrameItem::Cue(cue) => effects.extend(project_cue(cue, redeal_layout)),
        }
    }
    Ok(effects)
}

fn project_child(
    frame: &SemanticFrame,
    writes_reduce_hp: bool,
    hurt_info_layout: HurtInfoWireLayout,
    redeal_layout: RedealWireLayout,
) -> Result<Option<ActEffect>, ProjectionError> {
    Ok(
        project_frame(frame, writes_reduce_hp, hurt_info_layout, redeal_layout)?
            .map(EffectPacket::from_fight_step),
    )
}

fn normalize_framed_step(effect: ActEffect) -> FightStep {
    normalize_effect_tree(effect)
        .fight_step
        .expect("a framed effect contains a fight step")
}

#[cfg(test)]
fn project_change_for_test(change: &BattleChange) -> Result<Vec<ActEffect>, ProjectionError> {
    project_change_with_reduce_hp(change, true)
}

#[cfg(test)]
fn project_change_with_reduce_hp(
    change: &BattleChange,
    writes_reduce_hp: bool,
) -> Result<Vec<ActEffect>, ProjectionError> {
    project_change(
        change,
        writes_reduce_hp,
        HurtInfoWireLayout::Version6,
        RedealWireLayout::Version6,
    )
}

fn project_change(
    change: &BattleChange,
    writes_reduce_hp: bool,
    hurt_info_layout: HurtInfoWireLayout,
    redeal_layout: RedealWireLayout,
) -> Result<Vec<ActEffect>, ProjectionError> {
    Ok(match change {
        BattleChange::SkillLifecycle(
            crate::engine::skill::action::SkillLifecycle::DirectUltimateBodyCompleted {
                source_uid,
            },
        ) => vec![EffectPacket::direct_use_ex_skill(*source_uid)],
        BattleChange::SkillLifecycle(
            crate::engine::skill::action::SkillLifecycle::EmitterAttackStarted(attack),
        ) => vec![EffectPacket::emitter_attack_marker(*attack)],
        BattleChange::SkillLifecycle(
            crate::engine::skill::action::SkillLifecycle::EmitterSkillEnded { .. },
        ) => vec![EffectPacket::emitter_skill_end()],
        BattleChange::SkillLifecycle(
            crate::engine::skill::action::SkillLifecycle::ActionCompleted(_),
        ) => Vec::new(),
        BattleChange::SkillLifecycle(
            crate::engine::skill::action::SkillLifecycle::PhaseCompleted(_),
        ) => Vec::new(),
        BattleChange::Buff(changes) => EffectPacket::recorded_buff_changes(changes),
        BattleChange::BuffFanout(fanout) => fanout
            .added
            .iter()
            .flat_map(EffectPacket::buff_add_direct)
            .chain(fanout.refreshed.iter().flat_map(|refreshed| {
                std::iter::once(EffectPacket::buff_update(&refreshed.update))
                    .chain(refreshed.markers.iter().map(EffectPacket::buff_marker))
            }))
            .collect(),
        BattleChange::RaspberryCapacity(result) => match result.as_ref() {
            crate::engine::skill::buff_act::raspberry::CapacityResult::Applied(changes) => {
                let Some(max_hp) = changes.hp.max_hp else {
                    return Ok(Vec::new());
                };
                vec![
                    EffectPacket::current_hp_change(max_hp.target_uid, max_hp.after_current),
                    EffectPacket::buff_act_info(
                        max_hp.target_uid,
                        changes.buff_uid,
                        changes.buff_act_id,
                        vec![changes.current, changes.cap],
                    ),
                    EffectPacket::max_hp_change(
                        max_hp.target_uid,
                        max_hp.after_max,
                        changes.buff_act_id,
                    ),
                ]
            }
            crate::engine::skill::buff_act::raspberry::CapacityResult::AtCap(sync) => vec![
                EffectPacket::buff_act_info(
                    sync.target_uid,
                    sync.buff_uid,
                    sync.buff_act_id,
                    vec![sync.current, sync.cap],
                ),
                EffectPacket::max_hp_change(sync.target_uid, sync.max_hp, sync.buff_act_id),
            ],
        },
        BattleChange::BuffFeatureMarker(marker) => vec![EffectPacket::buff_marker(marker)],
        BattleChange::EffectMarker(marker) => vec![EffectPacket::effect_marker(marker.clone())],
        BattleChange::SceneChange { scene_id } => EffectPacket::scene_change(*scene_id).to_vec(),
        BattleChange::BuffActTrigger(trigger) => {
            vec![EffectPacket::buff_act_trigger(*trigger)]
        }
        BattleChange::BuffActInfoMarker(marker) => {
            vec![EffectPacket::buff_act_info_with_team_and_str(
                marker.target_uid,
                marker.buff_uid,
                marker.act_id,
                marker.params.clone(),
                marker.str_param.clone().unwrap_or_default(),
                marker.team_type,
            )]
        }
        BattleChange::Hp(changes) => {
            if let Some(config_effect) = changes.kill {
                return Ok(vec![EffectPacket::kill(changes.target_uid, config_effect)]);
            }
            let mut effects = Vec::new();
            if let Some(shield) = changes
                .shield_absorbed
                .filter(|_| hurt_info_layout == HurtInfoWireLayout::Version6)
            {
                effects.push(EffectPacket::shield(
                    shield.target_uid,
                    shield.after,
                    changes
                        .damage
                        .map(|damage| damage.hurt.hurt_effect_type)
                        .unwrap_or_default(),
                ));
            }
            if let Some(shield) = changes.shield_granted {
                effects.push(EffectPacket::shield_value_change(
                    shield.target_uid,
                    shield.added,
                ));
            }
            if let Some(hp) = changes.hp {
                let hp = apply_hp_wire_layout(hp, writes_reduce_hp);
                let mut effect = hp
                    .hurt
                    .filter(|hurt| {
                        hurt.damage_from == crate::engine::manager::hp::HurtDamageFromType::Buff
                    })
                    .map(|hurt| {
                        EffectPacket::damage_by_buff_act_with_hurt_info_layout(
                            hp,
                            hurt.buff_act_id,
                            hurt_info_layout,
                        )
                    })
                    .unwrap_or_else(|| {
                        EffectPacket::hp_with_hurt_info_and_toughness_layout(
                            hp,
                            changes.toughness,
                            hurt_info_layout,
                        )
                    });
                apply_absorbed_shield_wire(
                    &mut effect,
                    changes.team_shared_shield_absorbed,
                    changes.shield_absorbed,
                    hurt_info_layout,
                );
                effects.push(effect);
            } else if let Some(damage) = changes.damage {
                let mut effect = EffectPacket::fully_absorbed_damage_with_toughness_layout(
                    changes.target_uid,
                    damage,
                    changes.toughness,
                    hurt_info_layout,
                );
                apply_absorbed_shield_wire(
                    &mut effect,
                    changes.team_shared_shield_absorbed,
                    changes.shield_absorbed,
                    hurt_info_layout,
                );
                effects.push(effect);
            }
            if let Some(removed) = &changes.team_shared_shield_removed {
                effects.extend(EffectPacket::recorded_buff_changes(removed));
            }
            if let Some(max_hp) = changes.max_hp {
                let wire = (changes.origin.domain
                    == crate::engine::skill::rule::RuleDomain::BuffAct)
                    .then(|| {
                        crate::engine::skill::buff_act::wire::find(
                            changes.origin.key.opcode,
                            changes.origin.key.type_name,
                        )
                        .and_then(|definition| definition.max_hp)
                    })
                    .flatten();
                for _ in 0..wire.map_or(1, |rule| rule.repeats) {
                    effects.push(EffectPacket::max_hp_change(
                        max_hp.target_uid,
                        max_hp.after_max,
                        wire.map_or(0, |rule| rule.buff_act_id),
                    ));
                    effects.push(EffectPacket::current_hp_change(
                        max_hp.target_uid,
                        max_hp.after_current,
                    ));
                }
            }
            if let Some(death) = changes.death {
                effects.push(EffectPacket::dead(death.target_uid));
            }
            effects
        }
        BattleChange::Death(death) => vec![EffectPacket::dead(death.target_uid)],
        BattleChange::NuoDiKaHit(hit) => vec![EffectPacket::nuo_di_ka_hit(*hit)],
        BattleChange::Injury(change) => {
            if change.counter_owner_uid == 0 || change.after <= change.before {
                Vec::new()
            } else {
                ((change.before + 1)..=change.after)
                    .map(|value| {
                        EffectPacket::fight_counter(
                            change.counter_owner_uid,
                            crate::engine::manager::injury::InjuryCounterKind::TeamInjury.id(),
                            value,
                            change.team_type,
                        )
                    })
                    .collect()
            }
        }
        BattleChange::Shield(changes) => {
            let shield = changes.hp.as_ref().and_then(|hp| hp.shield_granted);
            if let Some(buff) = &changes.buff {
                let mut effects = EffectPacket::recorded_buff_changes(buff);
                if let Some(shield) = shield
                    && let Some(marker) = effects.iter_mut().find(|effect| {
                        effect.effect_type
                            == Some(sonettobuf::effect_type_enum::EffectType::Shield as i32)
                    })
                {
                    marker.effect_num = Some(shield.after);
                }
                effects
            } else if let Some(shield) = changes.team_shared {
                vec![EffectPacket::buff_act_info(
                    shield.owner_uid,
                    shield.buff_uid,
                    shield.buff_act_id,
                    vec![shield.after],
                )]
            } else {
                shield
                    .map(|shield| {
                        EffectPacket::shield_value_change(shield.target_uid, shield.added)
                    })
                    .into_iter()
                    .collect()
            }
        }
        BattleChange::ExPoint(ExPointChanges::Value { change, .. })
            if change.applied_delta != 0 =>
        {
            vec![EffectPacket::ex_point(*change)]
        }
        BattleChange::ExPoint(ExPointChanges::Max { change, .. }) if change.applied_delta != 0 => {
            vec![EffectPacket::ex_point_max_add(
                change.target_uid,
                change.applied_delta,
            )]
        }
        BattleChange::ExPoint(_) => Vec::new(),
        BattleChange::Eureka(EurekaChanges::Changed { change, .. })
            if change.applied_delta != 0 =>
        {
            vec![EffectPacket::eureka(*change)]
        }
        BattleChange::Eureka(EurekaChanges::Max { change, .. }) => {
            vec![EffectPacket::eureka_max_add(
                change.target_uid,
                change.power_id,
                change.delta,
            )]
        }
        BattleChange::Eureka(_) => Vec::new(),
        BattleChange::Gauge(change)
            if change.key.kind == GaugeKind::TeamEnergy
                && matches!(change.key.owner, GaugeOwner::Team(_)) =>
        {
            let GaugeOwner::Team(team) = change.key.owner else {
                unreachable!()
            };
            (change.applied_delta != 0)
                .then(|| EffectPacket::team_energy_change(team, change.applied_delta))
                .into_iter()
                .collect()
        }
        BattleChange::Gauge(change) if change.key.kind == GaugeKind::ImpromptuInspiration => {
            (change.kind == GaugeChangeKind::Snapshot || change.applied_delta != 0)
                .then(|| {
                    EffectPacket::emitter_energy_change(
                        if change.kind == GaugeChangeKind::Snapshot {
                            change.after
                        } else {
                            change.applied_delta
                        },
                    )
                })
                .into_iter()
                .collect()
        }
        BattleChange::Emitter(change) => (!change.enabled_before && change.enabled_after)
            .then(EffectPacket::emitter_create)
            .into_iter()
            .collect(),
        BattleChange::Gauge(change)
            if change.key.kind == GaugeKind::Bloodtithe
                && matches!(change.key.owner, GaugeOwner::Team(_)) =>
        {
            let GaugeOwner::Team(team) = change.key.owner else {
                unreachable!()
            };
            match change.kind {
                GaugeChangeKind::Enabled if !change.enabled_before && change.enabled_after => vec![
                    EffectPacket::blood_pool_max_create(team, change.config_effect, 0),
                    EffectPacket::blood_pool_max_change(team, change.after_max.unwrap_or_default()),
                ],
                GaugeChangeKind::Max if change.applied_delta != 0 => {
                    vec![EffectPacket::blood_pool_max_change(
                        team,
                        change.applied_delta,
                    )]
                }
                GaugeChangeKind::Value if change.applied_delta != 0 => {
                    vec![EffectPacket::blood_pool_value_change(
                        change.source_uid,
                        team,
                        change.applied_delta,
                        change.config_effect,
                    )]
                }
                _ => Vec::new(),
            }
        }
        BattleChange::Gauge(change)
            if change.key.kind == GaugeKind::LingeringGlow
                && matches!(change.key.owner, GaugeOwner::Team(_)) =>
        {
            let GaugeOwner::Team(team) = change.key.owner else {
                unreachable!()
            };
            match change.kind {
                GaugeChangeKind::Enabled if !change.enabled_before && change.enabled_after => {
                    vec![EffectPacket::blood_pool_max_create(
                        team,
                        change.config_effect,
                        change.after_max.unwrap_or_default(),
                    )]
                }
                GaugeChangeKind::Value if change.applied_delta != 0 => {
                    vec![EffectPacket::blood_pool_value_change(
                        change.source_uid,
                        team,
                        change.applied_delta,
                        change.config_effect,
                    )]
                }
                _ => Vec::new(),
            }
        }
        BattleChange::Gauge(change) => return Err(ProjectionError::Gauge(change.key.kind)),
        BattleChange::NuoDiKa(change) => vec![EffectPacket::nuo_di_ka_channel(*change)],
        BattleChange::Conduit(crate::engine::manager::conduit::ConduitChange::Initialized(
            area,
        )) => vec![EffectPacket::conduit_initialized(area)],
        BattleChange::Conduit(crate::engine::manager::conduit::ConduitChange::GroupSelected {
            ..
        }) => Vec::new(),
        BattleChange::Conduit(
            crate::engine::manager::conduit::ConduitChange::SkillGroupChanged {
                origin,
                source_uid,
                team,
                group,
            },
        ) => vec![EffectPacket::conduit_group_selected(
            *source_uid,
            *team,
            *group,
            origin.key.opcode,
        )],
        BattleChange::Conduit(crate::engine::manager::conduit::ConduitChange::SkillBegan {
            team,
            power_id,
            spent,
            ..
        }) if *spent > 0 => vec![EffectPacket::conduit_skill_began(*team, *power_id, *spent)],
        BattleChange::Conduit(crate::engine::manager::conduit::ConduitChange::SkillBegan {
            ..
        }) => Vec::new(),
        BattleChange::Conduit(
            crate::engine::manager::conduit::ConduitChange::SkillCostCommitted {
                source_uid,
                team,
                activation_cost,
                consumed_this_round,
                ..
            },
        ) if *activation_cost > 0 => vec![EffectPacket::conduit_skill_cost_committed(
            *source_uid,
            *team,
            *consumed_this_round,
        )],
        BattleChange::Conduit(
            crate::engine::manager::conduit::ConduitChange::SkillCostCommitted { .. },
        ) => Vec::new(),
        BattleChange::Conduit(crate::engine::manager::conduit::ConduitChange::SkillFinished {
            source_uid,
            team,
            uses_this_round,
            ..
        }) => vec![EffectPacket::conduit_skill_finished(
            *source_uid,
            *team,
            *uses_this_round,
        )],
        BattleChange::Conduit(
            crate::engine::manager::conduit::ConduitChange::ActivationCompleted(_),
        ) => Vec::new(),
        BattleChange::Conduit(crate::engine::manager::conduit::ConduitChange::RunningChanged {
            running,
            ..
        }) => vec![EffectPacket::conduit_running(*running)],
        BattleChange::Conduit(crate::engine::manager::conduit::ConduitChange::PowerChanged {
            team,
            power_id,
            applied_delta,
            kind,
            ..
        }) => vec![EffectPacket::conduit_power_changed(
            *team,
            *power_id,
            *applied_delta,
            *kind,
        )],
        BattleChange::Conduit(crate::engine::manager::conduit::ConduitChange::PowersCleared {
            origin,
            source_uid,
            team,
            ..
        }) => vec![EffectPacket::conduit_powers_cleared(
            *source_uid,
            *team,
            origin.key.opcode,
        )],
        BattleChange::Conduit(crate::engine::manager::conduit::ConduitChange::PowersReset {
            team,
        }) => vec![EffectPacket::conduit_powers_reset(*team)],
        BattleChange::Conduit(crate::engine::manager::conduit::ConduitChange::SkillStopped {
            source_uid,
            team,
            skill_id,
            ..
        }) => vec![EffectPacket::conduit_skill_stopped(
            *source_uid,
            *team,
            *skill_id,
        )],
        BattleChange::Conduit(
            crate::engine::manager::conduit::ConduitChange::DeviceRestarted { source_uid, team },
        ) => vec![EffectPacket::conduit_device_restarted(*source_uid, *team)],
        BattleChange::Card(changes) if changes.kind == CardChangeKind::Setup => Vec::new(),
        BattleChange::Card(changes) if changes.kind == CardChangeKind::AiQueueRefreshed => {
            Vec::new()
        }
        BattleChange::Card(changes) if changes.kind == CardChangeKind::AiQueueSet => Vec::new(),
        BattleChange::Card(changes) if changes.kind == CardChangeKind::TeamCardsSet => Vec::new(),
        BattleChange::Card(changes) if changes.kind == CardChangeKind::OwnerSkillsReplaced => {
            Vec::new()
        }
        BattleChange::Card(changes) if changes.kind == CardChangeKind::AiOwnerRemoved => changes
            .owner_removal
            .map(|removal| EffectPacket::remove_entity_cards(removal.owner_uid, removal.team_type))
            .into_iter()
            .collect(),
        BattleChange::Card(changes) if changes.kind == CardChangeKind::Moved => Vec::new(),
        BattleChange::Card(changes) if changes.kind == CardChangeKind::Dissolved => changes
            .operation
            .clone()
            .map(CardPacket::from_change)
            .into_iter()
            .collect(),
        BattleChange::Card(changes)
            if changes.kind == CardChangeKind::Composed
                && changes.origin == Some(CARD_PLAY_ORIGIN) =>
        {
            Vec::new()
        }
        BattleChange::Card(changes) if changes.kind == CardChangeKind::Composed => {
            (!changes.composed_owners.is_empty())
                .then(|| CardPacket::cards_compose(Vec::new()))
                .into_iter()
                .collect()
        }
        BattleChange::Card(changes) if changes.kind == CardChangeKind::Played => Vec::new(),
        BattleChange::Card(changes)
            if matches!(
                changes.kind,
                CardChangeKind::CastChannelRecorded
                    | CardChangeKind::CastChannelResolved
                    | CardChangeKind::CastChannelRemoved
            ) =>
        {
            Vec::new()
        }
        BattleChange::Card(changes) if changes.kind == CardChangeKind::QueuedRankChanged => {
            Vec::new()
        }
        BattleChange::Card(changes) if changes.kind == CardChangeKind::HandRankChanged => {
            let entity = changes
                .entity
                .clone()
                .ok_or(ProjectionError::Card(changes.kind))?;
            let config_effect = changes
                .origin
                .map(|origin| origin.key.opcode)
                .ok_or(ProjectionError::Card(changes.kind))?;
            changes
                .rank_results
                .iter()
                .filter_map(|result| match result {
                    crate::engine::manager::card::CardRankResult::Changed(change) => Some(
                        CardPacket::hand_rank_change(change, entity.clone(), config_effect),
                    ),
                    crate::engine::manager::card::CardRankResult::Failed(_) => None,
                })
                .collect()
        }
        BattleChange::Card(changes) if changes.kind == CardChangeKind::AroundRanksChanged => {
            changes
                .rank_results
                .iter()
                .map(|result| match result {
                    crate::engine::manager::card::CardRankResult::Changed(change) => {
                        CardPacket::play_around_rank_change(change)
                    }
                    crate::engine::manager::card::CardRankResult::Failed(failure) => {
                        CardPacket::play_around_rank_failure(failure)
                    }
                })
                .collect()
        }
        BattleChange::Card(changes) if changes.kind == CardChangeKind::PlayedRanksResolved => {
            changes
                .rank_results
                .iter()
                .filter_map(|result| match result {
                    crate::engine::manager::card::CardRankResult::Changed(change) => {
                        Some(CardPacket::play_around_rank_change(change))
                    }
                    crate::engine::manager::card::CardRankResult::Failed(_) => None,
                })
                .collect()
        }
        BattleChange::Card(changes) if changes.kind == CardChangeKind::ActionQueueCommitted => {
            let queue = changes
                .action_queue
                .as_ref()
                .expect("action-queue commits retain their historical snapshots");
            vec![
                CardPacket::use_cards(queue.cards.clone()),
                CardPacket::hand_after_use_cards(changes.after.clone()),
                CardPacket::card_deck_num(queue.deck_num, queue.team),
            ]
        }
        BattleChange::Card(changes) if changes.kind == CardChangeKind::UseCardQueued => {
            let queued = changes
                .queued_use_card
                .as_ref()
                .expect("queued use-card changes retain their historical card");
            vec![CardPacket::add_use_card(
                queued.card_index,
                queued.card.clone(),
                queued.source_skill_id,
            )]
        }
        BattleChange::Card(changes) if changes.kind == CardChangeKind::EnergyAllocated => {
            vec![CardPacket::allocate_card_energy(changes.after.clone(), 1)]
        }
        BattleChange::Card(changes) if changes.kind == CardChangeKind::EnergyChanged => {
            vec![CardPacket::allocate_card_energy(changes.after.clone(), 1)]
        }
        BattleChange::Card(changes) if changes.kind == CardChangeKind::EnergyCleared => {
            vec![CardPacket::clear_card_energy(changes.after.clone(), 1)]
        }
        BattleChange::Card(changes) if changes.kind == CardChangeKind::HandLimitChanged => {
            let (target_uid, limit) = changes
                .hand_limit
                .ok_or(ProjectionError::Card(changes.kind))?;
            let config_effect = changes
                .origin
                .map(|origin| origin.key.opcode)
                .ok_or(ProjectionError::Card(changes.kind))?;
            vec![EffectPacket::card_hand_limit(
                target_uid,
                limit,
                config_effect,
            )]
        }
        BattleChange::Card(changes) if changes.kind == CardChangeKind::HandLimitCleared => {
            Vec::new()
        }
        BattleChange::Card(changes) if changes.kind == CardChangeKind::ConsumedForEffect => {
            vec![EffectPacket::card_remove(&changes.consumed_indices)]
        }
        BattleChange::Card(changes) if changes.kind == CardChangeKind::GeneratedAdded => changes
            .operation
            .clone()
            .map(CardPacket::from_change)
            .into_iter()
            .collect(),
        BattleChange::Card(changes) if changes.kind == CardChangeKind::UniversalAdded => changes
            .after
            .iter()
            .skip(changes.before.len())
            .filter_map(|card| card.skill_id)
            .map(CardPacket::universal_card)
            .collect(),
        BattleChange::Card(changes) if changes.kind == CardChangeKind::RedealtKeepRanks => {
            vec![CardPacket::redeal_keep_ranks(
                changes.after.clone(),
                changes
                    .origin
                    .map(|origin| origin.key.opcode)
                    .unwrap_or_default(),
                redeal_layout,
            )]
        }
        BattleChange::Card(changes)
            if matches!(
                changes.kind,
                CardChangeKind::TemporaryAdded | CardChangeKind::TemporaryChanged
            ) =>
        {
            let operation = changes
                .operation
                .clone()
                .expect("temporary-card commits retain their exact operation");
            let mut effects = vec![CardPacket::from_change(operation.clone())];
            if changes.kind == CardChangeKind::TemporaryAdded
                && let crate::engine::manager::card::CardChange::SpCardAdd {
                    target_uid,
                    team_type,
                    ..
                } = operation
            {
                effects.push(CardPacket::change_to_temp_card(
                    target_uid,
                    changes.after.len().to_string(),
                    team_type,
                ));
            }
            effects
        }
        BattleChange::Card(changes) if changes.kind == CardChangeKind::Enchanted => changes
            .operation
            .clone()
            .map(CardPacket::from_change)
            .into_iter()
            .collect(),
        BattleChange::Card(changes) if changes.kind == CardChangeKind::TemporaryExpired => {
            Vec::new()
        }
        BattleChange::Card(changes) if changes.kind == CardChangeKind::Drawn => {
            vec![CardPacket::cards_push(changes.after.clone(), 1)]
        }
        BattleChange::Card(changes) if changes.kind == CardChangeKind::CrystalAdded => changes
            .added
            .clone()
            .map(EffectPacket::crystal_add_card)
            .into_iter()
            .collect(),
        BattleChange::Card(changes) if changes.kind == CardChangeKind::PrecastAdded => changes
            .added
            .clone()
            .map(EffectPacket::butterfly_add_hand_card)
            .into_iter()
            .collect(),
        BattleChange::Card(changes) if changes.kind == CardChangeKind::Refilled => Vec::new(),
        BattleChange::Card(changes) if changes.kind == CardChangeKind::PlayedInvalidated => changes
            .operation
            .clone()
            .map(CardPacket::from_change)
            .into_iter()
            .collect(),
        BattleChange::Card(changes) => return Err(ProjectionError::Card(changes.kind)),
        BattleChange::Field(change) if change.before == change.after => Vec::new(),
        BattleChange::Field(change)
            if matches!(
                change.kind,
                FieldChangeKind::Deployed
                    | FieldChangeKind::Progress
                    | FieldChangeKind::Duration
                    | FieldChangeKind::Level
            ) =>
        {
            let Some(applied) = magic_circle_snapshot(change) else {
                return Err(ProjectionError::Field(change.kind));
            };
            let mut effect = match change.kind {
                FieldChangeKind::Deployed => EffectPacket::magic_circle_add(&applied),
                FieldChangeKind::Level => EffectPacket::magic_circle_upgrade(&applied),
                FieldChangeKind::Progress | FieldChangeKind::Duration => {
                    EffectPacket::magic_circle_update(&applied)
                }
                FieldChangeKind::Removed => unreachable!(),
            };
            if change.kind == FieldChangeKind::Duration {
                effect.reserve_str = Some(change.applied_delta.to_string());
            }
            vec![effect]
        }
        BattleChange::Field(change) => return Err(ProjectionError::Field(change.kind)),
        BattleChange::Summon(changes) => {
            let applied = |level| SummonApplyResult {
                target_uid: changes.target_uid,
                summoned_id: changes.summoned_id,
                level,
                uid: summoned_lane(changes.summoned_id) as i64,
                from_uid: changes.owner_uid,
            };
            match changes.operation {
                SummonOperation::Add { .. } if changes.before.is_none() => changes
                    .after
                    .map(|state| EffectPacket::summoned_add(&applied(state.level)))
                    .into_iter()
                    .collect(),
                SummonOperation::ChangeLevel { level } if changes.before != changes.after => {
                    vec![EffectPacket::summoned_level_up(
                        &applied(changes.after.map(|state| state.level).unwrap_or_default()),
                        level,
                        changes.origin.key.opcode,
                    )]
                }
                SummonOperation::AddLevel { delta } if changes.before != changes.after => {
                    vec![EffectPacket::summoned_level_up(
                        &applied(changes.after.map(|state| state.level).unwrap_or_default()),
                        delta,
                        changes.origin.key.opcode,
                    )]
                }
                SummonOperation::Remove { count } if changes.before != changes.after => {
                    vec![EffectPacket::summoned_delete(
                        changes.owner_uid,
                        changes.summoned_id,
                        count,
                        changes.origin.key.opcode,
                    )]
                }
                SummonOperation::ChangeLevel { .. }
                | SummonOperation::AddLevel { .. }
                | SummonOperation::Remove { .. }
                | SummonOperation::Add { .. } => Vec::new(),
            }
        }
        BattleChange::Entity(changes) => match changes.operation {
            crate::engine::manager::entity::EntityOperation::SummonCombatant { .. }
            | crate::engine::manager::entity::EntityOperation::SummonSpecial { .. } => {
                vec![EffectPacket::monster_summon(
                    changes.target_uid,
                    changes.entity.clone(),
                    changes.origin.key.opcode,
                )]
            }
            crate::engine::manager::entity::EntityOperation::Transform { .. } => {
                vec![EffectPacket::monster_change(
                    changes.entity.clone(),
                    changes.origin.key.opcode,
                )]
            }
        },
        BattleChange::ToughnessRecovered(change) => {
            vec![EffectPacket::toughness_recover(*change)]
        }
        BattleChange::Upgrade(change)
            if matches!(change.operation, UpgradeOperation::Offer { .. }) =>
        {
            let UpgradeOperation::Offer { origin, upgrade_id } = change.operation else {
                unreachable!()
            };
            vec![EffectPacket::notify_upgrade_hero(
                change.owner_uid,
                upgrade_id,
                origin.key.opcode,
            )]
        }
        BattleChange::Upgrade(_) => return Err(ProjectionError::Upgrade),
        BattleChange::UpgradeApplied(applied) => {
            let option_id = applied
                .change
                .selected_after
                .ok_or(ProjectionError::Upgrade)?;
            let mut effects = vec![EffectPacket::hero_upgrade(
                applied.change.owner_uid,
                option_id,
                applied.entity.clone(),
            )];
            effects.extend(
                applied
                    .buff_changes
                    .iter()
                    .flat_map(EffectPacket::recorded_buff_changes),
            );
            effects.push(CardPacket::cards_push(
                applied.card_changes.after.clone(),
                1,
            ));
            effects
        }
        BattleChange::EntityPromotion(promotion) => vec![EffectPacket::change_hero(
            promotion.defeated_uid,
            promotion.position,
            promotion.entering.clone(),
        )],
        BattleChange::WaveAdvanced(change) => {
            vec![EffectPacket::new_change_wave(change.fight.clone())]
        }
    })
}

fn apply_absorbed_shield_wire(
    effect: &mut ActEffect,
    team_shared: Option<crate::engine::manager::hp::TeamSharedShieldAbsorption>,
    shield: Option<crate::engine::manager::hp::ShieldChange>,
    layout: HurtInfoWireLayout,
) {
    if layout != HurtInfoWireLayout::Version7 || (team_shared.is_none() && shield.is_none()) {
        return;
    }
    let Some(hurt) = &mut effect.hurt_info else {
        return;
    };
    let team_shared = team_shared
        .map(|change| format!("{}#{}", change.buff_uid, change.consumed))
        .unwrap_or_default();
    let shield = shield
        .map(|change| format!("{}#{}", change.buff_uid, change.absorbed))
        .unwrap_or_default();
    hurt.absorb_hurt_param = Some(format!(
        r#"{{"consumeFakeHpBuffMap":"","reduceTeamShareShieldBuffMap":"{team_shared}","reduceShieldBuffMap":"{shield}"}}"#
    ));
}

fn apply_hp_wire_layout(
    mut change: crate::engine::manager::hp::HpChange,
    writes_reduce_hp: bool,
) -> crate::engine::manager::hp::HpChange {
    if !writes_reduce_hp && let Some(hurt) = &mut change.hurt {
        hurt.reduce_hp = 0;
    }
    change
}

fn magic_circle_snapshot(
    change: &FieldChange,
) -> Option<crate::engine::mechanic::magic_circle::MagicCircleApplyResult> {
    let state = change.after?;
    Some(
        crate::engine::mechanic::magic_circle::MagicCircleApplyResult {
            target_uid: state.create_uid,
            circle_id: state.definition.field_id,
            info: MagicCircleInfo {
                magic_circle_id: Some(state.definition.field_id),
                round: Some(state.definition.duration),
                create_uid: Some(state.create_uid),
                electric_level: Some(state.level),
                electric_progress: Some(state.progress),
                max_electric_progress: Some(state.next_upgrade_progress),
            },
        },
    )
}

fn project_cue(cue: &RoundCue, redeal_layout: RedealWireLayout) -> Vec<ActEffect> {
    match cue {
        RoundCue::EnterFightDeal => vec![CardPacket::enter_fight_deal()],
        RoundCue::ClearUniversalCard => vec![EffectPacket::clear_universal_card()],
        RoundCue::DealCard1 => vec![CardPacket::deal_card1()],
        RoundCue::LayerHaloSync { buffs } => {
            buffs.iter().map(EffectPacket::layer_halo_sync).collect()
        }
        RoundCue::NextRoundCards {
            cards,
            deck_count,
            team_type,
        } => vec![
            CardPacket::next_round_cards(cards.clone()),
            CardPacket::card_deck_num(*deck_count, *team_type),
        ],
        RoundCue::DeckCount { count, team_type } => {
            vec![CardPacket::card_deck_num(*count, *team_type)]
        }
        RoundCue::DealCards { team_type } => EffectPacket::round_end_deal(*team_type),
        RoundCue::CardInvalid {
            card_index,
            team_type,
            reason,
        } => vec![CardPacket::card_invalid(
            *card_index,
            *team_type,
            reason.config_effect(),
        )],
        RoundCue::CardsCompose { .. } => vec![CardPacket::cards_compose(Vec::new())],
        RoundCue::RedealHandSync { cards } => match redeal_layout {
            RedealWireLayout::Version6 => Vec::new(),
            RedealWireLayout::Version7 => vec![CardPacket::redeal_hand_sync(cards.clone())],
        },
        RoundCue::SmallRoundEnd { team_type } => {
            vec![EffectPacket::small_round_end(*team_type)]
        }
        RoundCue::ChangeRound { round } => vec![EffectPacket::change_round(*round)],
    }
}

#[cfg(test)]
mod test;
