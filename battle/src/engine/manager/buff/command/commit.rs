use super::*;

impl BuffManager {
    /// Applies a validated `BuffPlan` exactly once without re-resolving its policies.
    pub(crate) fn commit(&mut self, hp: &HpManager, plan: BuffPlan) -> BuffChanges {
        let origin = plan.origin;
        match plan.action {
            BuffPlanAction::Grant(plan) => {
                let change = self.commit_grant_plan(hp, *plan);
                BuffChanges::new(origin, change)
            }
            BuffPlanAction::GrantInternalChild(plan) => {
                let change = self.commit_grant_plan(hp, *plan);
                BuffChanges::new(origin, change).internal()
            }
            BuffPlanAction::Accumulate(plan) => {
                let change = self.commit_grant_plan(hp, *plan);
                BuffChanges::without_refresh_echo(origin, change)
            }
            BuffPlanAction::Consume(plan) => BuffChanges::without_refresh_echo(
                origin,
                plan.actions.into_iter().fold(
                    BuffReplaceResult::default(),
                    |mut change, action| {
                        let next = self.commit_consume_action(plan.target_uid, action);
                        change.removed.extend(next.removed);
                        change.refreshed.extend(next.refreshed);
                        change
                    },
                ),
            ),
            BuffPlanAction::ConsumeEffectCount(plan) => {
                for action in plan.actions {
                    self.commit_consume_action(plan.target_uid, action);
                }
                BuffChanges::new(origin, BuffReplaceResult::default())
            }
            BuffPlanAction::Convert(plan) => {
                let ConvertPlan { consume, grant } = *plan;
                let mut change = consume.actions.into_iter().fold(
                    BuffReplaceResult::default(),
                    |mut change, action| {
                        let next = self.commit_consume_action(consume.target_uid, action);
                        change.removed.extend(next.removed);
                        change.refreshed.extend(next.refreshed);
                        change
                    },
                );
                if let Some(grant) = grant {
                    let granted = self.commit_grant_plan(hp, grant);
                    change.removed.extend(granted.removed);
                    change.refreshed.extend(granted.refreshed);
                    change.added = granted.added;
                    change.rejected = granted.rejected;
                    change.fanout.extend(granted.fanout);
                }
                BuffChanges::new(origin, change)
            }
            BuffPlanAction::Replace(plan) => {
                BuffChanges::new(origin, self.commit_replace_plan(hp, *plan))
            }
            BuffPlanAction::Remove(plan) => BuffChanges::new(
                origin,
                BuffReplaceResult {
                    removed: plan
                        .buff_uids
                        .into_iter()
                        .flat_map(|buff_uid| {
                            let Some(index) = self.buffs.iter().position(|active| {
                                active.owner_uid == plan.target_uid
                                    && active.buff.uid == Some(buff_uid)
                            }) else {
                                return Vec::new();
                            };
                            let mut removed = self.remove_at(index, plan.config_effect);
                            if plan.clear_amount {
                                for result in &mut removed {
                                    result.buff.layer = Some(0);
                                }
                            }
                            if plan.clear_count {
                                for result in &mut removed {
                                    result.buff.count = Some(0);
                                }
                            }
                            removed
                        })
                        .collect(),
                    added: None,
                    refreshed: Vec::new(),
                    rejected: None,
                    fanout: Vec::new(),
                },
            ),
            BuffPlanAction::SetAmount(plan) => BuffChanges::set_amount(
                origin,
                BuffReplaceResult {
                    removed: Vec::new(),
                    added: None,
                    refreshed: plan
                        .exists
                        .then(|| {
                            let (layer, count) = match plan.amount {
                                BuffAmount::Layer(value) => (Some(value), None),
                                BuffAmount::Count(value) => (None, Some(value)),
                            };
                            self.update(plan.target_uid, plan.buff_uid, layer, count, None)
                        })
                        .flatten()
                        .into_iter()
                        .collect(),
                    rejected: None,
                    fanout: Vec::new(),
                },
            ),
            BuffPlanAction::SetState(plan) => BuffChanges::new(
                origin,
                BuffReplaceResult {
                    removed: Vec::new(),
                    added: None,

                    refreshed: plan
                        .exists
                        .then(|| {
                            self.set_state(
                                plan.target_uid,
                                plan.buff_uid,
                                plan.ex_info,
                                plan.params,
                                plan.act_info,
                            )
                        })
                        .flatten()
                        .into_iter()
                        .collect(),
                    rejected: None,
                    fanout: Vec::new(),
                },
            ),
            BuffPlanAction::SetInternalState(plan) => BuffChanges::new(
                origin,
                BuffReplaceResult {
                    removed: Vec::new(),
                    added: None,
                    refreshed: plan
                        .exists
                        .then(|| {
                            self.set_state(
                                plan.target_uid,
                                plan.buff_uid,
                                plan.ex_info,
                                plan.params,
                                plan.act_info,
                            )
                        })
                        .flatten()
                        .into_iter()
                        .collect(),
                    rejected: None,
                    fanout: Vec::new(),
                },
            )
            .internal(),
            BuffPlanAction::SetStateSnapshot(plan) => BuffChanges::new(
                origin,
                BuffReplaceResult {
                    removed: Vec::new(),
                    added: None,
                    refreshed: plan
                        .exists
                        .then(|| {
                            self.set_state(
                                plan.target_uid,
                                plan.buff_uid,
                                plan.ex_info,
                                plan.params,
                                plan.act_info,
                            )
                        })
                        .flatten()
                        .into_iter()
                        .collect(),
                    rejected: None,
                    fanout: Vec::new(),
                },
            )
            .with_state_snapshot_wire(),
            BuffPlanAction::AccumulateActValue(update) => {
                self.accumulate_act_value(update.buff_uid, update.act_id, update.delta);
                BuffChanges::new(origin, BuffReplaceResult::default())
            }
            BuffPlanAction::ChangeDuration(plans) => BuffChanges::new(
                origin,
                BuffReplaceResult {
                    refreshed: plans
                        .into_iter()
                        .filter_map(|plan| {
                            self.update(
                                plan.target_uid,
                                plan.buff_uid,
                                None,
                                None,
                                Some(plan.duration),
                            )
                        })
                        .collect(),
                    ..Default::default()
                },
            ),
            BuffPlanAction::AddSpecialCount(plan) => BuffChanges::new(
                origin,
                BuffReplaceResult {
                    removed: Vec::new(),
                    added: None,
                    refreshed: self.add_special_count(
                        plan.target_uid,
                        &plan.marker_ids,
                        plan.count,
                    ),
                    rejected: None,
                    fanout: Vec::new(),
                },
            ),
            BuffPlanAction::ReserveChildUids(plan) => {
                for uid in plan.uids {
                    super::uid_policy::commit(self, plan.target_uid, uid);
                }
                BuffChanges::new(origin, BuffReplaceResult::default())
            }
            BuffPlanAction::ReserveGrantUid(plan) => {
                let uid = super::uid_policy::commit(self, plan.target_uid, plan.uid);
                self.reserve_grant_uid(plan.target_uid, plan.buff_id, uid);
                BuffChanges::new(origin, BuffReplaceResult::default())
            }
            BuffPlanAction::AdvanceDuration(plans) => {
                let mut transitions = Vec::new();
                let change =
                    plans
                        .into_iter()
                        .fold(BuffReplaceResult::default(), |mut changes, plan| {
                            if let Some(next) = self.commit_duration_advance(plan) {
                                transitions.extend(
                                    next.removed
                                        .iter()
                                        .cloned()
                                        .map(BuffLifecycleTransition::Removed),
                                );
                                transitions.extend(
                                    next.refreshed
                                        .iter()
                                        .cloned()
                                        .map(BuffLifecycleTransition::Refreshed),
                                );
                                changes.removed.extend(next.removed);
                                changes.refreshed.extend(next.refreshed);
                            }
                            changes
                        });
                BuffChanges::without_refresh_echo(origin, change)
                    .with_lifecycle_transitions(transitions)
            }
            BuffPlanAction::SyncRoundStartDuration(plans) => BuffChanges::new(
                origin,
                BuffReplaceResult {
                    refreshed: plans
                        .into_iter()
                        .filter_map(|plan| self.commit_round_start_duration_sync(plan))
                        .collect(),
                    ..Default::default()
                },
            ),
        }
    }

