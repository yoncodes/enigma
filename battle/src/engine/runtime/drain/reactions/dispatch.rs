use super::{
    event::{queued_entity_entry_halo_fanout, route_skill_start_settlement},
    *,
};

#[allow(clippy::too_many_arguments)]
pub(in crate::engine::runtime::drain) fn dispatch_reactions(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    event: &BattleEvent,
    parent_path: Option<&[usize]>,
    reuse_path: Option<&[usize]>,
    action_path: Option<&[usize]>,
    entry_halo_scope: Option<&[usize]>,
    reentry_skill: Option<(i64, i32, Option<i64>)>,
    current_skill_target: Option<i64>,
    lane: Option<ReactionLane>,
    owner_uids: Option<&[i64]>,
    execute_unscoped_after_action: bool,
    publication_phase: Option<crate::engine::event::subscription::PublicationPhase>,
    mut multiplicity: Option<&mut crate::engine::event::dispatcher::DispatchMultiplicity<'_>>,
) -> Result<ReactionBatch, DrainError> {
    let mut reactions = ReactionBatch::default();
    let lifecycle_pool = matches!(
        event,
        BattleEvent::EntityEntered { .. } | BattleEvent::EntityTransformed { .. }
    )
    .then(|| pool.runtime_view(managers));
    let pool = lifecycle_pool.as_ref().unwrap_or(pool);
    let lifecycle_owner = match event {
        BattleEvent::EntityEntered { target_uid }
        | BattleEvent::EntityTransformed { target_uid } => Some([*target_uid]),
        _ => None,
    };
    let requested_owner_uids =
        owner_uids.or_else(|| lifecycle_owner.as_ref().map(|owners| owners.as_slice()));
    let scoped_owner_uids = terminal_owner_scope(pool, managers, requested_owner_uids);
    let owner_uids = scoped_owner_uids.as_deref();
    let transaction_ops = if !matches!(lane, Some(ReactionLane::Skills)) {
        crate::engine::skill::buff_act::transaction_rule_ops_for(managers, event, |definition| {
            reaction_lane_accepts_buff_act(lane, definition.key.opcode, definition.key.type_name)
        })
    } else {
        Vec::new()
    }
    .into_iter()
    .filter(|(feature, _)| owner_uids.is_none_or(|uids| uids.contains(&feature.owner_uid)))
    .collect::<Vec<_>>();
    let mut transaction_frame_groups = std::collections::HashMap::new();
    for (feature, op) in transaction_ops {
        let definition = required_buff_act_definition(feature.act_id(), &feature.act_type)?;
        if !reaction_lane_accepts_buff_act(lane, definition.key.opcode, definition.key.type_name) {
            continue;
        }
        let timing = definition.runtime.reaction_timing;
        let publication =
            crate::engine::skill::buff_act::transaction_publication(&feature, &op, event.kind())
                .ok_or_else(|| DrainError::MissingBuffActDefinition {
                    opcode: feature.act_id(),
                    type_name: feature.act_type.clone(),
                })?;
        let selected_for_pass = publication_phase.is_none_or(|phase| {
            if timing == crate::engine::event::subscription::ReactionTiming::AfterAction {
                phase == crate::engine::event::subscription::PublicationPhase::AfterPublish
            } else {
                publication == phase
            }
        });
        if !selected_for_pass {
            continue;
        }
        if timing == crate::engine::event::subscription::ReactionTiming::AfterAction
            && action_path.is_none()
            && !execute_unscoped_after_action
        {
            continue;
        }
        let frame_scope = definition.runtime.frame_scope;
        let causing_path = reuse_path.or(parent_path);
        let (frame_path, parent_path) = match frame_scope {
            crate::engine::skill::buff_act::registry::RuntimeFrameScope::CausingFrame => {
                (causing_path.map(|path| path.to_vec()), None)
            }
            crate::engine::skill::buff_act::registry::RuntimeFrameScope::SubscriberFrame => {
                let parent =
                    if timing == crate::engine::event::subscription::ReactionTiming::AfterAction {
                        action_path.or(causing_path)
                    } else {
                        causing_path
                    };
                (None, parent.map(|path| path.to_vec()))
            }
            crate::engine::skill::buff_act::registry::RuntimeFrameScope::IndependentEvent => {
                (None, None)
            }
        };

        let frame_group = matches!(
            frame_scope,
            crate::engine::skill::buff_act::registry::RuntimeFrameScope::CausingFrame
                | crate::engine::skill::buff_act::registry::RuntimeFrameScope::SubscriberFrame
        )
        .then(|| {
            transaction_frame_groups
                .entry((feature.owner_uid, feature.buff_uid, feature.act_id()))
                .or_insert_with(|| std::rc::Rc::new(std::cell::RefCell::new(None)))
                .clone()
        });
        let queued = QueuedOp {
            op,
            trigger: SkillOpTrigger::Event(event.clone()),
            skill_execution: None,
            frame_path,
            parent_path,
            frame_group,
            independent_parent_group: None,
            frame_owner: Some(FrameOwner::BuffAct {
                owner_uid: feature.owner_uid,
                source_uid: feature.source_uid,
                buff_uid: feature.buff_uid,
                buff_id: feature.buff_id,
                key: definition.key,
            }),
            subscriber: Some(SubscriberContext::new(feature.owner_uid)),
            caster_frame: None,
        };
        if timing == crate::engine::event::subscription::ReactionTiming::AfterAction {
            reactions.after_action.push(queued);
        } else if timing
            == crate::engine::event::subscription::ReactionTiming::BeforeHitPassiveSkills
        {
            reactions.before_hit_passive_skills.push(queued);
        } else if timing
            == crate::engine::event::subscription::ReactionTiming::BeforeAfterHitObservers
        {
            reactions.before_after_hit_observers.push(queued);
        } else if timing == crate::engine::event::subscription::ReactionTiming::AfterHitObservers {
            reactions.after_hit_skills.push(queued);
        } else if timing == crate::engine::event::subscription::ReactionTiming::AfterSettlement {
            reactions.after_settlement.push(queued);
        } else if publication == crate::engine::event::subscription::PublicationPhase::BeforePublish
        {
            reactions.before_publish.push(queued);
        } else {
            reactions.after_publish.push(queued);
        }
    }
    let runs_before = publication_phase.is_none_or(|phase| {
        phase == crate::engine::event::subscription::PublicationPhase::BeforePublish
    });
    let runs_after = publication_phase.is_none_or(|phase| {
        phase == crate::engine::event::subscription::PublicationPhase::AfterPublish
    });
    if runs_before {
        reactions.before_publish.extend(queued_selected_timing(
            pool,
            managers,
            catalog,
            determinism,
            event,
            parent_path,
            reuse_path,
            action_path,
            reentry_skill,
            current_skill_target,
            lane,
            owner_uids,
            crate::engine::event::subscription::PublicationPhase::BeforePublish,
            crate::engine::event::subscription::ReactionTiming::Immediate,
            multiplicity.as_deref_mut(),
        )?);
    }
    if lane.is_none()
        && let BattleEvent::SkillAction(action) = event
    {
        reactions
            .after_publish
            .extend(
                managers
                    .buff
                    .plan_action_expiries(action)
                    .into_iter()
                    .map(|expiry| QueuedOp {
                        op: RuleOp::Command(
                            crate::engine::skill::rule::output::BattleCommand::Buff(
                                crate::engine::manager::buff::BuffCommand::ExpireAction(
                                    crate::engine::manager::buff::BuffRemove {
                                        origin: crate::engine::skill::rule::CommandOrigin {
                                            domain:
                                                crate::engine::skill::rule::RuleDomain::Lifecycle,
                                            key: expiry.trigger.key(),
                                        },
                                        target_uid: expiry.owner_uid,
                                        selector:
                                            crate::engine::manager::buff::BuffRemoveSelector::Uid(
                                                expiry.buff_uid,
                                            ),
                                    },
                                ),
                            ),
                        ),
                        trigger: SkillOpTrigger::Event(event.clone()),
                        skill_execution: None,
                        frame_path: None,
                        parent_path: parent_path.map(|path| path.to_vec()),
                        frame_group: None,
                        independent_parent_group: None,
                        frame_owner: Some(FrameOwner::BuffAct {
                            owner_uid: expiry.owner_uid,
                            source_uid: expiry.source_uid,
                            buff_uid: expiry.buff_uid,
                            buff_id: expiry.buff_id,
                            key: expiry.trigger.key(),
                        }),
                        subscriber: None,
                        caster_frame: None,
                    }),
            );
    }
    if runs_after {
        let mut selected = queued_selected_timing(
            pool,
            managers,
            catalog,
            determinism,
            event,
            parent_path,
            reuse_path,
            action_path,
            reentry_skill,
            current_skill_target,
            lane,
            owner_uids,
            crate::engine::event::subscription::PublicationPhase::AfterPublish,
            crate::engine::event::subscription::ReactionTiming::Immediate,
            multiplicity.as_deref_mut(),
        )?;
        let settlement = if lane.is_none() {
            route_skill_start_settlement(event, reuse_path, &mut selected)
        } else {
            None
        };
        reactions.after_publish.extend(selected);
        if let Some(settlement) = settlement {
            reactions.after_publish.push(settlement);
        }
    }
    for (timing, destination) in [
        (
            crate::engine::event::subscription::ReactionTiming::AfterSettlement,
            &mut reactions.after_settlement,
        ),
        (
            crate::engine::event::subscription::ReactionTiming::AfterSkill,
            &mut reactions.after_skill,
        ),
        (
            crate::engine::event::subscription::ReactionTiming::AfterHitSkills,
            &mut reactions.after_hit,
        ),
        (
            crate::engine::event::subscription::ReactionTiming::BeforeHitPassiveSkills,
            &mut reactions.before_hit_passive_skills,
        ),
        (
            crate::engine::event::subscription::ReactionTiming::BeforeAfterHitObservers,
            &mut reactions.before_after_hit_observers,
        ),
        (
            crate::engine::event::subscription::ReactionTiming::AfterHitObservers,
            &mut reactions.after_hit_skills,
        ),
        (
            crate::engine::event::subscription::ReactionTiming::AfterSharedHit,
            &mut reactions.shared_hit_skills,
        ),
        (
            crate::engine::event::subscription::ReactionTiming::AfterAction,
            &mut reactions.after_action,
        ),
    ] {
        for publication in [
            crate::engine::event::subscription::PublicationPhase::BeforePublish,
            crate::engine::event::subscription::PublicationPhase::AfterPublish,
        ] {
            if (publication == crate::engine::event::subscription::PublicationPhase::BeforePublish
                && !runs_before)
                || (publication
                    == crate::engine::event::subscription::PublicationPhase::AfterPublish
                    && !runs_after)
            {
                continue;
            }
            destination.extend(queued_selected_timing(
                pool,
                managers,
                catalog,
                determinism,
                event,
                parent_path,
                reuse_path,
                action_path,
                reentry_skill,
                current_skill_target,
                lane,
                owner_uids,
                publication,
                timing,
                multiplicity.as_deref_mut(),
            )?);
        }
    }
    if lane.is_none() {
        let duration_advances = match event {
            BattleEvent::ActionQueueCommitted { team, .. } => {
                let mut owner_uids = match *team {
                    1 => pool.attacker_all.iter().map(|entity| entity.uid).collect(),
                    2 => pool.defender_all.iter().map(|entity| entity.uid).collect(),
                    _ => Vec::new(),
                };
                if let Some(side_uid) = match *team {
                    1 => Some(crate::engine::fight::rules::ATTACKER_SIDE_UID),
                    2 => Some(crate::engine::fight::rules::DEFENDER_SIDE_UID),
                    _ => None,
                } {
                    owner_uids.push(side_uid);
                }
                crate::engine::skill::buff_act::effect_time::duration_stages_for_event(
                    crate::engine::event::kind::EventKind::ActionQueueCommitted,
                )
                .filter_map(|take_stage| {
                    let buff_uids = managers.buff.duration_buff_uids(take_stage, &owner_uids);
                    if buff_uids.is_empty() {
                        return None;
                    }
                    crate::engine::manager::buff::BuffDurationAdvance::new(
                        take_stage,
                        owner_uids.clone(),
                        Some(buff_uids),
                    )
                })
                .collect()
            }
            _ => crate::engine::manager::buff::BuffDurationAdvance::for_event(event),
        };
        reactions
            .after_publish
            .extend(duration_advances.into_iter().map(|advance| QueuedOp {
                op: RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Buff(
                    crate::engine::manager::buff::BuffCommand::AdvanceDuration(advance),
                )),
                trigger: SkillOpTrigger::Event(event.clone()),
                skill_execution: None,
                frame_path: reuse_path.map(|path| path.to_vec()),
                parent_path: None,
                frame_group: None,
                independent_parent_group: None,
                frame_owner: Some(FrameOwner::EventRule),
                subscriber: None,
                caster_frame: None,
            }));
        if publication_phase.is_none_or(|phase| {
            phase == crate::engine::event::subscription::PublicationPhase::AfterPublish
        }) && let Some(fanout) =
            queued_entity_entry_halo_fanout(event, entry_halo_scope, parent_path)
        {
            reactions.after_publish.push(fanout);
        }
    }
    Ok(reactions)
}

