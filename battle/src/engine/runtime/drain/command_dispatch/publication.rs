pub(super) mod before;

use super::*;

#[derive(Clone, Copy)]
pub(super) struct Scope<'a> {
    pub(super) context: TargetContext,
    pub(super) trigger: &'a SkillOpTrigger,
    pub(super) frame_path: &'a FramePath,
    pub(super) event_scope: &'a FramePath,
    pub(super) action_scope: Option<&'a FramePath>,
    pub(super) entry_halo_scope: &'a FramePath,
    pub(super) field_transfer_owner: Option<&'a [usize]>,
    pub(super) field_level_grant: Option<&'a super::super::field_level::Grant>,
    pub(super) current_skill: Option<(i64, i32, Option<i64>)>,
    pub(super) fanout_output_owner: OutputOwner,
    pub(super) layer_halo_fanout_output_owner: OutputOwner,
}

pub(super) fn record(
    access: DrainAccess<'_>,
    scope: Scope<'_>,
    outcome: &RuleOutcome,
    replacement: Option<&BuffReplacementParts>,
    events: &[BattleEvent],
    fanout: Vec<BuffFanoutResult>,
) -> Result<(), DrainError> {
    let DrainAccess {
        managers,
        pool,
        catalog,
        determinism,
        result,
        state,
    } = access;
    let Scope {
        context,
        trigger,
        frame_path,
        event_scope,
        action_scope,
        entry_halo_scope,
        current_skill,
        fanout_output_owner,
        layer_halo_fanout_output_owner,
        ..
    } = scope;
    let (leading_fanout, fanout): (Vec<_>, Vec<_>) = fanout
        .into_iter()
        .partition(|fanout| fanout.precedes_carrier_change());
    for fanout in leading_fanout {
        let parent = output_frame_path(layer_halo_fanout_output_owner, event_scope);
        if fanout.allies.iter().all(|ally| ally.replaced.is_empty()) {
            push_fanout(&mut result.frames, &parent, fanout, frame_trigger(trigger));
            continue;
        }
        let fanout_path =
            push_fanout_frame(&mut result.frames, &parent, &fanout, frame_trigger(trigger));
        let mut lead = fanout;
        let allies = std::mem::take(&mut lead.allies);
        for ally in allies {
            let part = |publish_part| {
                let mut part = lead.clone();
                let mut ally = ally.clone();
                ally.part = publish_part;
                part.allies = vec![ally];
                crate::engine::runtime::change::BattleChange::BuffFanout(Box::new(part))
            };
            if ally.replaced.is_empty() {
                push_change(
                    &mut result.frames,
                    &fanout_path,
                    part(crate::engine::manager::buff::BuffPublishPart::Whole),
                );
                continue;
            }
            push_change(
                &mut result.frames,
                &fanout_path,
                part(crate::engine::manager::buff::BuffPublishPart::Lead),
            );
            let removal_events = events
                .iter()
                .filter(|event| {
                    matches!(event, BattleEvent::BuffRemoved(change)
                        if ally.replaced.iter().any(|removed| removed.buff.uid == Some(change.buff_uid)))
                })
                .cloned()
                .collect::<Vec<_>>();
            let removal_reactions = dispatch_event_groups(
                pool,
                managers,
                catalog,
                determinism,
                state,
                &[removal_events],
                event_scope,
                &fanout_path,
                action_scope.map(Vec::as_slice),
                entry_halo_scope,
                current_skill,
                false,
                context.current_round > 0,
                crate::engine::event::subscription::PublicationPhase::BeforePublish,
                None,
                None,
                None,
                None,
            )?;
            if !removal_reactions.before_publish.is_empty() {
                drain_nested_queue(
                    managers,
                    pool,
                    catalog,
                    determinism,
                    removal_reactions.before_publish.into(),
                    result,
                    state,
                )?;
            }
            push_change(
                &mut result.frames,
                &fanout_path,
                part(crate::engine::manager::buff::BuffPublishPart::Removal),
            );
            push_change(
                &mut result.frames,
                &fanout_path,
                part(crate::engine::manager::buff::BuffPublishPart::Addition),
            );
        }
    }
    if let RuleOutcome::DeathSettlement(death) = outcome {
        push_death(&mut result.frames, frame_path, managers, state, *death);
    } else {
        for change in outcome.changes() {
            let change = match (replacement, change) {
                (Some(parts), crate::engine::runtime::change::BattleChange::Buff(_)) => {
                    crate::engine::runtime::change::BattleChange::Buff(Box::new(
                        parts.addition.clone(),
                    ))
                }
                (_, change) => change,
            };
            push_change(&mut result.frames, frame_path, change);
        }
    }
    for (owner, change) in outcome.owned_changes() {
        let nested_owner = match owner {
            crate::engine::skill::behavior::registry::OutputOwner::NestedLifecycleMechanic => {
                Some(FrameOwner::LifecycleMechanic)
            }
            crate::engine::skill::behavior::registry::OutputOwner::InitialSovereignBoost => {
                Some(FrameOwner::InitialSovereignBoost)
            }
            _ => None,
        };
        if let Some(nested_owner) = nested_owner {
            let nested = push_child(
                &mut result.frames,
                frame_path,
                nested_owner,
                frame_trigger(trigger),
            );
            push_change(&mut result.frames, &nested, change);
        } else {
            push_change(
                &mut result.frames,
                &output_frame_path(owner, frame_path),
                change,
            );
        }
    }
    for fanout in fanout {
        push_fanout(
            &mut result.frames,
            &output_frame_path(fanout_output_owner, event_scope),
            fanout,
            frame_trigger(trigger),
        );
    }

    Ok(())
}

