use super::*;

impl EffectPacket {
    pub fn buff_add(change: &BuffApplyResult) -> Vec<ActEffect> {
        let mut effects = Self::buff_add_direct(change);
        for fanout in &change.fanout {
            effects.extend(Self::buff_add(fanout));
        }
        effects
    }

    pub fn buff_changes(
        change: &crate::engine::manager::buff::BuffReplaceResult,
    ) -> Vec<ActEffect> {
        if let Some(rejected) = &change.rejected {
            return std::iter::once(Self::buff_reject(rejected))
                .chain(change.removed.iter().flat_map(Self::buff_delete_effects))
                .chain(change.refreshed.iter().map(Self::buff_update))
                .collect();
        }
        let mut effects: Vec<_> = change
            .removed
            .iter()
            .flat_map(Self::buff_delete_effects)
            .collect();
        if let Some(added) = &change.added {
            effects.extend(Self::buff_add(added));
        }
        for refreshed in &change.refreshed {
            if crate::engine::manager::buff::emits_existing_layer_on_refresh(
                refreshed.after.buff_id.unwrap_or_default(),
            ) && refreshed.before.uid == refreshed.after.uid
                && refreshed.before.layer.unwrap_or_default() > 0
            {
                effects.push(Self::buff_update(&BuffUpdateResult {
                    target_uid: refreshed.target_uid,
                    before: refreshed.before.clone(),
                    after: refreshed.before.clone(),
                }));
            }
            effects.push(Self::buff_update(refreshed));
            if refresh_increases_effect_value(refreshed) {
                effects.extend(
                    marker::refresh_markers(refreshed.after.buff_id.unwrap_or_default())
                        .into_iter()
                        .map(|marker| {
                            Self::buff_marker(&BuffMarkerResult {
                                target_uid: refreshed.target_uid,
                                effect_type: marker.effect_type,
                                effect_num: 0,
                                buff_act_id: 0,
                            })
                        }),
                );
            }
        }
        effects
    }

    pub fn recorded_buff_changes(
        changes: &crate::engine::manager::buff::BuffChanges,
    ) -> Vec<ActEffect> {
        if !changes.is_wire_visible() {
            return Vec::new();
        }
        if !changes.lifecycle_transitions.is_empty() {
            return changes
                .lifecycle_transitions
                .iter()
                .flat_map(|transition| match transition {
                    crate::engine::manager::buff::BuffLifecycleTransition::Removed(removed) => {
                        let mut effects = Self::buff_delete_effects(removed);
                        if let Some(shield) = changes
                            .shield_removed
                            .iter()
                            .find(|shield| shield.buff_uid == removed.buff.uid.unwrap_or_default())
                        {
                            effects.push(Self::shield_delete(shield.target_uid, shield.value));
                        }
                        effects
                    }
                    crate::engine::manager::buff::BuffLifecycleTransition::Refreshed(refreshed) => {
                        vec![Self::buff_update(refreshed)]
                    }
                })
                .collect();
        }
        let change = &changes.change;
        if let Some(rejected) = &change.rejected {
            return std::iter::once(Self::buff_reject(rejected))
                .chain(change.removed.iter().flat_map(Self::buff_delete_effects))
                .chain(change.refreshed.iter().map(Self::buff_update))
                .collect();
        }
        let mut effects = Vec::new();
        if changes.pre_add_markers_before_remove
            && let Some(added) = &change.added
        {
            effects.extend(Self::buff_add_pre_markers(added));
        }
        for removed in &change.removed {
            if let Some(reason) = Self::buff_delete_reason(removed) {
                effects.push(reason);
            }
            effects.push(Self::buff_delete(removed));
            if let Some(shield) = changes
                .shield_removed
                .iter()
                .find(|shield| shield.buff_uid == removed.buff.uid.unwrap_or_default())
            {
                effects.push(Self::shield_delete(shield.target_uid, shield.value));
            }
        }
        if let Some(added) = &change.added {
            effects.extend(if changes.pre_add_markers_before_remove {
                Self::buff_add_body(added)
            } else {
                Self::buff_add(added)
            });
        }
        for (index, refreshed) in change.refreshed.iter().enumerate() {
            let snapshot_markers = changes
                .state_snapshot_wire
                .iter()
                .filter(|wire| wire.refresh_index == index)
                .collect::<Vec<_>>();
            if !snapshot_markers.is_empty() {
                effects.extend(snapshot_markers.into_iter().map(|wire| {
                    Self::buff_snapshot_marker(
                        refreshed.target_uid,
                        wire.effect_type,
                        refreshed.after.clone(),
                        wire.reserve_str.clone(),
                    )
                }));
                continue;
            }
            let wire = changes.refresh_wire.get(index);
            if wire.is_some_and(|wire| wire.echo_before) {
                effects.push(Self::buff_update(&BuffUpdateResult {
                    target_uid: refreshed.target_uid,
                    before: refreshed.before.clone(),
                    after: refreshed.before.clone(),
                }));
            }
            effects.push(Self::buff_update(refreshed));
            if let Some(wire) = wire {
                effects.extend(wire.markers.iter().map(Self::buff_marker));
            }
        }
        effects
    }

