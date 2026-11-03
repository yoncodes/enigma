use crate::engine::{
    buff::halo::{self, HaloKind, HaloScope},
    manager::hp::HpManager,
};

use super::{
    BuffAddArgs, BuffDefinition, BuffManager, BuffRoute,
    rules::{BuffPolicy, BuffStorage, DuplicateGrant},
    uid_policy::{self, UidAllocationPlan},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GrantAction {
    Reject(i32),
    Resist,
    RefreshCount,
    RefreshLayer,
    RefreshExisting,
    ProlongDuration,
    KeepExisting,
    RetainEnhancedVariant,
    ReplaceExisting,
    Add,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LayerRefreshPlan {
    NoChange,
    Echo {
        buff_uid: i64,
    },
    PromoteRestored {
        buff_id: i32,
        next_layer: i32,
        next_duration: i32,
    },
    Update {
        buff_uid: i64,
        next_layer: i32,
        next_duration: i32,
    },
}

#[derive(Debug, Clone)]
pub(super) struct FanoutSpec {
    pub(super) emitter_uid: i64,
    pub(super) route: BuffRoute,
    pub(super) definition: BuffDefinition,
    pub(super) args: BuffAddArgs,
    pub(super) duration: i32,
    pub(super) rule: crate::engine::skill::rule::DefinitionKey,
    pub(super) source_attack: Option<i32>,
}

#[derive(Debug, Clone)]
pub(super) struct PlannedFanout {
    pub(super) spec: FanoutSpec,
    pub(super) uid: UidAllocationPlan,
}

#[derive(Debug, Clone)]
pub(super) struct PlannedEntryHaloFanout {
    pub(super) emitter_uid: i64,
    pub(super) carrier_buff_uid: i64,
    pub(super) carrier_buff_id: i32,
    pub(super) fanout: PlannedFanout,
}

#[derive(Debug, Clone)]
pub(super) struct PlannedFanoutRefresh {
    pub(super) spec: FanoutSpec,
    pub(super) buff_uid: i64,
    pub(super) carrier_buff_uid: i64,
    pub(super) carrier_buff_id: i32,
    pub(super) uid_reservation: Option<UidAllocationPlan>,
}

impl BuffManager {
    pub(super) fn entry_halo_fanout_plans(
        &self,
        hp: &HpManager,
        target_uids: &[i64],
    ) -> Vec<PlannedEntryHaloFanout> {
        let mut carriers = self
            .buffs
            .iter()
            .filter(|active| {
                active.fanout_emitter_uid.is_none()
                    && self
                        .entities
                        .iter()
                        .any(|entity| entity.uid == active.owner_uid && entity.active)
                    && hp.current(active.owner_uid) > 0
                    && active.definition.as_ref().is_none_or(|definition| {
                        definition.duration <= 0 || active.buff.duration.unwrap_or_default() > 0
                    })
            })
            .flat_map(|active| {
                let emitter_uid = active.owner_uid;
                let source_uid = active.buff.from_uid.unwrap_or(emitter_uid);
                let carrier_buff_id = active.buff.buff_id.unwrap_or_default();
                let carrier_buff_uid = active.buff.uid.unwrap_or_default();
                let layer = active.buff.layer.unwrap_or_default();
                let count = active.buff.count.unwrap_or_default();
                let duration = active.buff.duration.unwrap_or_default();
                halo::carriers(self.catalog(), carrier_buff_id)
                    .into_iter()
                    .map(move |carrier| {
                        (
                            emitter_uid,
                            source_uid,
                            carrier_buff_uid,
                            carrier_buff_id,
                            layer,
                            count,
                            duration,
                            carrier,
                        )
                    })
            })
            .collect::<Vec<_>>();
        carriers.sort_by_key(|carrier| carrier.0);
        let mut preceding_by_lane = std::collections::HashMap::<i32, Vec<UidAllocationPlan>>::new();
        let mut seen = std::collections::HashSet::new();
        let mut plans = Vec::new();

        for (
            emitter_uid,
            source_uid,
            carrier_buff_uid,
            carrier_buff_id,
            layer,
            count,
            duration,
            carrier,
        ) in carriers
        {
            let Some(emitter_team) = self.team_type(emitter_uid) else {
                continue;
            };
            let fanout_buff_id = carrier.linked_buff_id.unwrap_or(carrier_buff_id);
            let Some(definition) =
                BuffDefinition::configured(self.catalog().game_data(), fanout_buff_id)
            else {
                continue;
            };
            for &target_uid in target_uids {
                let Some(target_team) = self
                    .entities
                    .iter()
                    .find(|entity| entity.uid == target_uid && entity.active)
                    .map(|entity| entity.team_type)
                else {
                    continue;
                };
                let in_scope = match carrier.scope {
                    HaloScope::AlliedTeam => target_team == emitter_team,
                    HaloScope::OtherAllies => {
                        target_team == emitter_team && target_uid != emitter_uid
                    }
                    HaloScope::OpposingTeam => target_team != emitter_team,
                };
                if !in_scope
                    || hp.current(target_uid) <= 0
                    || self.has_fanout_buff(target_uid, fanout_buff_id, emitter_uid)
                {
                    continue;
                }
                let rule = crate::engine::skill::rule::DefinitionKey::new(
                    carrier.opcode,
                    carrier.type_name,
                );
                if !seen.insert((emitter_uid, rule, target_uid, fanout_buff_id)) {
                    continue;
                }
                let lane = if self.shared_uid_lane {
                    0
                } else {
                    emitter_team
                };
                let preceding = preceding_by_lane.entry(lane).or_default();
                let uid = uid_policy::children_after_sequence(
                    self,
                    emitter_uid,
                    0,
                    preceding.iter().copied(),
                    1,
                )[0];
                preceding.push(uid);
                plans.push(PlannedEntryHaloFanout {
                    emitter_uid,
                    carrier_buff_uid,
                    carrier_buff_id,
                    fanout: PlannedFanout {
                        spec: FanoutSpec {
                            emitter_uid,
                            route: BuffRoute::new(source_uid, target_uid, fanout_buff_id),
                            definition: definition.clone(),
                            args: BuffAddArgs {
                                layer,
                                count,
                                layer_specified: true,
                            },
                            duration,
                            rule,
                            source_attack: None,
                        },
                        uid,
                    },
                });
            }
        }
        plans
    }

    pub(super) fn grant_layer(
        &self,
        route: BuffRoute,
        definition: &BuffDefinition,
        args: BuffAddArgs,
    ) -> i32 {
        let layer = definition.raw_layer(args.layer, args.layer_specified, args.count);
        let mut limit = self.grant_stack_limit(route, definition);
        if limit > 0 && definition.shares_timed_stack_cap() {
            let other_duration_layers = self
                .buffs
                .iter()
                .filter(|active| {
                    active.effective
                        && active.owner_uid == route.target_uid
                        && active.buff.buff_id == Some(route.buff_id)
                        && active.buff.duration != Some(definition.duration)
                })
                .map(|active| super::count_or_layer_from(&active.buff, active.definition.as_ref()))
                .sum::<i32>();
            limit = limit.saturating_sub(other_duration_layers);
        }
        if limit <= 0 {
            return if definition.stack_max_layer() > 0 {
                0
            } else {
                definition.cap_layer(layer)
            };
        }
        layer.min(limit)
    }

    pub(super) fn grant_stack_limit(&self, route: BuffRoute, definition: &BuffDefinition) -> i32 {
        let base_limit = definition.stack_max_layer();
        if base_limit <= 0 {
            return base_limit;
        }
        let generic_bonus = self.max_buff_layer_bonus(route.source_uid, route.buff_id);
        let burn_bonus = if definition.features().iter().any(|feature| {
            feature.kind == Some(crate::engine::skill::buff_act::registry::BuffActKind::Burn)
        }) {
            self.max_burn_layer_bonus(route.target_uid)
        } else {
            0
        };
        base_limit + generic_bonus + burn_bonus
    }

    fn max_buff_layer_bonus(&self, source_uid: i64, buff_id: i32) -> i32 {
        let source_team = self.team_type(source_uid);
        let mut seen = std::collections::BTreeSet::new();
        self.buffs
            .iter()
            .filter(|active| Some(active.team_type) == source_team)
            .filter_map(|active| {
                let feature = active.definition.as_ref()?.features().iter().find(|feature| {
                    feature.kind
                        == Some(
                            crate::engine::skill::buff_act::registry::BuffActKind::ModifyMaxBuffLayers,
                        )
                        && feature.values.get(1) == Some(&buff_id)
                })?;
                let bonus = feature.values.get(2).copied()?.max(0);
                let source = active
                    .buff
                    .from_uid
                    .filter(|uid| *uid != 0)
                    .unwrap_or(active.owner_uid);
                seen.insert((source, active.buff.buff_id.unwrap_or_default()))
                    .then_some(bonus)
            })
            .sum()
    }

    fn max_burn_layer_bonus(&self, target_uid: i64) -> i32 {
        let mut seen = std::collections::BTreeSet::new();
        self.buffs
            .iter()
            .filter(|active| active.owner_uid == target_uid)
            .filter_map(|active| {
                let definition = active.definition.as_ref()?;
                let bonus = definition.features().iter().find_map(|feature| {
                    (feature.kind
                        == Some(
                            crate::engine::skill::buff_act::registry::BuffActKind::ModifyMaxBurnLayers,
                        ))
                    .then(|| feature.values.get(1).copied())
                    .flatten()
                })?;
                let source = active
                    .buff
                    .from_uid
                    .filter(|uid| *uid != 0)
                    .unwrap_or(active.owner_uid);
                seen.insert((source, active.buff.buff_id.unwrap_or_default()))
                    .then_some(bonus.max(0))
            })
            .sum()
    }

    pub(super) fn resolve_grant_action(
        &self,
        route: BuffRoute,
        definition: &BuffDefinition,
        policy: &BuffPolicy,
        args: BuffAddArgs,
        repeat: i32,
        stack_layer: i32,
    ) -> GrantAction {
        if let Some(blocker) = self.blocking_buff_id(route.target_uid, definition) {
            return GrantAction::Reject(blocker);
        }
        if self.buffs.iter().any(|active| {
            active.owner_uid == route.target_uid
                && active
                    .definition
                    .as_ref()
                    .is_some_and(|resident| resident.is_enhanced_passive_variant_of(definition))
        }) {
            return GrantAction::RetainEnhancedVariant;
        }

        let has_same_id = self.has_buff_id(route.target_uid, route.buff_id);
        let has_matching = self
            .buffs
            .iter()
            .any(|active| policy.matches(active, route));
        if policy.storage == BuffStorage::Layered
            && has_same_id
            && !has_matching
            && stack_layer <= 0
        {
            return GrantAction::KeepExisting;
        }
        if policy.instance_limit.is_some_and(|limit| {
            self.buffs
                .iter()
                .filter(|active| policy.matches(active, route))
                .count()
                >= limit as usize
        }) {
            return GrantAction::KeepExisting;
        }
        if has_matching && policy.on_duplicate == DuplicateGrant::KeepExisting {
            return GrantAction::KeepExisting;
        }
        if policy.storage == BuffStorage::Counted && repeat > 0 && has_matching {
            return GrantAction::RefreshCount;
        }
        if policy.storage == BuffStorage::Layered && has_same_id {
            let delta = definition.layer(args.layer, args.layer_specified, args.count);
            if delta > 0 && has_matching {
                return GrantAction::RefreshLayer;
            }
            if has_matching {
                return GrantAction::KeepExisting;
            }
        }
        if has_matching
            && policy.on_duplicate != DuplicateGrant::AddSeparateCopy
            && definition.preserves_regrant()
        {
            if definition.features().iter().any(|feature| {
                feature.kind
                    == Some(crate::engine::skill::buff_act::registry::BuffActKind::HaloBase)
            }) {
                return GrantAction::RefreshExisting;
            }
            return GrantAction::KeepExisting;
        }
        if definition.prolongs_duration()
            && self.buffs.iter().any(|active| {
                active.owner_uid == route.target_uid
                    && active.type_id == definition.effective_type_id()
                    && active
                        .definition
                        .as_ref()
                        .is_some_and(BuffDefinition::prolongs_duration)
            })
        {
            return GrantAction::ProlongDuration;
        }
        if has_matching && policy.on_duplicate == DuplicateGrant::ReplaceExisting {
            return GrantAction::ReplaceExisting;
        }

        GrantAction::Add
    }

    pub(super) fn plan_layer_refresh(
        &self,
        route: BuffRoute,
        definition: &BuffDefinition,
        policy: &BuffPolicy,
        args: BuffAddArgs,
    ) -> Option<LayerRefreshPlan> {
        let active = self
            .buffs
            .iter()
            .find(|active| policy.matches(active, route))?;
        let current_layer = active.buff.layer.unwrap_or_default();
        let next_layer = self
            .grant_layer(
                route,
                definition,
                BuffAddArgs {
                    layer: current_layer
                        + definition.raw_layer(args.layer, args.layer_specified, args.count),
                    layer_specified: true,
                    count: 0,
                },
            )
            .max(current_layer);
        let next_duration = active
            .buff
            .duration
            .unwrap_or_default()
            .max(policy.lifetime.duration);
        let buff_uid = active.buff.uid;

        if active.buff.layer == Some(next_layer) && active.buff.duration == Some(next_duration) {
            return Some(if definition.emits_existing_layer_on_refresh() {
                buff_uid.map_or(LayerRefreshPlan::NoChange, |buff_uid| {
                    LayerRefreshPlan::Echo { buff_uid }
                })
            } else {
                LayerRefreshPlan::NoChange
            });
        }

        Some(buff_uid.map_or(
            LayerRefreshPlan::PromoteRestored {
                buff_id: route.buff_id,
                next_layer,
                next_duration,
            },
            |buff_uid| LayerRefreshPlan::Update {
                buff_uid,
                next_layer,
                next_duration,
            },
        ))
    }

    pub(super) fn partially_caps_layer_refresh(
        &self,
        route: BuffRoute,
        definition: &BuffDefinition,
        policy: &BuffPolicy,
        args: BuffAddArgs,
        refresh: LayerRefreshPlan,
    ) -> bool {
        let next_layer = match refresh {
            LayerRefreshPlan::PromoteRestored { next_layer, .. }
            | LayerRefreshPlan::Update { next_layer, .. } => next_layer,
            LayerRefreshPlan::NoChange | LayerRefreshPlan::Echo { .. } => return false,
        };
        let Some(current_layer) = self
            .buffs
            .iter()
            .find(|active| policy.matches(active, route))
            .and_then(|active| active.buff.layer)
        else {
            return false;
        };
        let requested_layer = current_layer.saturating_add(definition.raw_layer(
            args.layer,
            args.layer_specified,
            args.count,
        ));

        current_layer < next_layer && next_layer < requested_layer
    }

    pub(super) fn fanout_specs(
        &self,
        hp: &HpManager,
        route: BuffRoute,
        args: BuffAddArgs,
        duration: i32,
    ) -> Vec<FanoutSpec> {
        let BuffRoute {
            source_uid,
            target_uid: emitter_uid,
            buff_id,
        } = route;
        let Some(team_type) = self.team_type(emitter_uid) else {
            return Vec::new();
        };
        let mut specs = Vec::new();
        for carrier in halo::carriers(self.catalog(), buff_id) {
            let fanout_buff_id = carrier.linked_buff_id.unwrap_or(buff_id);
            let Some(definition) =
                BuffDefinition::configured(self.catalog().game_data(), fanout_buff_id)
            else {
                continue;
            };
            let include_owner =
                carrier.kind == HaloKind::Master && carrier.scope == HaloScope::AlliedTeam;
            for target in self
                .entities
                .iter()
                .filter(|entity| entity.active)
                .filter(|entity| match carrier.scope {
                    HaloScope::AlliedTeam | HaloScope::OtherAllies => entity.team_type == team_type,
                    HaloScope::OpposingTeam => entity.team_type != team_type,
                })
                .filter(|entity| carrier.kind == HaloKind::Base || hp.current(entity.uid) > 0)
                .filter(|entity| include_owner || entity.uid != emitter_uid)
            {
                if self.has_fanout_buff(target.uid, fanout_buff_id, emitter_uid) {
                    continue;
                }
                specs.push(FanoutSpec {
                    emitter_uid,
                    route: BuffRoute::new(source_uid, target.uid, fanout_buff_id),
                    definition: definition.clone(),
                    args: BuffAddArgs {
                        layer: args.layer,
                        count: args.count,
                        layer_specified: true,
                    },
                    duration,
                    rule: crate::engine::skill::rule::DefinitionKey::new(
                        carrier.opcode,
                        carrier.type_name,
                    ),
                    source_attack: None,
                });
            }
        }
        specs
    }

    pub(super) fn fanout_refresh_specs(
        &self,
        _hp: &HpManager,
        route: BuffRoute,
        carrier_buff_uid: i64,
        layer: i32,
        duration: i32,
    ) -> Vec<PlannedFanoutRefresh> {
        let BuffRoute {
            source_uid,
            target_uid: emitter_uid,
            buff_id: carrier_buff_id,
        } = route;
        let Some(team_type) = self.team_type(emitter_uid) else {
            return Vec::new();
        };
        let mut specs = Vec::new();
        for carrier in halo::carriers(self.catalog(), carrier_buff_id)
            .into_iter()
            .filter(|carrier| matches!(carrier.kind, HaloKind::Base | HaloKind::LayerMaster))
        {
            let fanout_buff_id = carrier.linked_buff_id.unwrap_or(carrier_buff_id);
            let Some(definition) =
                BuffDefinition::configured(self.catalog().game_data(), fanout_buff_id)
            else {
                continue;
            };
            let rule =
                crate::engine::skill::rule::DefinitionKey::new(carrier.opcode, carrier.type_name);
            for target in self
                .entities
                .iter()
                .filter(|entity| entity.active)
                .filter(|entity| match carrier.scope {
                    HaloScope::AlliedTeam | HaloScope::OtherAllies => entity.team_type == team_type,
                    HaloScope::OpposingTeam => entity.team_type != team_type,
                })
                .filter(|entity| entity.uid != emitter_uid)
            {
                let Some(buff_uid) = self.buffs.iter().find_map(|active| {
                    (active.owner_uid == target.uid
                        && active.buff.buff_id == Some(fanout_buff_id)
                        && active
                            .fanout_emitter_uid
                            .unwrap_or_else(|| active.buff.from_uid.unwrap_or(active.owner_uid))
                            == emitter_uid)
                        .then_some(active.buff.uid)
                        .flatten()
                }) else {
                    continue;
                };
                specs.push(PlannedFanoutRefresh {
                    spec: FanoutSpec {
                        emitter_uid,
                        route: BuffRoute::new(source_uid, target.uid, fanout_buff_id),
                        definition: definition.clone(),
                        args: BuffAddArgs {
                            layer,
                            count: 0,
                            layer_specified: true,
                        },
                        duration,
                        rule,
                        source_attack: None,
                    },
                    buff_uid,
                    carrier_buff_uid,
                    carrier_buff_id,
                    uid_reservation: None,
                });
            }
        }
        specs
    }

    fn has_fanout_buff(&self, owner_uid: i64, buff_id: i32, emitter_uid: i64) -> bool {
        self.buffs.iter().any(|active| {
            active.owner_uid == owner_uid
                && active.buff.buff_id == Some(buff_id)
                && active
                    .fanout_emitter_uid
                    .unwrap_or_else(|| active.buff.from_uid.unwrap_or(active.owner_uid))
                    == emitter_uid
        })
    }
}