fn reaction_lane_accepts_buff_act(lane: Option<ReactionLane>, act_id: i32, act_type: &str) -> bool {
    use crate::engine::event::subscription::ReactionTiming;

    match lane {
        Some(ReactionLane::BuffActsBeforeSettlement) => {
            crate::engine::skill::buff_act::registry::reaction_timing(act_id, act_type)
                != ReactionTiming::AfterSettlement
        }
        Some(ReactionLane::BuffActsAfterSettlement) => {
            crate::engine::skill::buff_act::registry::reaction_timing(act_id, act_type)
                == ReactionTiming::AfterSettlement
        }
        _ => true,
    }
}

#[allow(clippy::too_many_arguments)]
fn queued_selected_timing(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    event: &BattleEvent,
    parent_path: Option<&[usize]>,
    reuse_path: Option<&[usize]>,
    action_path: Option<&[usize]>,
    reentry_skill: Option<(i64, i32, Option<i64>)>,
    current_skill_target: Option<i64>,
    lane: Option<ReactionLane>,
    owner_uids: Option<&[i64]>,
    publication: crate::engine::event::subscription::PublicationPhase,
    timing: crate::engine::event::subscription::ReactionTiming,
    multiplicity: Option<&mut crate::engine::event::dispatcher::DispatchMultiplicity<'_>>,
) -> Result<Vec<QueuedOp>, DrainError> {
    let reaction_scope = multiplicity.as_ref().map(|multiplicity| {
        (
            multiplicity.event_batch_id,
            multiplicity.action_path.map(|path| path.to_vec()),
        )
    });
    let mut dispatched = dispatcher::dispatch_selected_event(
        pool,
        managers,
        catalog,
        determinism,
        event,
        dispatcher::DispatchSelection {
            owner_uids,
            publication: Some(publication),
            lane,
            timing,
        },
        multiplicity,
    )?;
    if !matches!(
        lane,
        Some(
            ReactionLane::BuffActs
                | ReactionLane::BuffActsBeforeSettlement
                | ReactionLane::BuffActsAfterSettlement
        )
    ) && event.kind() == crate::engine::event::kind::EventKind::BuffRejected
        && let Some((owner_uid, skill_id, _)) = reentry_skill
    {
        let current = dispatcher::dispatch_selected_skill_event(
            pool,
            managers,
            catalog,
            determinism,
            event,
            dispatcher::DispatchSkillSelection {
                owner_skill: (owner_uid, skill_id),
                publication,
                timing,
            },
        )?;
        for skill in current.skills {
            if !dispatched.skills.contains(&skill) {
                dispatched.skills.push(skill);
            }
        }
    }
    queued_reactions(
        pool,
        dispatched,
        event,
        parent_path,
        reuse_path,
        action_path,
        reentry_skill,
        current_skill_target,
        reaction_scope,
    )
}

