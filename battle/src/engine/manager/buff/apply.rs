use super::grant_plan::{
    FanoutSpec, GrantAction, LayerRefreshPlan, PlannedFanout, PlannedFanoutRefresh,
};
use super::*;
use crate::engine::skill::buff_act::registry::BuffActKind;

impl BuffManager {
    #[cfg(test)]
    pub(crate) fn add(
        &mut self,
        hp: &HpManager,
        source_uid: i64,
        target_uid: i64,
        buff_id: i32,
        layer: i32,
    ) -> Option<BuffApplyResult> {
        self.add_unconditional(hp, source_uid, target_uid, buff_id, layer)
    }

    #[cfg(test)]
    fn add_unconditional(
        &mut self,
        hp: &HpManager,
        source_uid: i64,
        target_uid: i64,
        buff_id: i32,
        layer: i32,
    ) -> Option<BuffApplyResult> {
        let request = command::GrantRequest {
            source_uid,
            target_uid,
            buff_id,
            input: command::GrantInput::UnconditionalLayer(layer),
            occurrences: 1,
            child_uid_reservations: 0,
            force_normal_uid: false,
        };
        self.plan_grant(hp, request)
            .ok()
            .and_then(|plan| self.commit_grant_plan(hp, plan).added)
    }

    #[cfg(test)]
    pub(crate) fn add_replacing_excluded(
        &mut self,
        hp: &HpManager,
        source_uid: i64,
        target_uid: i64,
        buff_id: i32,
        layer: i32,
    ) -> BuffReplaceResult {
        self.add_replacing_excluded_with_layer_specified(
            hp, source_uid, target_uid, buff_id, layer, true,
        )
    }