    pub(in crate::engine::manager::buff) fn commit_grant_plan(
        &mut self,
        hp: &HpManager,
        plan: GrantPlan,
    ) -> BuffReplaceResult {
        let transition = plan.transition.clone();
        let initial_act_info_markers = plan
            .initial_act_info_markers
            .clone()
            .or_else(|| plan.initial_act_info.clone());
        let records_transaction_stack_progress = plan.definition.is_stackable_type()
            && (matches!(plan.layer_refresh, Some(LayerRefreshPlan::Update { .. }))
                || matches!(plan.action, GrantAction::Reject(_)));
        let mut immunity_change = plan
            .immunity_action
            .map(|(owner_uid, action)| self.commit_consume_action(owner_uid, action))
            .unwrap_or_default();
        let mut planned_removed = plan
            .excluded_uids
            .iter()
            .flat_map(|uid| self.delete(plan.route.target_uid, *uid))
            .collect::<Vec<_>>();
        planned_removed.extend(
            plan.replacement_uids
                .iter()
                .flat_map(|uid| self.delete(plan.route.target_uid, *uid)),
        );
        for uid in &plan.capacity_eviction_uids {
            planned_removed.extend(self.delete(plan.route.target_uid, *uid).into_iter().map(
                |mut removed| {
                    if removed.buff.uid == Some(*uid) {
                        removed.delete_reason = Some(BuffDeleteReason::Overflow);
                    }
                    removed
                },
            ));
        }
        planned_removed.append(&mut immunity_change.removed);
        for uid in &plan.pre_add_uids {
            super::uid_policy::commit(self, plan.route.target_uid, *uid);
        }
        let layer_refresh_uid = plan
            .layer_refresh_uid
            .map(|uid| super::uid_policy::commit(self, plan.route.target_uid, uid));
        let mut change = self.commit_grant_action(
            hp,
            plan.route,
            &plan.definition,
            plan.args,
            plan.repeat,
            plan.action,
            planned_removed,
            plan.uid,
            &plan.refresh_uids,
            plan.layer_refresh,
            layer_refresh_uid,
            &plan.fanout,
        );
        change
            .fanout
            .extend(self.commit_fanout_refreshes(&plan.fanout_refreshes));
        if records_transaction_stack_progress {
            self.record_transaction_stack_progress(plan.route.buff_id);
        }
        change.refreshed.splice(0..0, immunity_change.refreshed);
        if (plan.initial_params.is_some() || plan.initial_act_info.is_some())
            && let Some(added) = change.added.as_mut()
            && let Some(update) = self.set_state(
                added.target_uid,
                added.buff.uid.unwrap_or_default(),
                None,
                plan.initial_params,
                plan.initial_act_info,
            )
        {
            added.buff = update.after;
            for marker in &mut added.markers {
                marker.effect_num = crate::engine::buff::marker::effect_num(
                    marker.effect_type,
                    added.buff.buff_id.unwrap_or_default(),
                    added.buff.act_common_params.as_deref(),
                );
            }
        }
        if let (Some(added), Some(act_info)) = (change.added.as_mut(), initial_act_info_markers) {
            let target_uid = added.target_uid;
            let buff_uid = added.buff.uid.unwrap_or_default();
            added
                .pre_markers
                .extend(act_info.into_iter().filter_map(|info| {
                    Some(super::BuffActInfoMarkerResult {
                        target_uid,
                        buff_uid,
                        act_id: info.act_id?,
                        params: info.param,
                        str_param: info.str_param,
                        team_type: 0,
                    })
                }));
        }
        let snapshot_buff_uid = change
            .added
            .as_ref()
            .and_then(|added| added.buff.uid)
            .or_else(|| change.refreshed.first().and_then(|update| update.after.uid));
        if let Some(buff_uid) = snapshot_buff_uid {
            self.commit_grant_snapshots(buff_uid, &plan.dot_snapshots);
            self.commit_grant_values(buff_uid, &plan.grant_values);
        }
        if change.added.is_some() {
            for uid in plan.post_add_uids {
                super::uid_policy::commit(self, plan.route.target_uid, uid);
            }
        }
        if let Some(transition) = transition {
            let source_uids = transition.removed_uids.clone();
            let transitioned = self.commit_replace_plan(hp, *transition);
            if transitioned.added.is_some() {
                change.refreshed.retain(|refresh| {
                    refresh
                        .after
                        .uid
                        .is_none_or(|uid| !source_uids.contains(&uid))
                });
                change.removed.extend(transitioned.removed);
                change.refreshed.extend(transitioned.refreshed);
                change.added = transitioned.added;
                change.rejected = transitioned.rejected;
                change.fanout.extend(transitioned.fanout);
            }
        }
        change
    }

