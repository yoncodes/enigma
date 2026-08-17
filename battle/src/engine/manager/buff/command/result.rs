use super::super::{BuffDefinition, BuffFanoutResult, BuffRemoveResult, BuffUpdateResult};
use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct BuffChanges {
    pub origin: CommandOrigin,
    pub change: BuffReplaceResult,
    pub lifecycle_transitions: Vec<BuffLifecycleTransition>,
    pub fanout: Vec<BuffFanoutResult>,
    pub pre_add_markers_before_remove: bool,
    pub refresh_wire: Vec<BuffRefreshWire>,
    pub state_snapshot_wire: Vec<BuffStateSnapshotWire>,
    pub shield_removed: Vec<BuffShieldRemoveResult>,
    pub act_info_markers: Vec<BuffActInfoMarkerResult>,
    wire_visible: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BuffLifecycleTransition {
    Removed(BuffRemoveResult),
    Refreshed(BuffUpdateResult),
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuffRefreshWire {
    pub echo_before: bool,
    pub markers: Vec<BuffMarkerResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuffStateSnapshotWire {
    pub refresh_index: usize,
    pub effect_type: i32,
    pub effect_num: i32,
    pub reserve_str: Option<String>,
}

impl BuffChanges {
    pub(super) fn new(
        catalog: crate::catalog::BattleCatalog,
        origin: CommandOrigin,
        change: BuffReplaceResult,
    ) -> Self {
        Self::with_refresh_echo(catalog, origin, change, true)
    }

    pub(super) fn set_amount(
        catalog: crate::catalog::BattleCatalog,
        origin: CommandOrigin,
        change: BuffReplaceResult,
    ) -> Self {
        Self::without_refresh_echo(catalog, origin, change)
    }

    pub(super) fn without_refresh_echo(
        catalog: crate::catalog::BattleCatalog,
        origin: CommandOrigin,
        change: BuffReplaceResult,
    ) -> Self {
        Self::with_refresh_echo(catalog, origin, change, false)
    }

    fn with_refresh_echo(
        catalog: crate::catalog::BattleCatalog,
        origin: CommandOrigin,
        mut change: BuffReplaceResult,
        echo_existing_layer: bool,
    ) -> Self {
        let has_add = change.added.is_some();
        let pre_add_markers_before_remove = !change.removed.is_empty()
            && change
                .added
                .as_ref()
                .is_some_and(|added| !added.pre_markers.is_empty());
        let refresh_wire = change
            .refreshed
            .iter()
            .map(|refresh| {
                let definition = BuffDefinition::configured(
                    catalog.game_data(),
                    refresh.after.buff_id.unwrap_or_default(),
                );
                let markers = if !has_add
                    && (refresh_increases_effect_value(refresh)
                        || definition
                            .as_ref()
                            .is_some_and(BuffDefinition::refreshes_unchanged))
                {
                    definition
                        .as_ref()
                        .map(|definition| {
                            definition
                                .wire_markers(
                                    crate::engine::skill::buff_act::wire::WirePhase::Refresh,
                                )
                                .into_iter()
                                .map(|effect_type| BuffMarkerResult {
                                    target_uid: refresh.target_uid,
                                    effect_type,
                                    effect_num: definition.marker_effect_num(
                                        catalog.game_data(),
                                        effect_type,
                                        refresh.after.act_common_params.as_deref(),
                                    ),
                                    buff_act_id: 0,
                                })
                                .collect()
                        })
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                BuffRefreshWire {
                    echo_before: echo_existing_layer
                        && definition
                            .as_ref()
                            .is_some_and(BuffDefinition::emits_existing_layer_on_refresh)
                        && refresh.before.uid == refresh.after.uid
                        && refresh.before.layer.unwrap_or_default() > 0,
                    markers,
                }
            })
            .collect();
        let mut fanout = std::mem::take(&mut change.fanout);
        fanout.extend(
            change
                .added
                .as_mut()
                .map(|added| {
                    let emitter_uid = added.target_uid;
                    let carrier_buff_uid = added.buff.uid.unwrap_or_default();
                    let carrier_buff_id = added.buff.buff_id.unwrap_or_default();
                    let mut groups = Vec::<BuffFanoutResult>::new();
                    for child in std::mem::take(&mut added.fanout) {
                        let Some(rule) = child.derived_by else {
                            continue;
                        };
                        if let Some(group) = groups.iter_mut().find(|group| group.rule == rule) {
                            group.added.push(child);
                        } else {
                            groups.push(BuffFanoutResult {
                                rule,
                                emitter_uid,
                                carrier_buff_uid,
                                carrier_buff_id,
                                added: vec![child],
                                refreshed: Vec::new(),
                            });
                        }
                    }
                    groups
                })
                .unwrap_or_default(),
        );
        Self {
            origin,
            change,
            lifecycle_transitions: Vec::new(),
            fanout,
            pre_add_markers_before_remove,
            refresh_wire,
            state_snapshot_wire: Vec::new(),
            shield_removed: Vec::new(),
            act_info_markers: Vec::new(),
            wire_visible: true,
        }
    }

    pub(super) fn with_lifecycle_transitions(
        mut self,
        transitions: Vec<BuffLifecycleTransition>,
    ) -> Self {
        self.lifecycle_transitions = transitions;
        self
    }

    pub(super) fn internal(mut self) -> Self {
        self.wire_visible = false;
        self
    }

    pub(super) fn with_act_info_marker(mut self, marker: BuffActInfoMarkerResult) -> Self {
        self.act_info_markers.push(marker);
        self
    }

    pub fn is_wire_visible(&self) -> bool {
        self.wire_visible
    }

    pub(super) fn with_state_snapshot_wire(
        mut self,
        catalog: crate::catalog::BattleCatalog,
    ) -> Self {
        self.state_snapshot_wire = self
            .change
            .refreshed
            .iter()
            .enumerate()
            .flat_map(|(refresh_index, refresh)| {
                BuffDefinition::configured(
                    catalog.game_data(),
                    refresh.after.buff_id.unwrap_or_default(),
                )
                .into_iter()
                .flat_map(move |definition| {
                    definition
                        .state_snapshot_wire(refresh.after.act_common_params.as_deref())
                        .into_iter()
                        .map(move |(effect_type, reserve_str)| BuffStateSnapshotWire {
                            refresh_index,
                            effect_type,
                            effect_num: definition.state_snapshot_effect_num(
                                effect_type,
                                refresh.before.act_common_params.as_deref(),
                                refresh.after.act_common_params.as_deref(),
                            ),
                            reserve_str,
                        })
                })
            })
            .collect();
        self
    }

    pub fn events(&self) -> Vec<BattleEvent> {
        if !self.wire_visible {
            return Vec::new();
        }
        let mut events = if self.lifecycle_transitions.is_empty() {
            self.change.events()
        } else {
            self.lifecycle_transitions
                .iter()
                .flat_map(|transition| match transition {
                    BuffLifecycleTransition::Removed(removed) => BuffReplaceResult {
                        removed: vec![removed.clone()],
                        ..Default::default()
                    }
                    .events(),
                    BuffLifecycleTransition::Refreshed(refreshed) => BuffReplaceResult {
                        refreshed: vec![refreshed.clone()],
                        ..Default::default()
                    }
                    .events(),
                })
                .collect()
        };
        for fanout in &self.fanout {
            for added in &fanout.added {
                events.extend(
                    BuffReplaceResult {
                        added: Some(added.clone()),
                        ..Default::default()
                    }
                    .events(),
                );
            }
            for refreshed in &fanout.refreshed {
                events.extend(
                    BuffReplaceResult {
                        refreshed: vec![refreshed.update.clone()],
                        ..Default::default()
                    }
                    .events(),
                );
            }
        }
        events
    }
}

fn refresh_increases_effect_value(change: &BuffUpdateResult) -> bool {
    change.before.buff_id != change.after.buff_id
        || change.after.layer.unwrap_or_default() > change.before.layer.unwrap_or_default()
        || change.after.count.unwrap_or_default() > change.before.count.unwrap_or_default()
        || change.before.act_common_params != change.after.act_common_params
        || change.before.act_info != change.after.act_info
}