    #[cfg(test)]
    pub(crate) fn add_replacing_excluded_with_layer_specified(
        &mut self,
        hp: &HpManager,
        source_uid: i64,
        target_uid: i64,
        buff_id: i32,
        layer: i32,
        layer_specified: bool,
    ) -> BuffReplaceResult {
        let request = command::GrantRequest {
            source_uid,
            target_uid,
            buff_id,
            input: if layer_specified {
                command::GrantInput::Layer(layer)
            } else {
                command::GrantInput::Default
            },
            occurrences: 1,
            child_uid_reservations: 0,
            force_normal_uid: false,
        };
        self.plan_grant(hp, request)
            .map(|plan| self.commit_grant_plan(hp, plan))
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(crate) fn add_repeated_replacing_excluded(
        &mut self,
        hp: &HpManager,
        source_uid: i64,
        target_uid: i64,
        buff_id: i32,
        layer: i32,
    ) -> BuffReplaceResult {
        let request = command::GrantRequest {
            source_uid,
            target_uid,
            buff_id,
            input: command::GrantInput::Default,
            occurrences: u32::try_from(layer).unwrap_or_default(),
            child_uid_reservations: 0,
            force_normal_uid: false,
        };
        self.plan_grant(hp, request)
            .map(|plan| self.commit_grant_plan(hp, plan))
            .unwrap_or_default()
    }

    fn add_one_with_uid(
        &mut self,
        hp: &HpManager,
        route: BuffRoute,
        definition: &BuffDefinition,
        args: BuffAddArgs,
        uid_owner: i64,
        uid_plan: uid_policy::UidAllocationPlan,
    ) -> Option<BuffApplyResult> {
        let layer = self.grant_layer(route, definition, args);
        let BuffRoute {
            source_uid,
            target_uid,
            buff_id,
        } = route;
        let team_type = self.team_type(target_uid).unwrap_or_default();
        let uid = uid_policy::commit(self, uid_owner, uid_plan);
        let count = definition.count(args.count);
        let mut buff = BuffInfo {
            buff_id: Some(buff_id),
            duration: Some(definition.duration),
            uid: Some(uid),
            ex_info: Some(0),
            from_uid: Some(source_uid),
            count: Some(count),
            layer: Some(layer.max(0)),
            act_common_params: Some(definition.act_common_params.clone()),
            r#type: Some(buff_wire_type(buff_id, source_uid, target_uid)),
            ..Default::default()
        };
        let current_hp = hp.current(target_uid);
        let pre_markers = definition.initial_wire_states(target_uid, uid, team_type, current_hp);
        let pre_effects = definition.pre_add_wire_effects(target_uid);
        buff.act_info = pre_markers
            .iter()
            .map(|marker| BuffActInfo {
                act_id: Some(marker.act_id),
                param: marker.params.clone(),
                str_param: marker.str_param.clone().or_else(|| Some(String::new())),
            })
            .collect();

        self.buffs.push(ActiveBuff {
            owner_uid: target_uid,
            team_type,
            type_id: definition.effective_type_id(),
            definition: Some(definition.clone()),
            buff: buff.clone(),
        });
        self.record_added(target_uid, buff_id, count_or_layer(&buff));
        let markers = marker::add_markers(buff_id)
            .into_iter()
            .map(|marker| BuffMarkerResult {
                target_uid,
                effect_type: marker.effect_type,
                effect_num: marker::effect_num(
                    marker.effect_type,
                    buff_id,
                    buff.act_common_params.as_deref(),
                ),
                buff_act_id: 0,
            })
            .collect();

        Some(BuffApplyResult {
            source_uid,
            target_uid,
            buff,
            pre_markers,
            pre_effects,
            markers,
            fanout: Vec::new(),
            derived_by: None,
        })
    }

    fn add_with_definition_using_uid(
        &mut self,
        hp: &HpManager,
        route: BuffRoute,
        definition: &BuffDefinition,
        args: BuffAddArgs,
        uid: uid_policy::UidAllocationPlan,
        fanout: &[PlannedFanout],
    ) -> Option<BuffApplyResult> {
        let mut applied =
            self.add_one_with_uid(hp, route, definition, args, route.target_uid, uid)?;
        applied.fanout = self.commit_fanout(hp, fanout);
        Some(applied)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn commit_grant_action(
        &mut self,
        hp: &HpManager,
        route: BuffRoute,
        definition: &BuffDefinition,
        args: BuffAddArgs,
        repeat: i32,
        action: GrantAction,
        removed: Vec<BuffRemoveResult>,
        uid: Option<uid_policy::UidAllocationPlan>,
        refresh_uids: &[uid_policy::UidAllocationPlan],
        layer_refresh: Option<LayerRefreshPlan>,
        layer_refresh_uid: Option<i64>,
        fanout: &[PlannedFanout],
    ) -> BuffReplaceResult {
        let mut result = BuffReplaceResult::default();
        match action {
            GrantAction::Reject(blocker) => {
                result.rejected =
                    Some(self.reject_with_blocker(route, definition, args, blocker, uid));
            }
            GrantAction::RefreshCount => {
                result.refreshed = self.refresh_typed_count_using_uids(
                    route.target_uid,
                    route.buff_id,
                    definition,
                    repeat,
                    refresh_uids,
                );
            }
            GrantAction::RefreshLayer => {
                result.refreshed = match layer_refresh {
                    Some(plan) => {
                        self.commit_layer_refresh(route.target_uid, plan, layer_refresh_uid)
                    }
                    None => self.refresh_stack_layer(
                        route.source_uid,
                        route.target_uid,
                        route.buff_id,
                        definition,
                        args,
                    ),
                }
                .into_iter()
                .collect();
            }
            GrantAction::RefreshExisting => {
                result.refreshed = self
                    .refresh_existing(route, definition)
                    .into_iter()
                    .collect();
            }
            GrantAction::KeepExisting => {}
            GrantAction::RetainEnhancedVariant => {
                if let Some(uid) = uid {
                    uid_policy::commit(self, route.target_uid, uid);
                }
            }
            GrantAction::ReplaceExisting => {
                result.added = uid.and_then(|uid| {
                    self.add_with_definition_using_uid(hp, route, definition, args, uid, fanout)
                });
            }
            GrantAction::Add => {
                result.added = uid.and_then(|uid| {
                    self.add_with_definition_using_uid(hp, route, definition, args, uid, fanout)
                });
                if result.added.is_some() && repeat > 1 {
                    result.refreshed = self.refresh_typed_count_using_uids(
                        route.target_uid,
                        route.buff_id,
                        definition,
                        repeat - 1,
                        refresh_uids,
                    );
                }
            }
        }
        result.removed = removed;
        result
    }

    fn refresh_existing(
        &mut self,
        route: BuffRoute,
        definition: &BuffDefinition,
    ) -> Option<BuffUpdateResult> {
        let active = self.buffs.iter_mut().find(|active| {
            active.owner_uid == route.target_uid && active.buff.buff_id == Some(route.buff_id)
        })?;
        let before = active.buff.clone();
        active.buff.duration = Some(
            active
                .buff
                .duration
                .unwrap_or_default()
                .max(definition.duration),
        );
        Some(BuffUpdateResult {
            target_uid: route.target_uid,
            before,
            after: active.buff.clone(),
        })
    }

    fn reject_with_blocker(
        &mut self,
        route: BuffRoute,
        definition: &BuffDefinition,
        args: BuffAddArgs,
        blocker_buff_id: i32,
        uid: Option<uid_policy::UidAllocationPlan>,
    ) -> BuffRejectResult {
        let uid = uid.unwrap_or_else(|| uid_policy::normal(self, route.target_uid, 0));
        let uid = uid_policy::commit(self, route.target_uid, uid);
        BuffRejectResult {
            target_uid: route.target_uid,
            blocker_buff_id,
            buff: BuffInfo {
                buff_id: Some(route.buff_id),
                duration: Some(definition.duration),
                uid: Some(uid),
                ex_info: Some(0),
                from_uid: Some(route.source_uid),
                count: Some(definition.count(args.count)),
                layer: Some(if definition.uses_stack_layer() {
                    definition.layer(args.layer, args.layer_specified, args.count)
                } else {
                    0
                }),
                act_common_params: Some(definition.act_common_params.clone()),
                r#type: Some(buff_wire_type(
                    route.buff_id,
                    route.source_uid,
                    route.target_uid,
                )),
                ..Default::default()
            },
        }
    }

