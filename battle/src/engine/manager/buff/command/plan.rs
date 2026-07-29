use super::*;

impl BuffManager {
    #[cfg(test)]
    pub(super) fn plan(
        &self,
        hp: &HpManager,
        command: BuffCommand,
    ) -> Result<BuffPlan, BuffCommandError> {
        self.plan_with_source_attack(hp, command, None)
    }

    /// Builds a complete mutation plan without changing active state or consuming UIDs.
    /// Cross-manager workflows may validate the plan before committing any state.
    pub(crate) fn plan_with_source_attack(
        &self,
        hp: &HpManager,
        command: BuffCommand,
        source_attack: Option<i32>,
    ) -> Result<BuffPlan, BuffCommandError> {
        let (origin, action) = match command {
            BuffCommand::Grant(grant) => (
                grant.origin,
                BuffPlanAction::Grant(Box::new(self.plan_grant_with_source_attack(
                    hp,
                    (&grant).into(),
                    source_attack,
                )?)),
            ),
            BuffCommand::GrantRelated(related) => {
                let grant = related.grant;
                let (input, force_normal_uid) = match related.relation {
                    BuffGrantRelation::Child => (
                        GrantInput::ChildInstance {
                            layer: grant.amount.unwrap_or_default(),
                            layer_specified: grant.amount.is_some(),
                        },
                        false,
                    ),
                    BuffGrantRelation::Normal => (
                        grant.amount.map_or(GrantInput::Default, GrantInput::Layer),
                        true,
                    ),
                };
                (
                    grant.origin,
                    BuffPlanAction::Grant(Box::new(self.plan_grant_with_source_attack(
                        hp,
                        GrantRequest {
                            source_uid: grant.source_uid,
                            target_uid: grant.target_uid,
                            buff_id: grant.buff_id,
                            input,
                            occurrences: grant.occurrences,
                            child_uid_reservations: grant.child_uid_reservations,
                            force_normal_uid,
                        },
                        source_attack,
                    )?)),
                )
            }
            BuffCommand::GrantIndependent(grant) => (
                grant.origin,
                BuffPlanAction::Grant(Box::new(self.plan_grant_with_source_attack(
                    hp,
                    GrantRequest {
                        source_uid: grant.source_uid,
                        target_uid: grant.target_uid,
                        buff_id: grant.buff_id,
                        input: GrantInput::IndependentInstance {
                            layer: grant.amount.unwrap_or_default(),
                            layer_specified: grant.amount.is_some(),
                        },
                        occurrences: grant.occurrences,
                        child_uid_reservations: grant.child_uid_reservations,
                        force_normal_uid: false,
                    },
                    source_attack,
                )?)),
            ),
            BuffCommand::Accumulate(grant) => {
                let mut plan =
                    self.plan_grant_with_source_attack(hp, (&grant).into(), source_attack)?;
                plan.layer_refresh_uid = None;
                (grant.origin, BuffPlanAction::Accumulate(Box::new(plan)))
            }
            BuffCommand::GrantStateful(grant) => {
                let mut plan = self.plan_grant_with_source_attack(
                    hp,
                    GrantRequest {
                        source_uid: grant.source_uid,
                        target_uid: grant.target_uid,
                        buff_id: grant.buff_id,
                        input: grant.amount.map_or(GrantInput::Default, GrantInput::Layer),
                        occurrences: 1,
                        child_uid_reservations: 0,
                        force_normal_uid: false,
                    },
                    source_attack,
                )?;
                if grant.params.is_some() {
                    plan.initial_params = grant.params;
                }
                if grant.act_info.is_some() {
                    plan.initial_act_info = grant.act_info;
                }
                (grant.origin, BuffPlanAction::Grant(Box::new(plan)))
            }
            BuffCommand::GrantUsingChildUid(grant) => (
                grant.origin,
                BuffPlanAction::Grant(Box::new(self.plan_grant_with_source_attack(
                    hp,
                    GrantRequest {
                        source_uid: grant.source_uid,
                        target_uid: grant.target_uid,
                        buff_id: grant.buff_id,
                        input: GrantInput::ChildUid {
                            layer: grant.amount.unwrap_or_default(),
                            layer_specified: grant.amount.is_some(),
                        },
                        occurrences: grant.occurrences,
                        child_uid_reservations: grant.child_uid_reservations,
                        force_normal_uid: false,
                    },
                    source_attack,
                )?)),
            ),
            BuffCommand::GrantUsingNormalUid(grant) => (
                grant.origin,
                BuffPlanAction::Grant(Box::new(self.plan_grant_with_source_attack(
                    hp,
                    GrantRequest {
                        source_uid: grant.source_uid,
                        target_uid: grant.target_uid,
                        buff_id: grant.buff_id,
                        input: grant.amount.map_or(GrantInput::Default, GrantInput::Layer),
                        occurrences: grant.occurrences,
                        child_uid_reservations: grant.child_uid_reservations,
                        force_normal_uid: true,
                    },
                    source_attack,
                )?)),
            ),
            command @ (BuffCommand::GrantChild(_) | BuffCommand::GrantInternalChild(_)) => {
                let internal = matches!(&command, BuffCommand::GrantInternalChild(_));
                let grant = match command {
                    BuffCommand::GrantChild(grant) | BuffCommand::GrantInternalChild(grant) => {
                        grant
                    }
                    _ => unreachable!(),
                };
                let triggered = grant.origin.domain == RuleDomain::BuffAct
                    && crate::engine::skill::buff_act::registry::reserves_trigger_child_uid(
                        grant.origin.key,
                    );
                let mut plan = self.plan_grant_with_source_attack(
                    hp,
                    GrantRequest {
                        source_uid: grant.source_uid,
                        target_uid: grant.target_uid,
                        buff_id: grant.buff_id,
                        input: if triggered {
                            GrantInput::TriggeredChildInstance {
                                layer: grant.amount.unwrap_or_default(),
                                layer_specified: grant.amount.is_some(),
                            }
                        } else {
                            GrantInput::ChildInstance {
                                layer: grant.amount.unwrap_or_default(),
                                layer_specified: grant.amount.is_some(),
                            }
                        },
                        occurrences: 1,
                        child_uid_reservations: 0,
                        force_normal_uid: false,
                    },
                    source_attack,
                )?;
                if grant.params.is_some() {
                    plan.initial_params = grant.params;
                }
                if grant.act_info.is_some() {
                    plan.initial_act_info = grant.act_info;
                }
                (
                    grant.origin,
                    if internal {
                        BuffPlanAction::GrantInternalChild(Box::new(plan))
                    } else {
                        BuffPlanAction::Grant(Box::new(plan))
                    },
                )
            }
            BuffCommand::Consume(consume) => (
                consume.origin,
                BuffPlanAction::Consume(self.plan_consume(
                    consume,
                    false,
                    ConsumeField::Configured,
                )?),
            ),
            BuffCommand::ConsumeCount(consume) => (
                consume.origin,
                BuffPlanAction::Consume(self.plan_consume(consume, false, ConsumeField::Count)?),
            ),
            BuffCommand::ConsumeEffectCount(consume) => (
                consume.origin,
                BuffPlanAction::ConsumeEffectCount(self.plan_consume(
                    consume,
                    false,
                    ConsumeField::Count,
                )?),
            ),
            BuffCommand::ConsumeCoalesced(consume) => (
                consume.origin,
                BuffPlanAction::Consume(self.plan_consume(
                    consume,
                    true,
                    ConsumeField::Configured,
                )?),
            ),
            BuffCommand::Convert(convert) => {
                if convert.source_uid == 0
                    || convert.target_uid == 0
                    || convert.source_buff_id <= 0
                    || convert.output_buff_id <= 0
                {
                    return Err(BuffCommandError::InvalidConsume);
                }
                let consume = self.plan_consume(
                    BuffConsume {
                        origin: convert.origin,
                        target_uid: convert.source_uid,
                        selector: BuffSelector::ExactId(convert.source_buff_id),
                        amount: 1,
                        depleted: DepletedBuff::Remove,
                    },
                    false,
                    ConsumeField::Configured,
                )?;
                let has_source = consume
                    .actions
                    .iter()
                    .any(|action| !matches!(action, ConsumeAction::Noop));
                let grant = has_source
                    .then(|| {
                        self.plan_grant_with_source_attack(
                            hp,
                            GrantRequest {
                                source_uid: convert.source_uid,
                                target_uid: convert.target_uid,
                                buff_id: convert.output_buff_id,
                                input: GrantInput::Default,
                                occurrences: 1,
                                child_uid_reservations: 0,
                                force_normal_uid: false,
                            },
                            source_attack,
                        )
                    })
                    .transpose()?;
                (
                    convert.origin,
                    BuffPlanAction::Convert(Box::new(ConvertPlan { consume, grant })),
                )
            }
            BuffCommand::Replace(replace) => (
                replace.origin,
                BuffPlanAction::Replace(Box::new(self.plan_replace(hp, replace, source_attack)?)),
            ),
            BuffCommand::Remove(remove) => (
                remove.origin,
                BuffPlanAction::Remove(self.plan_remove(
                    remove,
                    remove.origin.key.opcode,
                    false,
                    false,
                )?),
            ),
            BuffCommand::RemoveAfterTrigger(remove) => (
                remove.origin,
                BuffPlanAction::Remove(self.plan_remove(remove, 0, false, false)?),
            ),
            BuffCommand::Deactivate(remove) => (
                remove.origin,
                BuffPlanAction::Remove(self.plan_remove(remove, 0, true, false)?),
            ),
            BuffCommand::ExpireAction(remove) => (
                remove.origin,
                BuffPlanAction::Remove(self.plan_remove(remove, 0, true, true)?),
            ),
            BuffCommand::Dispel(dispel) => (
                dispel.origin,
                BuffPlanAction::Remove(self.plan_dispel(dispel)?),
            ),
            BuffCommand::SetAmount(update) => (
                update.origin,
                BuffPlanAction::SetAmount(self.plan_set_amount(update)?),
            ),
            BuffCommand::SetState(update) => (
                update.origin,
                BuffPlanAction::SetState(self.plan_set_state(update)?),
            ),
            BuffCommand::SetInternalState(update) => (
                update.origin,
                BuffPlanAction::SetInternalState(self.plan_set_state(update)?),
            ),
            BuffCommand::SetStateSnapshot(update) => (
                update.origin,
                BuffPlanAction::SetStateSnapshot(self.plan_set_state(update)?),
            ),
            BuffCommand::AccumulateActValue(update) => {
                if update.target_uid == 0
                    || update.buff_uid == 0
                    || update.act_id <= 0
                    || update.delta == 0
                    || self.snapshot(update.target_uid, update.buff_uid).is_none()
                {
                    return Err(BuffCommandError::InvalidSetState);
                }
                (update.origin, BuffPlanAction::AccumulateActValue(update))
            }
            BuffCommand::ChangeDuration(update) => {
                let selector_valid = match update.selector {
                    BuffSelector::IdOrType(value)
                    | BuffSelector::ExactId(value)
                    | BuffSelector::TypeId(value) => value > 0,
                    BuffSelector::Uid(value) => value > 0,
                };
                if update.target_uid == 0 || !selector_valid || update.delta == 0 {
                    return Err(BuffCommandError::InvalidDurationChange);
                }
                let plans = self
                    .buffs
                    .iter()
                    .filter(|active| {
                        active.owner_uid == update.target_uid
                            && Self::matches_selector(active, update.selector)
                    })
                    .filter_map(|active| {
                        Some(DurationChangePlan {
                            target_uid: update.target_uid,
                            buff_uid: active.buff.uid?,
                            duration: active
                                .buff
                                .duration
                                .unwrap_or_default()
                                .saturating_add(update.delta)
                                .max(0),
                        })
                    })
                    .collect();
                (update.origin, BuffPlanAction::ChangeDuration(plans))
            }
            BuffCommand::AddSpecialCount(update) => {
                if update.target_uid == 0
                    || update.count <= 0
                    || !update.marker_ids.iter().any(|id| *id > 0)
                {
                    return Err(BuffCommandError::InvalidSpecialCount);
                }
                (
                    update.origin,
                    BuffPlanAction::AddSpecialCount(SpecialCountPlan {
                        target_uid: update.target_uid,
                        marker_ids: update.marker_ids,
                        count: update.count,
                    }),
                )
            }
            BuffCommand::ReserveChildUids(reservation) => {
                if reservation.target_uid == 0 || reservation.count <= 0 {
                    return Err(BuffCommandError::InvalidUidReservation);
                }
                (
                    reservation.origin,
                    BuffPlanAction::ReserveChildUids(UidReservationPlan {
                        target_uid: reservation.target_uid,
                        uids: super::uid_policy::children(
                            self,
                            reservation.target_uid,
                            reservation.count,
                        ),
                    }),
                )
            }
            BuffCommand::ReserveGrantUid(reservation) => {
                if reservation.target_uid == 0 || reservation.buff_id <= 0 {
                    return Err(BuffCommandError::InvalidUidReservation);
                }
                BuffDefinition::get(reservation.buff_id)
                    .ok_or(BuffCommandError::MissingDefinition(reservation.buff_id))?;
                let uid = super::uid_policy::children(self, reservation.target_uid, 1)[0];
                (
                    reservation.origin,
                    BuffPlanAction::ReserveGrantUid(GrantUidReservationPlan {
                        target_uid: reservation.target_uid,
                        buff_id: reservation.buff_id,
                        uid,
                    }),
                )
            }
            BuffCommand::AdvanceDuration(advance) => {
                if advance.origin.domain != RuleDomain::EffectTime
                    || advance.origin.key.opcode != advance.take_stage
                {
                    return Err(BuffCommandError::InvalidDurationAdvance);
                }
                let plans = if let Some(buff_uids) = &advance.buff_uids {
                    self.plan_duration_advances_for_snapshot(
                        advance.take_stage,
                        &advance.owner_uids,
                        buff_uids,
                    )
                } else {
                    self.plan_duration_advances(advance.take_stage, &advance.owner_uids)
                };
                (advance.origin, BuffPlanAction::AdvanceDuration(plans))
            }
            BuffCommand::SyncRoundStartDuration(sync) => {
                if sync.origin.domain != RuleDomain::Lifecycle
                    || sync.origin.key != ROUND_START_DURATION_SYNC_KEY
                {
                    return Err(BuffCommandError::InvalidLifecycle);
                }
                (
                    sync.origin,
                    BuffPlanAction::SyncRoundStartDuration(
                        self.plan_round_start_duration_sync(&sync.owner_uids),
                    ),
                )
            }
            BuffCommand::CleanupRoundStart(cleanup) => {
                if cleanup.origin.domain != RuleDomain::Lifecycle
                    || cleanup.origin.key != ROUND_START_CLEANUP_KEY
                {
                    return Err(BuffCommandError::InvalidLifecycle);
                }
                (
                    cleanup.origin,
                    BuffPlanAction::CleanupRoundStart(self.plan_round_start_cleanup()),
                )
            }
        };
        Ok(BuffPlan { origin, action })
    }