    pub fn buff_add_direct(change: &BuffApplyResult) -> Vec<ActEffect> {
        let mut effects = Self::buff_add_pre_markers(change);
        effects.extend(Self::buff_add_body(change));
        effects
    }

    fn buff_add_pre_markers(change: &BuffApplyResult) -> Vec<ActEffect> {
        change
            .pre_markers
            .iter()
            .map(|marker| {
                Self::buff_act_info_with_team_and_str(
                    marker.target_uid,
                    marker.buff_uid,
                    marker.act_id,
                    marker.params.clone(),
                    marker.str_param.clone().unwrap_or_default(),
                    marker.team_type,
                )
            })
            .collect()
    }

    fn buff_add_body(change: &BuffApplyResult) -> Vec<ActEffect> {
        let mut effects = change
            .pre_effects
            .iter()
            .map(|effect| ActEffect {
                target_id: Some(effect.target_uid),
                effect_type: Some(effect.effect_type),
                effect_num: Some(effect.effect_num),
                effect_num1: Some(effect.effect_num1),
                config_effect: Some(0),
                ..Default::default()
            })
            .collect::<Vec<_>>();

        effects.push(ActEffect {
            target_id: Some(change.target_uid),
            effect_type: Some(EffectType::Buffadd as i32),
            effect_num: change.buff.buff_id,
            buff: Some(change.buff.clone()),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(0),
            effect_num1: Some(0),
            ..Default::default()
        });

        effects.extend(change.markers.iter().map(Self::buff_marker));
        effects
    }

    pub fn buff_update(change: &BuffUpdateResult) -> ActEffect {
        ActEffect {
            target_id: Some(change.target_uid),
            effect_type: Some(EffectType::Buffupdate as i32),
            effect_num: Some(0),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(0),
            effect_num1: Some(0),
            buff: Some(change.after.clone()),
            ..Default::default()
        }
    }

    pub fn buff_reject(change: &BuffRejectResult) -> ActEffect {
        ActEffect {
            target_id: Some(change.target_uid),
            effect_type: Some(EffectType::Buffreject as i32),
            effect_num: Some(change.blocker_buff_id),
            buff: Some(change.buff.clone()),
            config_effect: Some(1),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(0),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn buff_delete(change: &BuffRemoveResult) -> ActEffect {
        let mut buff = change.buff.clone();
        if !change.depleted {
            buff.duration = Some(0);
        }
        ActEffect {
            target_id: Some(change.target_uid),
            effect_type: Some(EffectType::Buffdel as i32),
            effect_num: Some(0),
            config_effect: Some(change.config_effect),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(0),
            effect_num1: Some(0),
            buff: Some(buff),
            ..Default::default()
        }
    }

    fn buff_delete_effects(change: &BuffRemoveResult) -> Vec<ActEffect> {
        Self::buff_delete_reason(change)
            .into_iter()
            .chain(std::iter::once(Self::buff_delete(change)))
            .collect()
    }

    fn buff_delete_reason(change: &BuffRemoveResult) -> Option<ActEffect> {
        Some(ActEffect {
            target_id: Some(change.target_uid),
            effect_type: Some(EffectType::Buffdelreason as i32),
            effect_num: Some(change.delete_reason? as i32),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: change.buff.uid,
            team_type: Some(0),
            effect_num1: Some(0),
            ..Default::default()
        })
    }

    pub fn buff_marker(change: &BuffMarkerResult) -> ActEffect {
        ActEffect {
            target_id: Some(change.target_uid),
            effect_type: Some(change.effect_type),
            effect_num: Some(change.effect_num),
            config_effect: Some(0),
            buff_act_id: Some(change.buff_act_id),
            reserve_id: Some(0),
            team_type: Some(0),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn buff_act_trigger(
        change: crate::engine::manager::buff::BuffActTriggerResult,
    ) -> ActEffect {
        ActEffect {
            target_id: Some(change.target_uid),
            effect_type: Some(EffectType::Trigger as i32),
            effect_num: Some(change.buff_id),
            config_effect: Some(-1),
            buff_act_id: Some(change.buff_act_id),
            reserve_id: Some(0),
            team_type: Some(0),
            effect_num1: Some(0),
            ..Default::default()
        }
    }

    pub fn buff_snapshot_marker(
        target_uid: i64,
        effect_type: i32,
        buff: BuffInfo,
        reserve_str: Option<String>,
    ) -> ActEffect {
        ActEffect {
            target_id: Some(target_uid),
            effect_type: Some(effect_type),

            effect_num: Some(0),
            config_effect: Some(0),
            buff_act_id: Some(0),
            reserve_id: Some(0),
            team_type: Some(0),
            effect_num1: Some(0),
            buff: Some(buff),
            reserve_str,
            ..Default::default()
        }
    }
}