    pub(super) fn blocking_buff_id(
        &self,
        target_uid: i64,
        buff_id: i32,
        definition: &BuffDefinition,
    ) -> Option<i32> {
        self.buffs.iter().find_map(|active| {
            let resident_buff_id = active.buff.buff_id.unwrap_or_default();
            let resident = active.definition.as_ref()?;
            let incoming_blocks_resident = definition.blocks_buff_id(resident_buff_id)
                || definition.blocks_status_id(resident.status_id);
            let resident_blocks_incoming =
                resident.blocks_buff_id(buff_id) || resident.blocks_status_id(definition.status_id);
            (active.owner_uid == target_uid
                && !incoming_blocks_resident
                && resident_blocks_incoming)
                .then_some(resident_buff_id)
        })
    }

    pub(super) fn immunity_blocker(
        &self,
        target_uid: i64,
        incoming_status: BuffStatus,
    ) -> Option<(i32, i64, command::ConsumeAction)> {
        let target_team = self.team_type(target_uid);
        self.buffs.iter().find_map(|active| {
            let definition = active.definition.as_ref()?;
            definition.features().iter().find_map(|feature| {
                let action = match feature.kind {
                    Some(BuffActKind::Immunity)
                        if active.owner_uid == target_uid
                            && incoming_status == BuffStatus::Control =>
                    {
                        command::ConsumeAction::Noop
                    }
                    Some(BuffActKind::ImmunityTimes)
                        if active.owner_uid == target_uid
                            && feature.values.get(1).copied().map(BuffStatus::from_id)
                                == Some(incoming_status)
                            && active.buff.count.unwrap_or_default() > 0 =>
                    {
                        Self::resolve_active_consume_action(
                            active,
                            1,
                            DepletedBuff::Remove,
                            command::ConsumeField::Count,
                        )
                    }
                    Some(BuffActKind::TeamImmunityTimes)
                        if Some(active.team_type) == target_team
                            && feature.values.get(1).copied().map(BuffStatus::from_id)
                                == Some(incoming_status) =>
                    {
                        let act_id = *feature.values.first()?;
                        let mut act_info = active.buff.act_info.clone();
                        let info = act_info
                            .iter_mut()
                            .find(|info| info.act_id == Some(act_id))?;
                        let remaining = info.param.first_mut()?;
                        if *remaining <= 0 {
                            return None;
                        }
                        *remaining -= 1;
                        command::ConsumeAction::SetInternalActInfo {
                            buff_uid: active.buff.uid?,
                            act_info,
                        }
                    }
                    _ => return None,
                };
                Some((
                    active.buff.buff_id.unwrap_or_default(),
                    active.owner_uid,
                    action,
                ))
            })
        })
    }