fn push_fanout_frame(
    frames: &mut [SemanticFrame],
    parent: &[usize],
    fanout: &crate::engine::manager::buff::BuffFanoutResult,
    trigger: FrameTrigger,
) -> Vec<usize> {
    push_child(
        frames,
        parent,
        FrameOwner::BuffRule {
            emitter_uid: fanout.emitter_uid,
            carrier_buff_uid: fanout.carrier_buff_uid,
            carrier_buff_id: fanout.carrier_buff_id,
            rule: fanout.rule,
        },
        trigger,
    )
}

fn push_fanout(
    frames: &mut [SemanticFrame],
    parent: &[usize],
    fanout: crate::engine::manager::buff::BuffFanoutResult,
    trigger: FrameTrigger,
) {
    let fanout_path = push_fanout_frame(frames, parent, &fanout, trigger);
    push_change(
        frames,
        &fanout_path,
        crate::engine::runtime::change::BattleChange::BuffFanout(Box::new(fanout)),
    );
}

pub(super) struct ReleasedEvents<'a> {
    pub(super) started_after_reactions: Option<ReactionBatch>,
    pub(super) event_groups: &'a [Vec<BattleEvent>],
    pub(super) team_layer_settlements: Vec<BattleEvent>,
    pub(super) released_hit_events: Option<&'a [Vec<BattleEvent>]>,
    pub(super) terminal_owner_uids: Option<&'a [i64]>,
    pub(super) pending_deaths: Option<&'a [PendingDeath]>,
    pub(super) was_skill_action_started: bool,
    pub(super) was_hp_batch: bool,
}