pub(in crate::engine::runtime::drain) fn dispatch_owner_reactions(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    event: &BattleEvent,
    owner_uids: &[i64],
) -> Result<Vec<QueuedOp>, DrainError> {
    let scoped_owner_uids = terminal_owner_scope(pool, managers, Some(owner_uids))
        .expect("an explicit owner scope remains explicit");
    let dispatched = dispatcher::dispatch_owner_event(
        pool,
        managers,
        catalog,
        determinism,
        event,
        &scoped_owner_uids,
    )?;
    let mut reactions =
        queued_reactions(pool, dispatched, event, None, None, None, None, None, None)?;
    if let Some(fanout) = queued_entity_entry_halo_fanout(event, None, None) {
        reactions.push(fanout);
    }
    assign_subscriber_death_groups(&mut [&mut reactions]);
    Ok(reactions)
}

pub(super) fn terminal_owner_scope(
    pool: &TargetPool,
    managers: &BattleManagers,
    requested: Option<&[i64]>,
) -> Option<Vec<i64>> {
    let winners = managers
        .terminal_outcome()
        .and_then(|outcome| outcome.winning_team())
        .map(|team| pool.team_uids(team));
    match (requested, winners) {
        (Some(requested), Some(winners)) => Some(
            requested
                .iter()
                .copied()
                .filter(|owner_uid| winners.contains(owner_uid))
                .collect(),
        ),
        (None, Some(winners)) => Some(winners),
        (Some(requested), None) => Some(requested.to_vec()),
        (None, None) => None,
    }
}