    fn refresh_typed_count_using_uids(
        &mut self,
        target_uid: i64,
        buff_id: i32,
        definition: &BuffDefinition,
        repeat: i32,
        planned_uids: &[uid_policy::UidAllocationPlan],
    ) -> Vec<BuffUpdateResult> {
        if !definition.uses_typed_count() || repeat <= 0 {
            return Vec::new();
        }

        let Some(index) = self.buffs.iter().position(|active| {
            active.owner_uid == target_uid && active.buff.buff_id == Some(buff_id)
        }) else {
            return Vec::new();
        };
        let mut updates = Vec::with_capacity(repeat as usize);
        for iteration in 0..repeat {
            if let Some(plan) = planned_uids.get(iteration as usize) {
                uid_policy::commit(self, target_uid, *plan);
            }
            let active = &mut self.buffs[index];
            let before = active.buff.clone();
            active.buff.count = Some(before.count.unwrap_or_default() + definition.count(0));
            active.buff.duration =
                Some(before.duration.unwrap_or_default().max(definition.duration));
            updates.push(BuffUpdateResult {
                target_uid,
                before,
                after: active.buff.clone(),
            });
            self.record_added(target_uid, buff_id, definition.count(0));
        }
        updates
    }

    fn refresh_stack_layer(
        &mut self,
        source_uid: i64,
        target_uid: i64,
        buff_id: i32,
        definition: &BuffDefinition,
        args: BuffAddArgs,
    ) -> Option<BuffUpdateResult> {
        if !definition.uses_stack_layer() {
            return None;
        }
        let route = BuffRoute::new(source_uid, target_uid, buff_id);
        let policy = BuffPolicy::for_buff_id(buff_id)?;
        let plan = self.plan_layer_refresh(route, definition, &policy, args)?;
        let promoted_uid = matches!(plan, LayerRefreshPlan::PromoteRestored { .. }).then(|| {
            let planned = uid_policy::plan(
                self,
                route,
                definition,
                policy.uid.allocation.uses_child_for_apply(args),
                0,
                0,
            );
            uid_policy::commit(self, target_uid, planned)
        });
        self.commit_layer_refresh(target_uid, plan, promoted_uid)
    }

    fn commit_layer_refresh(
        &mut self,
        target_uid: i64,
        plan: LayerRefreshPlan,
        promoted_uid: Option<i64>,
    ) -> Option<BuffUpdateResult> {
        let (index, next_layer, next_duration) = match plan {
            LayerRefreshPlan::NoChange => return None,
            LayerRefreshPlan::Echo { buff_uid } => {
                let active = self.buffs.iter().find(|active| {
                    active.owner_uid == target_uid && active.buff.uid == Some(buff_uid)
                })?;
                return Some(BuffUpdateResult {
                    target_uid,
                    before: active.buff.clone(),
                    after: active.buff.clone(),
                });
            }
            LayerRefreshPlan::PromoteRestored {
                buff_id,
                next_layer,
                next_duration,
            } => {
                let index = self.buffs.iter().position(|active| {
                    active.owner_uid == target_uid
                        && active.buff.buff_id == Some(buff_id)
                        && active.buff.uid.is_none()
                })?;
                (index, next_layer, next_duration)
            }
            LayerRefreshPlan::Update {
                buff_uid,
                next_layer,
                next_duration,
            } => {
                let index = self.buffs.iter().position(|active| {
                    active.owner_uid == target_uid && active.buff.uid == Some(buff_uid)
                })?;
                (index, next_layer, next_duration)
            }
        };
        let active = &mut self.buffs[index];
        let before = active.buff.clone();
        if active.buff.uid.is_none() {
            active.buff.uid = Some(promoted_uid?);
        }
        active.buff.layer = Some(next_layer);
        active.buff.duration = Some(next_duration);
        let result = BuffUpdateResult {
            target_uid,
            before,
            after: active.buff.clone(),
        };
        let added =
            result.after.layer.unwrap_or_default() - result.before.layer.unwrap_or_default();
        self.record_added(target_uid, result.after.buff_id.unwrap_or_default(), added);
        Some(result)
    }

    fn commit_fanout(&mut self, hp: &HpManager, plans: &[PlannedFanout]) -> Vec<BuffApplyResult> {
        plans
            .iter()
            .filter_map(|plan| self.commit_fanout_one(hp, &plan.spec, plan.uid))
            .collect()
    }