    fn commit_replace_plan(&mut self, hp: &HpManager, plan: ReplacePlan) -> BuffReplaceResult {
        let mut removed = Vec::new();
        for buff_uid in plan.removed_uids {
            removed.extend(self.delete(plan.target_uid, buff_uid));
        }
        if removed.is_empty() {
            return BuffReplaceResult::default();
        }
        let mut change = self.commit_grant_plan(hp, plan.grant);
        change.removed.splice(0..0, removed);
        change
    }

    pub(in crate::engine::manager::buff) fn commit_consume_action(
        &mut self,
        target_uid: i64,
        action: ConsumeAction,
    ) -> BuffReplaceResult {
        let mut change = BuffReplaceResult {
            removed: Vec::new(),
            added: None,
            refreshed: Vec::new(),
            rejected: None,
            fanout: Vec::new(),
        };
        match action {
            ConsumeAction::Noop => {}
            ConsumeAction::SetInternalActInfo { buff_uid, act_info } => {
                self.set_state(target_uid, buff_uid, None, None, Some(act_info));
            }
            ConsumeAction::Update {
                buff_uid,
                layer,
                count,
            } => {
                change
                    .refreshed
                    .extend(self.update(target_uid, buff_uid, layer, count, None));
            }
            ConsumeAction::Remove {
                buff_uid,
                layer,
                count,
            } => {
                change.removed = self.delete(target_uid, buff_uid);

                if let Some(removed) = change
                    .removed
                    .iter_mut()
                    .find(|removed| removed.buff.uid == Some(buff_uid))
                {
                    removed.depleted = true;
                    if let Some(layer) = layer {
                        removed.buff.layer = Some(layer);
                    }
                    if let Some(count) = count {
                        removed.buff.count = Some(count);
                    }
                }
            }
        }
        change
    }

    #[cfg(test)]
    pub(crate) fn execute(
        &mut self,
        hp: &HpManager,
        command: BuffCommand,
    ) -> Result<BuffChanges, BuffCommandError> {
        let plan = self.plan(hp, command)?;
        Ok(self.commit(hp, plan))
    }
}