    #[cfg(test)]
    pub(in crate::engine::manager::buff) fn plan_grant(
        &self,
        hp: &HpManager,
        request: GrantRequest,
    ) -> Result<GrantPlan, BuffCommandError> {
        self.plan_grant_with_source_attack(hp, request, None)
    }

    fn plan_grant_with_source_attack(
        &self,
        hp: &HpManager,
        request: GrantRequest,
        source_attack: Option<i32>,
    ) -> Result<GrantPlan, BuffCommandError> {
        if request.buff_id <= 0
            || request.target_uid == 0
            || matches!(
                request.input,
                GrantInput::Layer(value)
                    | GrantInput::ChildUid { layer: value, .. }
                    | GrantInput::IndependentInstance { layer: value, .. }
                    | GrantInput::ChildInstance { layer: value, .. }
                    | GrantInput::TriggeredChildInstance { layer: value, .. }
                    | GrantInput::UnconditionalLayer(value)
                    if value < 0
            )
            || request.occurrences == 0
        {
            return Err(BuffCommandError::InvalidGrant);
        }
        let definition = BuffDefinition::get(request.buff_id)
            .ok_or(BuffCommandError::MissingDefinition(request.buff_id))?;
        let occurrences = i32::try_from(request.occurrences)
            .map_err(|_| BuffCommandError::UnsupportedOccurrences(request.occurrences))?;
        let args = match request.input {
            GrantInput::ChildUid {
                layer,
                layer_specified,
            }
            | GrantInput::IndependentInstance {
                layer,
                layer_specified,
            }
            | GrantInput::ChildInstance {
                layer,
                layer_specified,
            }
            | GrantInput::TriggeredChildInstance {
                layer,
                layer_specified,
            } => {
                if definition.uses_stack_layer() {
                    BuffAddArgs {
                        layer,
                        count: 0,
                        layer_specified,
                    }
                } else {
                    BuffAddArgs::count(0)
                }
            }
            GrantInput::UnconditionalLayer(layer) => BuffAddArgs::layer(layer),
            GrantInput::Default | GrantInput::Layer(_) => {
                let base_amount = match request.input {
                    GrantInput::Default => 1,
                    GrantInput::Layer(amount) => amount,
                    GrantInput::ChildUid { .. }
                    | GrantInput::IndependentInstance { .. }
                    | GrantInput::ChildInstance { .. }
                    | GrantInput::TriggeredChildInstance { .. }
                    | GrantInput::UnconditionalLayer(_) => unreachable!(),
                };
                let amount = base_amount.checked_mul(occurrences).ok_or(
                    BuffCommandError::UnsupportedOccurrences(request.occurrences),
                )?;
                let layer_specified =
                    matches!(request.input, GrantInput::Layer(_)) || request.occurrences > 1;

                BuffAddArgs {
                    layer: if layer_specified { amount } else { 0 },
                    count: 0,
                    layer_specified,
                }
            }
        };
        let route = BuffRoute::new(request.source_uid, request.target_uid, request.buff_id);
        let policy = BuffPolicy::try_for_buff_id(request.buff_id)
            .map_err(BuffCommandError::InvalidPolicy)?;
        let unconditional = matches!(
            request.input,
            GrantInput::IndependentInstance { .. }
                | GrantInput::ChildInstance { .. }
                | GrantInput::TriggeredChildInstance { .. }
                | GrantInput::UnconditionalLayer(_)
        );
        let configured_blocker = (!unconditional)
            .then(|| self.blocking_buff_id(request.target_uid, request.buff_id, &definition))
            .flatten();
        let immunity = (!unconditional && configured_blocker.is_none())
            .then(|| self.immunity_blocker(request.target_uid, definition.status))
            .flatten();
        let blocker =
            configured_blocker.or_else(|| immunity.as_ref().map(|(buff_id, _, _)| *buff_id));
        let blocked = blocker.is_some();
        let mut excluded_uids = if blocked || unconditional {
            Vec::new()
        } else {
            policy
                .exclusions
                .remove_on_grant
                .iter()
                .flat_map(|excluded_id| {
                    self.buffs.iter().filter_map(|active| {
                        (active.owner_uid == request.target_uid
                            && active.buff.buff_id == Some(*excluded_id))
                        .then_some(active.buff.uid.unwrap_or_default())
                    })
                })
                .chain(self.buffs.iter().filter_map(|active| {
                    (active.owner_uid == request.target_uid
                        && active.definition.as_ref().is_some_and(|resident| {
                            policy
                                .exclusions
                                .remove_statuses_on_grant
                                .contains(&resident.status_id)
                        }))
                    .then_some(active.buff.uid.unwrap_or_default())
                }))
                .collect()
        };
        excluded_uids.sort_unstable();
        excluded_uids.dedup();
        let stack_layer = self.grant_layer(route, &definition, args);
        let repeated_stack = !self.shared_uid_lane
            && request.occurrences > 1
            && definition.is_stackable_type()
            && stack_layer > 1
            && !self.has_buff_id(request.target_uid, request.buff_id)
            && !blocked;
        let stack_reserve_before = if repeated_stack {
            stack_layer.saturating_sub(2)
        } else {
            0
        };
        let configured_reserve = i32::try_from(request.child_uid_reservations).map_err(|_| {
            BuffCommandError::UnsupportedOccurrences(request.child_uid_reservations)
        })?;
        let mut reserve_before = configured_reserve
            .checked_add(
                if matches!(request.input, GrantInput::TriggeredChildInstance { .. }) {
                    1
                } else {
                    stack_reserve_before
                },
            )
            .ok_or(BuffCommandError::UnsupportedOccurrences(
                request.child_uid_reservations,
            ))?;
        if args.layer_specified
            && stack_layer > 0
            && !repeated_stack
            && policy.uid.reserve_before_explicit_layer_apply
            && policy.uid.allocation.uses_child_for_apply(args)
            && !super::uid_policy::last_was_child(self, route.target_uid)
        {
            reserve_before =
                reserve_before
                    .checked_add(1)
                    .ok_or(BuffCommandError::UnsupportedOccurrences(
                        request.child_uid_reservations,
                    ))?;
        }
        let repeat = if matches!(
            request.input,
            GrantInput::IndependentInstance { .. }
                | GrantInput::ChildInstance { .. }
                | GrantInput::TriggeredChildInstance { .. }
                | GrantInput::UnconditionalLayer(_)
        ) {
            0
        } else {
            typed_count_repeat(&definition, args.layer, args.layer_specified, args.count)
        };
        let action = if unconditional {
            GrantAction::Add
        } else if let Some(blocker) = blocker {
            GrantAction::Reject(blocker)
        } else {
            let action = self.resolve_grant_action(route, &definition, &policy, args, repeat);
            if matches!(request.input, GrantInput::ChildUid { .. })
                && action == GrantAction::ReplaceExisting
            {
                GrantAction::RefreshExisting
            } else {
                action
            }
        };
        let capacity_eviction_uids = if matches!(action, GrantAction::Add)
            && let Some(capacity) = policy.shared_group_capacity
        {
            let mut uids = self
                .buffs
                .iter()
                .filter(|active| active.owner_uid == route.target_uid)
                .filter(|active| {
                    active.definition.as_ref().is_some_and(|resident| {
                        resident
                            .shared_group_capacity()
                            .is_some_and(|(group_id, _)| group_id == capacity.group_id)
                    })
                })
                .filter_map(|active| active.buff.uid)
                .collect::<Vec<_>>();
            uids.sort_unstable();
            let remove_count = uids
                .len()
                .saturating_add(1)
                .saturating_sub(capacity.max_instances as usize);
            uids.into_iter().take(remove_count).collect()
        } else {
            Vec::new()
        };
        let readds_prior_instance = action == GrantAction::Add
            && self
                .added_history_for_owner(route.target_uid)
                .contains(&route.buff_id);
        if reserve_before == 0
            && !super::uid_policy::last_was_child(self, route.target_uid)
            && (readds_prior_instance || action == GrantAction::ReplaceExisting)
            && policy.uid.reserve_before_reapply
        {
            reserve_before =
                reserve_before
                    .checked_add(1)
                    .ok_or(BuffCommandError::UnsupportedOccurrences(
                        request.child_uid_reservations,
                    ))?;
        }
        let uid_reserve_before = reserve_before;
        let normal_reserve_before = if action == GrantAction::ReplaceExisting
            || (readds_prior_instance && !unconditional)
        {
            policy.uid.normal_reservations_before_reapply
        } else {
            0
        };
        let uid = match action {
            GrantAction::Reject(_) => Some(super::uid_policy::plan(
                self,
                route,
                &definition,
                policy.uid.allocation.uses_child_for_apply(args),
                0,
                0,
            )),
            GrantAction::RetainEnhancedVariant => Some(super::uid_policy::plan(
                self,
                route,
                &definition,
                policy.uid.allocation.uses_child_for_apply(args),
                0,
                0,
            )),
            GrantAction::Add | GrantAction::ReplaceExisting => Some(super::uid_policy::plan(
                self,
                route,
                &definition,
                matches!(
                    request.input,
                    GrantInput::ChildUid { .. }
                        | GrantInput::ChildInstance { .. }
                        | GrantInput::TriggeredChildInstance { .. }
                ) || (!request.force_normal_uid
                    && policy.uid.allocation.uses_child_for_apply(args)),
                normal_reserve_before,
                reserve_before,
            )),
            _ => None,
        };
        let pre_add_uids = super::uid_policy::reservations(
            self,
            route.target_uid,
            normal_reserve_before,
            uid_reserve_before,
        );
        let fanout_specs = if matches!(action, GrantAction::Add | GrantAction::ReplaceExisting) {
            self.fanout_specs(
                hp,
                route.target_uid,
                route.buff_id,
                stack_layer,
                definition.count(args.count),
                policy.lifetime.duration,
            )
        } else {
            Vec::new()
        };
        let fanout_uids = uid
            .map(|root| {
                super::uid_policy::children_after_sequence(
                    self,
                    route.target_uid,
                    0,
                    pre_add_uids.iter().copied().chain(std::iter::once(root)),
                    fanout_specs.len() as i32,
                )
            })
            .unwrap_or_default();
        let fanout = fanout_specs
            .into_iter()
            .zip(fanout_uids)
            .map(|(spec, uid)| PlannedFanout { spec, uid })
            .collect::<Vec<_>>();
        let refresh_uids = match action {
            GrantAction::RefreshCount if definition.reserves_normal_uid_on_count_refresh() => {
                super::uid_policy::counted_refreshes(self, route.target_uid, repeat)
            }
            GrantAction::RefreshCount if definition.reserves_child_uid_on_count_refresh() => {
                super::uid_policy::children(self, route.target_uid, repeat)
            }
            GrantAction::Add if definition.uses_typed_count() && repeat > 1 => {
                super::uid_policy::children_after_sequence(
                    self,
                    route.target_uid,
                    0,
                    pre_add_uids
                        .iter()
                        .copied()
                        .chain(uid)
                        .chain(fanout.iter().map(|plan| plan.uid)),
                    repeat - 1,
                )
            }
            _ => Vec::new(),
        };
        let layer_refresh = (action == GrantAction::RefreshLayer)
            .then(|| self.plan_layer_refresh(route, &definition, &policy, args))
            .flatten();
        let partially_capped_layer_refresh = layer_refresh.is_some_and(|refresh| {
            self.partially_caps_layer_refresh(route, &definition, &policy, args, refresh)
        });
        let layer_refresh_uid =
            (matches!(
                layer_refresh,
                Some(LayerRefreshPlan::PromoteRestored { .. })
            ) || (matches!(layer_refresh, Some(LayerRefreshPlan::Update { .. }))
                && (policy.uid.reserve_on_layer_refresh || partially_capped_layer_refresh))
                || (action == GrantAction::RefreshLayer
                    && matches!(layer_refresh, Some(LayerRefreshPlan::NoChange))
                    && definition.is_stackable_type()
                    && self.transaction_has_stack_progress(route.buff_id)))
            .then(|| {
                super::uid_policy::plan(
                    self,
                    route,
                    &definition,
                    policy.uid.allocation.uses_child_for_apply(args),
                    0,
                    uid_reserve_before,
                )
            });
        let fanout_refreshes = match layer_refresh {
            Some(LayerRefreshPlan::Update {
                buff_uid,
                next_layer,
                next_duration,
            }) => self.fanout_refresh_specs(
                hp,
                route.target_uid,
                route.buff_id,
                buff_uid,
                next_layer,
                next_duration,
            ),
            Some(LayerRefreshPlan::PromoteRestored {
                next_layer,
                next_duration,
                ..
            }) => layer_refresh_uid
                .map(|uid| {
                    self.fanout_refresh_specs(
                        hp,
                        route.target_uid,
                        route.buff_id,
                        uid.uid,
                        next_layer,
                        next_duration,
                    )
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let reserve_after_add = repeated_stack
            || (action == GrantAction::Add
                && policy.uid.reserve_after_first_apply
                && stack_layer > 0);
        let post_add_normal_uids = if action == GrantAction::Add && self.shared_uid_lane {
            super::uid_policy::normals_after_sequence(
                self,
                route.target_uid,
                pre_add_uids
                    .iter()
                    .copied()
                    .chain(uid)
                    .chain(fanout.iter().map(|plan| plan.uid))
                    .chain(refresh_uids.iter().copied()),
                policy.uid.normal_reservations_after_first_apply,
            )
        } else {
            Vec::new()
        };
        let mut post_add_uids = post_add_normal_uids;
        if reserve_after_add {
            let child_uids = super::uid_policy::children_after_sequence(
                self,
                route.target_uid,
                0,
                pre_add_uids
                    .iter()
                    .copied()
                    .chain(uid)
                    .chain(fanout.iter().map(|plan| plan.uid))
                    .chain(refresh_uids.iter().copied())
                    .chain(post_add_uids.iter().copied()),
                if repeated_stack { 2 } else { 1 },
            );
            post_add_uids.extend(child_uids);
        }
        let dot_snapshots =
            Self::plan_grant_snapshots(&definition, route.source_uid, source_attack, args);
        let grant_values = self.plan_grant_values(&definition, route.source_uid);
        let initial_act_info = definition.initial_grant_value_act_info(&grant_values);
        let initial_params = self.plan_grant_params(&definition, route.source_uid);
        let replacement_uids = if action == GrantAction::ReplaceExisting {
            self.buffs
                .iter()
                .filter(|active| policy.matches(active, route))
                .filter_map(|active| active.buff.uid)
                .collect()
        } else {
            Vec::new()
        };

        let mut plan = GrantPlan {
            route,
            definition,
            args,
            repeat,
            action,
            excluded_uids,
            replacement_uids,
            capacity_eviction_uids,
            pre_add_uids,
            uid,
            refresh_uids,
            layer_refresh_uid,
            layer_refresh,
            fanout,
            fanout_refreshes,
            post_add_uids,
            initial_params,
            initial_act_info,
            initial_act_info_markers: None,
            dot_snapshots,
            grant_values,
            immunity_action: immunity.map(|(_, owner_uid, action)| (owner_uid, action)),
            transition: None,
        };
        if crate::engine::diagnostics::enabled(crate::engine::diagnostics::TraceArea::Buff) {
            eprintln!(
                "buff uid plan source={} target={} buff={} action={:?} pre={:?} visible={:?} refresh={:?} layer_refresh={:?} fanout={:?} post={:?}",
                route.source_uid,
                route.target_uid,
                route.buff_id,
                plan.action,
                plan.pre_add_uids,
                plan.uid,
                plan.refresh_uids,
                plan.layer_refresh_uid,
                plan.fanout
                    .iter()
                    .map(|fanout| (fanout.spec.route, fanout.uid))
                    .collect::<Vec<_>>(),
                plan.post_add_uids,
            );
        }
        if let Some((threshold, replacement_buff_id)) = plan.definition.stack_transition()
            && threshold > 0
            && replacement_buff_id > 0
        {
            let mut projected = self.clone();
            projected.commit_grant_plan(hp, plan.clone());
            let reached = projected
                .buffs
                .iter()
                .find(|active| {
                    active.owner_uid == route.target_uid
                        && active.buff.buff_id == Some(route.buff_id)
                })
                .is_some_and(|active| super::count_or_layer(&active.buff) >= threshold);
            if reached {
                plan.transition = Some(Box::new(projected.plan_replace_ids(
                    hp,
                    route.source_uid,
                    route.target_uid,
                    route.buff_id,
                    replacement_buff_id,
                    source_attack,
                )?));
            }
        }
        Ok(plan)
    }

    fn plan_consume(
        &self,
        consume: BuffConsume,
        coalesced: bool,
        field: ConsumeField,
    ) -> Result<ConsumePlan, BuffCommandError> {
        let selector = consume.selector;
        let selector_valid = match selector {
            BuffSelector::IdOrType(value) => value > 0,
            BuffSelector::ExactId(value) => value > 0,
            BuffSelector::TypeId(value) => value > 0,
            BuffSelector::Uid(value) => value > 0,
        };
        if consume.target_uid == 0 || !selector_valid || consume.amount <= 0 {
            return Err(BuffCommandError::InvalidConsume);
        }
        let actions = if coalesced {
            let mut remaining = consume.amount;
            self.buffs
                .iter()
                .filter(|active| {
                    active.owner_uid == consume.target_uid
                        && Self::matches_selector(active, selector)
                })
                .filter_map(|active| {
                    if remaining <= 0 {
                        return None;
                    }
                    let consumed = remaining.min(super::count_or_layer(&active.buff).max(0));
                    if consumed <= 0 {
                        return None;
                    }
                    remaining -= consumed;
                    Some(Self::resolve_active_consume_action(
                        active,
                        consumed,
                        consume.depleted,
                        field,
                    ))
                })
                .collect()
        } else {
            vec![self.resolve_consume_action(
                consume.target_uid,
                selector,
                consume.amount,
                consume.depleted,
                field,
            )]
        };
        Ok(ConsumePlan {
            target_uid: consume.target_uid,
            actions,
        })
    }

    fn plan_replace(
        &self,
        hp: &HpManager,
        replace: BuffReplace,
        source_attack: Option<i32>,
    ) -> Result<ReplacePlan, BuffCommandError> {
        let BuffSelector::IdOrType(source_id_or_type) = replace.source else {
            return Err(BuffCommandError::InvalidReplace);
        };
        self.plan_replace_ids(
            hp,
            replace.source_uid,
            replace.target_uid,
            source_id_or_type,
            replace.replacement_id_or_type,
            source_attack,
        )
    }

    fn plan_replace_ids(
        &self,
        hp: &HpManager,
        source_uid: i64,
        target_uid: i64,
        source_id_or_type: i32,
        replacement_id_or_type: i32,
        source_attack: Option<i32>,
    ) -> Result<ReplacePlan, BuffCommandError> {
        if target_uid == 0 || source_id_or_type <= 0 || replacement_id_or_type <= 0 {
            return Err(BuffCommandError::InvalidReplace);
        }
        let source_buff_id = self
            .buffs
            .iter()
            .find(|active| {
                active.owner_uid == target_uid
                    && (active.type_id == source_id_or_type
                        || active.buff.buff_id == Some(source_id_or_type))
            })
            .and_then(|active| active.buff.buff_id)
            .ok_or(BuffCommandError::InvalidReplace)?;
        let grant = self.plan_grant_with_source_attack(
            hp,
            GrantRequest {
                source_uid,
                target_uid,
                buff_id: replacement_id_or_type,
                input: GrantInput::UnconditionalLayer(1),
                occurrences: 1,
                child_uid_reservations: 0,
                force_normal_uid: false,
            },
            source_attack,
        )?;
        let removed_uids = self
            .buffs
            .iter()
            .filter(|active| {
                active.owner_uid == target_uid && active.buff.buff_id == Some(source_buff_id)
            })
            .filter_map(|active| active.buff.uid)
            .collect();
        Ok(ReplacePlan {
            target_uid,
            removed_uids,
            grant,
        })
    }

    fn plan_remove(
        &self,
        remove: BuffRemove,
        config_effect: i32,
        clear_amount: bool,
        clear_count: bool,
    ) -> Result<RemovePlan, BuffCommandError> {
        let valid = match remove.selector {
            BuffRemoveSelector::Uid(buff_uid) => buff_uid > 0,
            BuffRemoveSelector::ExactId(buff_id) => buff_id > 0,
            BuffRemoveSelector::IdOrType(id) => id > 0,
        };
        if remove.target_uid == 0 || !valid {
            return Err(BuffCommandError::InvalidRemove);
        }
        let buff_uids = self
            .buffs
            .iter()
            .filter(|active| {
                active.owner_uid == remove.target_uid
                    && match remove.selector {
                        BuffRemoveSelector::Uid(buff_uid) => active.buff.uid == Some(buff_uid),
                        BuffRemoveSelector::ExactId(buff_id) => {
                            active.buff.buff_id == Some(buff_id)
                        }
                        BuffRemoveSelector::IdOrType(id) => {
                            active.buff.buff_id == Some(id) || active.type_id == id
                        }
                    }
            })
            .filter_map(|active| active.buff.uid)
            .collect();
        Ok(RemovePlan {
            target_uid: remove.target_uid,
            buff_uids,
            config_effect,
            clear_amount,
            clear_count,
        })
    }

    fn plan_dispel(&self, dispel: BuffDispel) -> Result<RemovePlan, BuffCommandError> {
        if dispel.target_uid == 0
            || dispel.count < 0
            || dispel.statuses.is_empty()
            || dispel.statuses.contains(&super::BuffStatus::Unknown)
        {
            return Err(BuffCommandError::InvalidRemove);
        }
        let limit = usize::try_from(dispel.count).unwrap_or_default();
        let buff_uids = self
            .buffs
            .iter()
            .filter(|active| {
                active.owner_uid == dispel.target_uid
                    && !dispel
                        .excluded_ids_or_types
                        .iter()
                        .any(|id| active.buff.buff_id == Some(*id) || active.type_id == *id)
                    && active
                        .definition
                        .as_ref()
                        .is_some_and(|definition| dispel.statuses.contains(&definition.status))
            })
            .filter_map(|active| active.buff.uid)
            .take(if limit == 0 { usize::MAX } else { limit })
            .collect();
        Ok(RemovePlan {
            target_uid: dispel.target_uid,
            buff_uids,
            config_effect: dispel.origin.key.opcode,
            clear_amount: false,
            clear_count: false,
        })
    }

    fn plan_set_amount(&self, update: BuffSetAmount) -> Result<SetAmountPlan, BuffCommandError> {
        let value = match update.amount {
            BuffAmount::Layer(value) | BuffAmount::Count(value) => value,
        };
        if update.target_uid == 0 || update.buff_uid <= 0 || value < 0 {
            return Err(BuffCommandError::InvalidSetAmount);
        }
        let exists = self.buffs.iter().any(|active| {
            active.owner_uid == update.target_uid && active.buff.uid == Some(update.buff_uid)
        });
        Ok(SetAmountPlan {
            target_uid: update.target_uid,
            buff_uid: update.buff_uid,
            amount: update.amount,
            exists,
        })
    }

    fn plan_set_state(&self, update: BuffSetState) -> Result<SetStatePlan, BuffCommandError> {
        if update.target_uid == 0
            || update.buff_uid <= 0
            || (update.ex_info.is_none() && update.params.is_none() && update.act_info.is_none())
        {
            return Err(BuffCommandError::InvalidSetState);
        }
        let exists = self.buffs.iter().any(|active| {
            active.owner_uid == update.target_uid && active.buff.uid == Some(update.buff_uid)
        });
        Ok(SetStatePlan {
            target_uid: update.target_uid,
            buff_uid: update.buff_uid,
            ex_info: update.ex_info,
            params: update.params,
            act_info: update.act_info,
            exists,
        })
    }

    pub(in crate::engine::manager::buff) fn resolve_consume_action(
        &self,
        target_uid: i64,
        selector: BuffSelector,
        amount: i32,
        depleted: DepletedBuff,
        field: ConsumeField,
    ) -> ConsumeAction {
        let Some(active) = self.buffs.iter().find(|active| {
            active.owner_uid == target_uid && Self::matches_selector(active, selector)
        }) else {
            return ConsumeAction::Noop;
        };
        Self::resolve_active_consume_action(active, amount, depleted, field)
    }

    fn matches_selector(active: &super::ActiveBuff, selector: BuffSelector) -> bool {
        match selector {
            BuffSelector::IdOrType(value) => {
                active.type_id == value || active.buff.buff_id == Some(value)
            }
            BuffSelector::ExactId(value) => active.buff.buff_id == Some(value),
            BuffSelector::TypeId(value) => active.type_id == value,
            BuffSelector::Uid(value) => active.buff.uid == Some(value),
        }
    }

    pub(in crate::engine::manager::buff) fn resolve_active_consume_action(
        active: &super::ActiveBuff,
        amount: i32,
        depleted: DepletedBuff,
        field: ConsumeField,
    ) -> ConsumeAction {
        let uses_stack_layer = active
            .definition
            .as_ref()
            .is_some_and(BuffDefinition::uses_stack_layer);
        let uses_typed_count = active
            .definition
            .as_ref()
            .is_some_and(BuffDefinition::uses_typed_count);
        let mut layer = None;
        let mut count = None;
        let consume_count = field == ConsumeField::Count
            || (!uses_stack_layer && active.buff.count.unwrap_or_default() > 0);
        let next_amount = if !consume_count {
            let next = active
                .buff
                .layer
                .unwrap_or_default()
                .saturating_sub(amount.max(0));
            layer = Some(next);
            next
        } else {
            let next = active
                .buff
                .count
                .unwrap_or_default()
                .saturating_sub(amount.max(0));
            count = Some(next);
            next
        };
        let buff_uid = active.buff.uid.unwrap_or_default();
        if next_amount <= 0
            && matches!(depleted, DepletedBuff::Remove)
            && (field == ConsumeField::Count || uses_stack_layer || uses_typed_count)
        {
            ConsumeAction::Remove {
                buff_uid,
                layer: layer.filter(|_| {
                    active
                        .definition
                        .as_ref()
                        .is_some_and(BuffDefinition::clears_layer_on_depletion)
                }),
                count,
            }
        } else {
            ConsumeAction::Update {
                buff_uid,
                layer,
                count,
            }
        }
    }
}