    fn commit_fanout_one(
        &mut self,
        hp: &HpManager,
        spec: &FanoutSpec,
        uid: uid_policy::UidAllocationPlan,
    ) -> Option<BuffApplyResult> {
        let grant_values = self.plan_grant_values(&spec.definition, spec.route.source_uid);
        let mut child = self.add_one_with_uid(
            hp,
            spec.route,
            &spec.definition,
            spec.args,
            spec.route.source_uid,
            uid,
        )?;
        if let Some(buff_uid) = child.buff.uid {
            self.commit_grant_values(buff_uid, &grant_values);
        }
        child.buff.duration = Some(spec.duration);
        if let Some(active) = self.buffs.iter_mut().find(|active| {
            active.owner_uid == child.target_uid && active.buff.uid == child.buff.uid
        }) {
            active.buff.duration = Some(spec.duration);
        }
        child.markers = halo::fanout_markers(spec.route.buff_id)
            .into_iter()
            .map(|marker| BuffMarkerResult {
                target_uid: spec.route.target_uid,
                effect_type: marker.effect_type as i32,
                effect_num: marker::effect_num(
                    marker.effect_type as i32,
                    child.buff.buff_id.unwrap_or_default(),
                    child.buff.act_common_params.as_deref(),
                ),
                buff_act_id: 0,
            })
            .collect();
        child.markers.extend(
            spec.definition
                .fanout_wire_markers(crate::engine::skill::buff_act::wire::WirePhase::Add)
                .into_iter()
                .map(|effect_type| BuffMarkerResult {
                    target_uid: spec.route.target_uid,
                    effect_type,
                    effect_num: marker::effect_num(
                        effect_type,
                        child.buff.buff_id.unwrap_or_default(),
                        child.buff.act_common_params.as_deref(),
                    ),
                    buff_act_id: 0,
                }),
        );
        child.derived_by = Some(spec.rule);
        Some(child)
    }

    pub(in crate::engine::manager::buff) fn commit_fanout_refreshes(
        &mut self,
        plans: &[PlannedFanoutRefresh],
    ) -> Vec<BuffFanoutResult> {
        let mut groups = Vec::<BuffFanoutResult>::new();
        for plan in plans {
            let Some(update) = self.update(
                plan.spec.route.target_uid,
                plan.buff_uid,
                Some(plan.spec.args.layer),
                None,
                Some(plan.spec.duration),
            ) else {
                continue;
            };
            let mut markers = halo::fanout_markers(plan.spec.route.buff_id)
                .into_iter()
                .map(|marker| BuffMarkerResult {
                    target_uid: plan.spec.route.target_uid,
                    effect_type: marker.effect_type as i32,
                    effect_num: marker::effect_num(
                        marker.effect_type as i32,
                        update.after.buff_id.unwrap_or_default(),
                        update.after.act_common_params.as_deref(),
                    ),
                    buff_act_id: 0,
                })
                .collect::<Vec<_>>();
            markers.extend(
                plan.spec
                    .definition
                    .fanout_wire_markers(crate::engine::skill::buff_act::wire::WirePhase::Refresh)
                    .into_iter()
                    .map(|effect_type| BuffMarkerResult {
                        target_uid: plan.spec.route.target_uid,
                        effect_type,
                        effect_num: marker::effect_num(
                            effect_type,
                            update.after.buff_id.unwrap_or_default(),
                            update.after.act_common_params.as_deref(),
                        ),
                        buff_act_id: 0,
                    }),
            );
            let refreshed = BuffFanoutUpdateResult { update, markers };
            if let Some(group) = groups.iter_mut().find(|group| {
                group.rule == plan.spec.rule && group.carrier_buff_uid == plan.carrier_buff_uid
            }) {
                group.refreshed.push(refreshed);
            } else {
                groups.push(BuffFanoutResult {
                    rule: plan.spec.rule,
                    emitter_uid: plan.spec.route.source_uid,
                    carrier_buff_uid: plan.carrier_buff_uid,
                    carrier_buff_id: plan.carrier_buff_id,
                    added: Vec::new(),
                    refreshed: vec![refreshed],
                });
            }
        }
        groups
    }

    pub(super) fn has_source_buff(&self, owner_uid: i64, buff_id: i32, source_uid: i64) -> bool {
        self.buffs.iter().any(|active| {
            active.owner_uid == owner_uid
                && active.buff.buff_id == Some(buff_id)
                && active.buff.from_uid == Some(source_uid)
        })
    }

    pub(super) fn allocator_for(&mut self, team_type: i32) -> &mut BuffUidAllocator {
        if self.shared_uid_lane {
            &mut self.attacker
        } else if team_type == 2 {
            &mut self.defender
        } else {
            &mut self.attacker
        }
    }
}