pub(super) fn dispatch_after(
    access: DrainAccess<'_>,
    scope: Scope<'_>,
    events: ReleasedEvents<'_>,
) -> Result<ReactionBatch, DrainError> {
    let DrainAccess {
        managers,
        pool,
        catalog,
        determinism,
        result,
        state,
    } = access;
    let Scope {
        context,
        trigger,
        frame_path,
        event_scope,
        action_scope,
        entry_halo_scope,
        current_skill,
        field_transfer_owner,
        field_level_grant,
        ..
    } = scope;
    let ReleasedEvents {
        started_after_reactions,
        event_groups,
        team_layer_settlements,
        released_hit_events,
        terminal_owner_uids,
        pending_deaths,
        was_skill_action_started,
        was_hp_batch,
    } = events;
    // AfterPublish reactions are partitioned by their declared release
    // lane; after-hit and after-action work stays action-scoped in state.
    let mut reactions = if was_skill_action_started {
        started_after_reactions.unwrap_or_default()
    } else {
        dispatch_event_groups(
            pool,
            managers,
            catalog,
            determinism,
            state,
            event_groups,
            event_scope,
            frame_path,
            action_scope.map(Vec::as_slice),
            entry_halo_scope,
            current_skill,
            was_hp_batch,
            context.current_round > 0,
            crate::engine::event::subscription::PublicationPhase::AfterPublish,
            terminal_owner_uids,
            None,
            pending_deaths,
            field_transfer_owner,
        )?
    };
    if !team_layer_settlements.is_empty() {
        reactions.append(super::team_buff_layers::reactions(
            DrainAccess {
                managers,
                pool,
                catalog,
                determinism,
                result,
                state,
            },
            scope,
            team_layer_settlements,
            terminal_owner_uids,
        )?);
    }
    super::super::field_level::bind_reactions(
        field_level_grant,
        action_scope.map(Vec::as_slice),
        &mut reactions,
    );
    super::super::overflow_frame::defer_hit_storage(
        frame_path,
        action_scope.map(Vec::as_slice),
        trigger,
        &mut reactions,
    );
    state.defer_hp_thresholds(
        action_scope.map(Vec::as_slice),
        std::mem::take(&mut reactions.hp_thresholds),
    );
    state.defer_attacked_skills(
        action_scope.map(Vec::as_slice),
        std::mem::take(&mut reactions.attacked_skills),
    );
    state.defer_before_after_hit_observers(
        action_scope.map(Vec::as_slice),
        std::mem::take(&mut reactions.before_after_hit_observers),
    );
    state.defer_after_hit_skills(
        action_scope.map(Vec::as_slice),
        std::mem::take(&mut reactions.after_hit_skills),
    );
    state.defer_after_hit_shell(
        action_scope.map(Vec::as_slice),
        std::mem::take(&mut reactions.after_hit_shell),
    );
    state.defer_after_hit(
        action_scope.map(Vec::as_slice),
        std::mem::take(&mut reactions.after_hit),
    );
    let after_action = std::mem::take(&mut reactions.after_action);
    if action_scope.is_some() {
        state.defer_after_action(action_scope.map(Vec::as_slice), after_action);
    } else {
        reactions.after_publish.extend(after_action);
    }
    if let Some(hit_event_groups) = released_hit_events {
        let mut hit_buff_acts = dispatch_event_groups(
            pool,
            managers,
            catalog,
            determinism,
            state,
            hit_event_groups,
            event_scope,
            frame_path,
            action_scope.map(Vec::as_slice),
            entry_halo_scope,
            current_skill,
            true,
            context.current_round > 0,
            crate::engine::event::subscription::PublicationPhase::AfterPublish,
            terminal_owner_uids,
            Some(ReactionLane::BuffActs),
            None,
            None,
        )?;
        let mut hit_skills = dispatch_event_groups(
            pool,
            managers,
            catalog,
            determinism,
            state,
            hit_event_groups,
            event_scope,
            frame_path,
            action_scope.map(Vec::as_slice),
            entry_halo_scope,
            current_skill,
            true,
            context.current_round > 0,
            crate::engine::event::subscription::PublicationPhase::AfterPublish,
            terminal_owner_uids,
            Some(ReactionLane::Skills),
            None,
            None,
        )?;
        hit_buff_acts
            .attacked_skills
            .append(&mut hit_skills.attacked_skills);
        state.defer_attacked_skills(
            action_scope.map(Vec::as_slice),
            std::mem::take(&mut hit_buff_acts.attacked_skills),
        );
        hit_buff_acts
            .hp_thresholds
            .append(&mut hit_skills.hp_thresholds);
        state.defer_hp_thresholds(
            action_scope.map(Vec::as_slice),
            std::mem::take(&mut hit_buff_acts.hp_thresholds),
        );
        hit_buff_acts
            .before_after_hit_observers
            .append(&mut hit_skills.before_after_hit_observers);
        state.defer_before_after_hit_observers(
            action_scope.map(Vec::as_slice),
            std::mem::take(&mut hit_buff_acts.before_after_hit_observers),
        );
        hit_buff_acts
            .after_hit_skills
            .append(&mut hit_skills.after_hit_skills);
        state.defer_after_hit_skills(
            action_scope.map(Vec::as_slice),
            std::mem::take(&mut hit_buff_acts.after_hit_skills),
        );
        hit_buff_acts
            .after_hit_shell
            .append(&mut hit_skills.after_hit_shell);
        state.defer_after_hit_shell(
            action_scope.map(Vec::as_slice),
            std::mem::take(&mut hit_buff_acts.after_hit_shell),
        );
        state.defer_after_hit(
            action_scope.map(Vec::as_slice),
            std::mem::take(&mut hit_skills.after_hit),
        );
        let hit_after_action = std::mem::take(&mut hit_buff_acts.after_action);
        if action_scope.is_some() {
            state.defer_after_action(action_scope.map(Vec::as_slice), hit_after_action);
        } else {
            hit_buff_acts.after_publish.extend(hit_after_action);
        }
        let shared_hit_skills = std::mem::take(&mut hit_skills.shared_hit_skills);
        state.defer_hit_skills(action_scope.map(Vec::as_slice), shared_hit_skills);
        let hit_settlements = std::mem::take(&mut hit_skills.after_settlement);
        state.defer_hit_passive_skills(action_scope.map(Vec::as_slice), hit_skills.into_ordered());
        drain_nested_queue(
            managers,
            pool,
            catalog,
            determinism,
            hit_buff_acts
                .into_ordered()
                .into_iter()
                .chain(hit_settlements)
                .collect(),
            result,
            state,
        )?;
    }

    Ok(reactions)
}
