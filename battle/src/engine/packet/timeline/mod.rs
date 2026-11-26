mod card;
mod conduit;
mod gauge;
mod hp;

use sonettobuf::effect_type_enum::EffectType;
use sonettobuf::{ActEffect, FightStep, MagicCircleInfo};

use crate::engine::{
    fight::versions::{
        AbsorbHurtMapLayout, ChangeRoundWireLayout, HurtInfoWireLayout, RedealWireLayout,
    },
    manager::{
        card::CardChangeKind,
        eureka::EurekaChanges,
        ex_point::ExPointChanges,
        field::{FieldChange, FieldChangeKind},
        gauge::GaugeKind,
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
    project_for_version_with_absorb_map_layout(
        frames,
        fight_version,
        AbsorbHurtMapLayout::default(),
    )
}

pub(crate) fn project_for_version_with_absorb_map_layout(
    frames: &[SemanticFrame],
    fight_version: i32,
    absorb_map_layout: AbsorbHurtMapLayout,
) -> Result<Vec<FightStep>, ProjectionError> {
    let hurt_info_layout = crate::engine::fight::versions::hurt_info_wire_layout(fight_version)
        .ok_or(ProjectionError::FightVersion(fight_version))?;
    let redeal_layout = crate::engine::fight::versions::redeal_wire_layout(fight_version)
        .ok_or(ProjectionError::FightVersion(fight_version))?;
    let change_round_layout =
        crate::engine::fight::versions::change_round_wire_layout(fight_version)
            .ok_or(ProjectionError::FightVersion(fight_version))?;
    project_frames(
        frames,
        crate::engine::fight::versions::writes_reduce_hp(fight_version),
        hurt_info_layout,
        absorb_map_layout,
        redeal_layout,
        change_round_layout,
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
        AbsorbHurtMapLayout::default(),
        RedealWireLayout::Version6,
        ChangeRoundWireLayout::Incremental,
    )
}

fn project_frames(
    frames: &[SemanticFrame],
    writes_reduce_hp: bool,
    hurt_info_layout: HurtInfoWireLayout,
    absorb_map_layout: AbsorbHurtMapLayout,
    redeal_layout: RedealWireLayout,
    change_round_layout: ChangeRoundWireLayout,
) -> Result<Vec<FightStep>, ProjectionError> {
    let mut steps = Vec::new();
    for frame in frames {
        if let Some(step) = project_frame(
            frame,
            writes_reduce_hp,
            hurt_info_layout,
            absorb_map_layout,
            redeal_layout,
            change_round_layout,
        )? {
            if frame.owner == FrameOwner::ConduitSelectionCommit {
                steps.push(step.clone());
            }
            steps.push(step);
        }
    }
    Ok(steps)
}

fn project_frame(
    frame: &SemanticFrame,
    writes_reduce_hp: bool,
    hurt_info_layout: HurtInfoWireLayout,
    absorb_map_layout: AbsorbHurtMapLayout,
    redeal_layout: RedealWireLayout,
    change_round_layout: ChangeRoundWireLayout,
) -> Result<Option<FightStep>, ProjectionError> {
    let effects = project_frame_items(
        &frame.items,
        writes_reduce_hp,
        hurt_info_layout,
        absorb_map_layout,
        redeal_layout,
        change_round_layout,
    )?;
    if effects.is_empty()
        && !frame
            .items
            .iter()
            .any(|item| matches!(item, FrameItem::Cue(RoundCue::CastStep)))
    {
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
        FrameOwner::ConduitSkill {
            source_uid,
            skill_id,
            device_index,
            target_uid,
        } => Ok(Some(normalize_framed_step(
            EffectPacket::conduit_skill_fight_step(
                *skill_id,
                *source_uid,
                target_uid.unwrap_or_default(),
                *device_index,
                effects,
            ),
        ))),
        FrameOwner::ConduitAction {
            source_uid,
            selected_group: _,
            device_index,
            skill_position,
            target_uid,
        } => Ok(Some(normalize_framed_step(
            EffectPacket::conduit_fight_step(
                *source_uid,
                target_uid.unwrap_or_default(),
                *device_index,
                *skill_position,
                effects,
            ),
        ))),
        FrameOwner::ConduitStopped {
            source_uid,
            device_index,
            skill_position,
        } => Ok(Some(normalize_framed_step(
            EffectPacket::conduit_fight_step(
                *source_uid,
                0,
                *device_index,
                *skill_position,
                effects,
            ),
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
        FrameOwner::SetupMechanic | FrameOwner::LifecycleMechanic => {
            Ok(StepPacket::effect(effects).map(|mut step| {
                step.fake_timeline = Some(true);
                step
            }))
        }
        FrameOwner::BuffDuration { .. } => Ok(StepPacket::effect(effects)),
        FrameOwner::SetupSide(_)
        | FrameOwner::SetupEntity { .. }
        | FrameOwner::StageWave { .. }
        | FrameOwner::EventRule
        | FrameOwner::InitialSovereignBoost
        | FrameOwner::RoundPhase(_)
        | FrameOwner::PlayerActionCommit
        | FrameOwner::ConduitSelectionCommit
        | FrameOwner::SynchronizationCardState { .. }
        | FrameOwner::Command => Ok(StepPacket::effect(effects)),
    }
}

fn project_frame_items(
    items: &[FrameItem],
    writes_reduce_hp: bool,
    hurt_info_layout: HurtInfoWireLayout,
    absorb_map_layout: AbsorbHurtMapLayout,
    redeal_layout: RedealWireLayout,
    change_round_layout: ChangeRoundWireLayout,
) -> Result<Vec<ActEffect>, ProjectionError> {
    let mut effects = Vec::new();
    for item in items {
        match item {
            FrameItem::Change(change) => effects.extend(project_change(
                change.as_ref(),
                writes_reduce_hp,
                hurt_info_layout,
                absorb_map_layout,
                redeal_layout,
            )?),
            FrameItem::Child(frame) => {
                if matches!(frame.owner, FrameOwner::BuffDuration { .. }) {
                    effects.extend(project_frame_items(
                        &frame.items,
                        writes_reduce_hp,
                        hurt_info_layout,
                        absorb_map_layout,
                        redeal_layout,
                        change_round_layout,
                    )?);
                } else {
                    effects.extend(project_child(
                        frame,
                        writes_reduce_hp,
                        hurt_info_layout,
                        absorb_map_layout,
                        redeal_layout,
                        change_round_layout,
                    )?);
                }
            }
            FrameItem::Cue(cue) => {
                effects.extend(project_cue(cue, redeal_layout, change_round_layout))
            }
        }
    }
    Ok(effects)
}

fn project_child(
    frame: &SemanticFrame,
    writes_reduce_hp: bool,
    hurt_info_layout: HurtInfoWireLayout,
    absorb_map_layout: AbsorbHurtMapLayout,
    redeal_layout: RedealWireLayout,
    change_round_layout: ChangeRoundWireLayout,
) -> Result<Option<ActEffect>, ProjectionError> {
    Ok(project_frame(
        frame,
        writes_reduce_hp,
        hurt_info_layout,
        absorb_map_layout,
        redeal_layout,
        change_round_layout,
    )?
    .map(EffectPacket::from_fight_step))
}

fn normalize_framed_step(effect: ActEffect) -> FightStep {
    normalize_effect_tree(effect)
        .fight_step
        .expect("a framed effect contains a fight step")
}

#[cfg(test)]
pub(crate) fn project_change_for_test(
    change: &BattleChange,
) -> Result<Vec<ActEffect>, ProjectionError> {
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
        AbsorbHurtMapLayout::default(),
        RedealWireLayout::Version6,
    )
}

fn project_change(
    change: &BattleChange,
    writes_reduce_hp: bool,
    hurt_info_layout: HurtInfoWireLayout,
    absorb_map_layout: AbsorbHurtMapLayout,
    redeal_layout: RedealWireLayout,
) -> Result<Vec<ActEffect>, ProjectionError> {
    Ok(match change {
        BattleChange::UltimateDelegated { caster_uid } => {
            vec![EffectPacket::direct_use_ex_skill(*caster_uid)]
        }
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
            .chain(fanout.allies.iter().flat_map(layer_halo_ally_wire))
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
        BattleChange::CopyPreview { target_uid } => vec![EffectPacket::copy_preview(*target_uid)],
        BattleChange::SceneChange { scene_id } => EffectPacket::scene_change(*scene_id).to_vec(),
        BattleChange::Contract(crate::engine::manager::contract::ContractChange::Offered {
            origin,
            owner_uid,
            candidates,
        }) => vec![EffectPacket::contract_offer(
            *owner_uid,
            origin.key.opcode,
            candidates,
        )],
        BattleChange::Contract(
            crate::engine::manager::contract::ContractChange::OwnerSelected { owner_uid, .. },
        ) => vec![EffectPacket::contract_owner(*owner_uid)],
        BattleChange::Contract(
            crate::engine::manager::contract::ContractChange::BoundSelected { bound_uid, .. },
        ) => vec![EffectPacket::contract_bound(*bound_uid)],
        BattleChange::Contract(crate::engine::manager::contract::ContractChange::Cleared {
            ..
        }) => Vec::new(),
        BattleChange::FightProgress(change) => {
            EffectPacket::fight_progress_initialized(change.state).to_vec()
        }
        BattleChange::FightProgressValue(change) => {
            vec![EffectPacket::fight_progress_changed(*change)]
        }
        BattleChange::LegacyProgressDelta(change) => {
            vec![EffectPacket::legacy_progress_changed(*change)]
        }
        BattleChange::FightTask(change) => vec![EffectPacket::fight_task_update(change)],
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
        BattleChange::CrystalSelected(choice) => vec![EffectPacket::crystal_selected(*choice)],
        BattleChange::Hp(changes) => hp::project_hp(
            changes,
            writes_reduce_hp,
            hurt_info_layout,
            absorb_map_layout,
        ),
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
        BattleChange::Shield(changes) => hp::project_shield(changes),
        BattleChange::ExPoint(ExPointChanges::Value { change, .. })
            if change.applied_delta != 0 =>
        {
            vec![EffectPacket::ex_point(*change)]
        }
        BattleChange::ExPoint(ExPointChanges::Transfer { loss, gain, .. }) => [loss, gain]
            .into_iter()
            .filter(|change| change.applied_delta != 0)
            .map(|change| EffectPacket::ex_point(*change))
            .collect(),
        BattleChange::ExPoint(ExPointChanges::Resisted {
            target_uid,
            resistance_id,
        }) => vec![EffectPacket::ex_point_resisted(*target_uid, *resistance_id)],
        BattleChange::ExPoint(ExPointChanges::Max { change, .. }) if change.applied_delta != 0 => {
            vec![EffectPacket::ex_point_max(*change)]
        }
        BattleChange::ExPoint(ExPointChanges::SynchronizationDamageRecorded {
            action_target_uid,
            damage,
            ..
        }) => vec![EffectPacket::synchronization_damage(
            *action_target_uid,
            *damage,
            hurt_info_layout,
            absorb_map_layout,
        )],
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
        BattleChange::Gauge(change) => gauge::project_gauge(change)?,
        BattleChange::Emitter(change) => (!change.enabled_before && change.enabled_after)
            .then(EffectPacket::emitter_create)
            .into_iter()
            .collect(),
        BattleChange::NuoDiKa(change) => vec![EffectPacket::nuo_di_ka_channel(*change)],
        BattleChange::Conduit(change) => conduit::project_conduit(change),
        BattleChange::Card(changes) => card::project_card(changes, redeal_layout)?,
        BattleChange::CardStateSnapshot(snapshot) => {
            card::project_snapshot(snapshot, redeal_layout)
        }
        BattleChange::ActionQueueCardState(snapshot) => vec![
            CardPacket::hand_after_use_cards(snapshot.hand.clone()),
            CardPacket::card_deck_num(snapshot.deck_num, snapshot.team_type),
        ],
        BattleChange::PlayerActionQueue(queue) => vec![CardPacket::use_cards(&queue.actions)],
        BattleChange::Field(change) if change.before == change.after => Vec::new(),
        BattleChange::Field(change)
            if matches!(
                change.kind,
                FieldChangeKind::Deployed
                    | FieldChangeKind::Progress
                    | FieldChangeKind::Duration
                    | FieldChangeKind::Level
                    | FieldChangeKind::Removed
            ) =>
        {
            let mut effect = match change.kind {
                FieldChangeKind::Removed => change
                    .before
                    .map(|state| {
                        EffectPacket::magic_circle_delete(
                            state.owner_uid,
                            state.definition.field_id,
                        )
                    })
                    .ok_or(ProjectionError::Field(change.kind))?,
                kind => {
                    let applied =
                        magic_circle_snapshot(change).ok_or(ProjectionError::Field(change.kind))?;
                    match kind {
                        FieldChangeKind::Deployed => EffectPacket::magic_circle_add(&applied),
                        FieldChangeKind::Level => EffectPacket::magic_circle_upgrade(&applied),
                        FieldChangeKind::Progress | FieldChangeKind::Duration => {
                            EffectPacket::magic_circle_update(&applied)
                        }
                        FieldChangeKind::Removed => unreachable!(),
                    }
                }
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
            crate::engine::manager::entity::EntityOperation::ChangeCareer { career } => {
                vec![EffectPacket::career_change(
                    changes.target_uid,
                    career,
                    changes.origin.key.opcode,
                )]
            }
        },
        BattleChange::MoveFront(changes) => vec![EffectPacket::move_front(changes)],
        BattleChange::ToughnessReset(change) => {
            vec![EffectPacket::toughness_reset(*change)]
        }
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
                    .removed_buffs
                    .iter()
                    .chain(&applied.granted_buffs)
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
        BattleChange::ReserveQueued(queued) => vec![EffectPacket::call_monster_to_sub(
            queued.team_type,
            queued.entity.clone(),
        )],
        BattleChange::ReserveRetired(retired) => vec![EffectPacket::remove_monster_sub(
            retired.uid,
            retired.team_type,
        )],
        BattleChange::ReservesCleared(cleared) => vec![EffectPacket::clear_monster_sub(
            cleared.team_type,
            &cleared.uids,
        )],
        BattleChange::WaveAdvanced(change) => {
            vec![EffectPacket::new_change_wave(change.fight.clone())]
        }
    })
}

fn layer_halo_ally_wire(
    ally: &crate::engine::manager::buff::LayerHaloAllyResult,
) -> Vec<ActEffect> {
    use crate::engine::manager::buff::BuffPublishPart;
    let leads = !ally.replaced.is_empty();
    let lead = || {
        ally.added
            .iter()
            .flat_map(EffectPacket::buff_add_pre_markers)
            .collect::<Vec<_>>()
    };
    let removal = || {
        ally.replaced
            .iter()
            .map(EffectPacket::buff_delete)
            .collect::<Vec<_>>()
    };
    let addition = || {
        ally.trimmed
            .iter()
            .map(EffectPacket::buff_delete)
            .chain(ally.trim_refreshed.iter().map(EffectPacket::buff_update))
            .chain(ally.added.iter().flat_map(|added| {
                if leads {
                    EffectPacket::buff_add_body(added)
                } else {
                    EffectPacket::buff_add_direct(added)
                }
            }))
            .chain(ally.refreshed.iter().flat_map(|refreshed| {
                std::iter::once(EffectPacket::buff_update(&refreshed.update))
                    .chain(refreshed.markers.iter().map(EffectPacket::buff_marker))
            }))
            .collect::<Vec<_>>()
    };
    match ally.part {
        BuffPublishPart::Whole if leads => [lead(), removal(), addition()].concat(),
        BuffPublishPart::Whole | BuffPublishPart::Addition => addition(),
        BuffPublishPart::Lead => lead(),
        BuffPublishPart::Removal => removal(),
    }
}

fn magic_circle_snapshot(
    change: &FieldChange,
) -> Option<crate::engine::mechanic::magic_circle::MagicCircleApplyResult> {
    let state = change.after?;
    Some(
        crate::engine::mechanic::magic_circle::MagicCircleApplyResult {
            target_uid: state.owner_uid,
            circle_id: state.definition.field_id,
            info: MagicCircleInfo {
                magic_circle_id: Some(state.definition.field_id),
                round: Some(state.definition.duration),
                create_uid: Some(state.create_uid),
                electric_level: (state.level > 0).then_some(state.level),
                electric_progress: Some(state.progress),
                max_electric_progress: Some(state.next_upgrade_progress),
            },
        },
    )
}

fn project_cue(
    cue: &RoundCue,
    redeal_layout: RedealWireLayout,
    change_round_layout: ChangeRoundWireLayout,
) -> Vec<ActEffect> {
    match cue {
        RoundCue::CastStep => Vec::new(),
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
        RoundCue::ChangeRound { round } => {
            vec![EffectPacket::change_round(match change_round_layout {
                ChangeRoundWireLayout::Incremental => 0,
                ChangeRoundWireLayout::CurrentRound => *round,
            })]
        }
        RoundCue::ReserveSplitStart { team_type } => {
            vec![EffectPacket::reserve_split(
                *team_type,
                EffectType::Splitstart,
            )]
        }
        RoundCue::ReserveSplitEnd { team_type } => {
            vec![EffectPacket::reserve_split(
                *team_type,
                EffectType::Splitend,
            )]
        }
        RoundCue::ExtraRoundGranted => vec![EffectPacket::fight_param_change(18, 1)],
    }
}

#[cfg(test)]
mod tests;
