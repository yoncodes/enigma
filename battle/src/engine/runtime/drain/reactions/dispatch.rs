use super::{
    event::{queued_entity_entry_halo_fanout, route_skill_start_settlement},
    *,
};
use crate::engine::{
    event::subscription::{PublicationPhase, ReactionTiming},
    skill::buff_act::registry::RuntimeFrameScope,
};

/// One dispatch of an event's reactions: where they attach and which ones are selected.
#[derive(Clone, Copy)]
pub(in crate::engine::runtime::drain) struct ReactionDispatch<'a> {
    pub event: &'a BattleEvent,
    pub parent_path: Option<&'a [usize]>,
    pub reuse_path: Option<&'a [usize]>,
    pub action_path: Option<&'a [usize]>,
    pub entry_halo_scope: Option<&'a [usize]>,
    pub reentry_skill: Option<(i64, i32, Option<i64>)>,
    pub current_skill_target: Option<i64>,
    pub lane: Option<ReactionLane>,
    pub owner_uids: Option<&'a [i64]>,
    pub execute_unscoped_after_action: bool,
    pub publication_phase: Option<PublicationPhase>,
}

impl<'a> ReactionDispatch<'a> {
    /// An unscoped dispatch of every reaction to `event`.
    pub(in crate::engine::runtime::drain) fn for_event(event: &'a BattleEvent) -> Self {
        Self {
            event,
            parent_path: None,
            reuse_path: None,
            action_path: None,
            entry_halo_scope: None,
            reentry_skill: None,
            current_skill_target: None,
            lane: None,
            owner_uids: None,
            execute_unscoped_after_action: false,
            publication_phase: None,
        }
    }
}

pub(in crate::engine::runtime::drain) fn dispatch_reactions(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    dispatch: ReactionDispatch<'_>,
    mut multiplicity: Option<&mut crate::engine::event::dispatcher::DispatchMultiplicity<'_>>,
) -> Result<ReactionBatch, DrainError> {
    let ReactionDispatch {
        event,
        parent_path,
        reuse_path,
        action_path,
        entry_halo_scope,
        reentry_skill: _,
        current_skill_target: _,
        lane,
        owner_uids,
        execute_unscoped_after_action,
        publication_phase,
    } = dispatch;
    let mut reactions = ReactionBatch::default();
    let lifecycle_pool = matches!(
        event,
        BattleEvent::EntityEntered { .. } | BattleEvent::EntityTransformed { .. }
    )
    .then(|| pool.runtime_view(managers));
    let pool = lifecycle_pool.as_ref().unwrap_or(pool);
    let lifecycle_owner = match event {
        BattleEvent::EntityEntered { target_uid, .. }
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
            crate::engine::skill::buff_act::transaction_publication(&feature, &op, event, managers)
                .ok_or_else(|| DrainError::MissingBuffActDefinition {
                    opcode: feature.act_id(),
                    type_name: feature.act_type.clone(),
                })?;
        let selected_for_pass = publication_phase.is_none_or(|phase| {
            if timing == ReactionTiming::AfterAction {
                phase == PublicationPhase::AfterPublish
            } else {
                publication == phase
            }
        });
        if !selected_for_pass {
            continue;
        }
        if timing == ReactionTiming::AfterAction
            && action_path.is_none()
            && !execute_unscoped_after_action
        {
            continue;
        }
        let frame_scope = definition.runtime.frame_scope;
        let causing_path = reuse_path.or(parent_path);
        let (frame_path, parent_path) = match frame_scope {
            RuntimeFrameScope::CausingFrame => (causing_path.map(|path| path.to_vec()), None),
            RuntimeFrameScope::SubscriberFrame => {
                let parent = if timing == ReactionTiming::AfterAction {
                    action_path.or(causing_path)
                } else {
                    causing_path
                };
                (None, parent.map(|path| path.to_vec()))
            }
            RuntimeFrameScope::IndependentEvent => (None, None),
        };

        let frame_group = matches!(
            frame_scope,
            RuntimeFrameScope::CausingFrame | RuntimeFrameScope::SubscriberFrame
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
        let destination = match timing {
            ReactionTiming::AfterAction => &mut reactions.after_action,
            ReactionTiming::AttackedSkills if action_path.is_some() => {
                &mut reactions.attacked_skills
            }
            ReactionTiming::HpThresholds => &mut reactions.hp_thresholds,
            ReactionTiming::BeforeAfterHitObservers => &mut reactions.before_after_hit_observers,
            ReactionTiming::AfterHitObservers => &mut reactions.after_hit_skills,
            ReactionTiming::AfterHitShell => &mut reactions.after_hit_shell,
            ReactionTiming::AfterSettlement => &mut reactions.after_settlement,
            _ if publication == PublicationPhase::BeforePublish => &mut reactions.before_publish,
            _ => &mut reactions.after_publish,
        };
        destination.push(queued);
    }
    let runs_before =
        publication_phase.is_none_or(|phase| phase == PublicationPhase::BeforePublish);
    let runs_after = publication_phase.is_none_or(|phase| phase == PublicationPhase::AfterPublish);
    if runs_before {
        reactions.before_publish.extend(queued_selected_timing(
            pool,
            managers,
            catalog,
            determinism,
            ReactionDispatch {
                owner_uids,
                ..dispatch
            },
            (PublicationPhase::BeforePublish, ReactionTiming::Immediate),
            multiplicity.as_deref_mut(),
        )?);
    }
    if lane.is_none()
        && let BattleEvent::SkillAction(action) = event
    {
        reactions.after_publish.extend(
            managers
                .buff
                .plan_action_settlements(action)
                .into_iter()
                .map(|settlement| QueuedOp {
                    op: RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Buff(
                        settlement.command,
                    )),
                    trigger: SkillOpTrigger::Event(event.clone()),
                    skill_execution: None,
                    frame_path: None,
                    parent_path: parent_path.map(|path| path.to_vec()),
                    frame_group: None,
                    independent_parent_group: None,
                    frame_owner: Some(FrameOwner::BuffAct {
                        owner_uid: settlement.owner_uid,
                        source_uid: settlement.source_uid,
                        buff_uid: settlement.buff_uid,
                        buff_id: settlement.buff_id,
                        key: settlement.trigger.key(),
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
            ReactionDispatch {
                owner_uids,
                ..dispatch
            },
            (PublicationPhase::AfterPublish, ReactionTiming::Immediate),
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
            ReactionTiming::AfterSettlement,
            &mut reactions.after_settlement,
        ),
        (ReactionTiming::AfterSkill, &mut reactions.after_skill),
        (ReactionTiming::AfterHitSkills, &mut reactions.after_hit),
        (
            ReactionTiming::AttackedSkills,
            if action_path.is_some() {
                &mut reactions.attacked_skills
            } else {
                &mut reactions.after_publish
            },
        ),
        (ReactionTiming::HpThresholds, &mut reactions.hp_thresholds),
        (
            ReactionTiming::BeforeAfterHitObservers,
            &mut reactions.before_after_hit_observers,
        ),
        (
            ReactionTiming::AfterHitObservers,
            &mut reactions.after_hit_skills,
        ),
        (
            ReactionTiming::AfterHitShell,
            &mut reactions.after_hit_shell,
        ),
        (
            ReactionTiming::AfterSharedHit,
            &mut reactions.shared_hit_skills,
        ),
        (ReactionTiming::AfterAction, &mut reactions.after_action),
    ] {
        for publication in [
            PublicationPhase::BeforePublish,
            PublicationPhase::AfterPublish,
        ] {
            if (publication == PublicationPhase::BeforePublish && !runs_before)
                || (publication == PublicationPhase::AfterPublish && !runs_after)
            {
                continue;
            }
            destination.extend(queued_selected_timing(
                pool,
                managers,
                catalog,
                determinism,
                ReactionDispatch {
                    owner_uids,
                    ..dispatch
                },
                (publication, timing),
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
        if publication_phase.is_none_or(|phase| phase == PublicationPhase::AfterPublish)
            && let Some(fanout) =
                queued_entity_entry_halo_fanout(event, entry_halo_scope, parent_path)
        {
            reactions.after_publish.push(fanout);
        }
    }
    Ok(reactions)
}

fn reaction_lane_accepts_buff_act(lane: Option<ReactionLane>, act_id: i32, act_type: &str) -> bool {
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

fn queued_selected_timing(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    dispatch: ReactionDispatch<'_>,
    (publication, timing): (PublicationPhase, ReactionTiming),
    multiplicity: Option<&mut crate::engine::event::dispatcher::DispatchMultiplicity<'_>>,
) -> Result<Vec<QueuedOp>, DrainError> {
    let ReactionDispatch {
        event,
        parent_path,
        reuse_path,
        action_path,
        reentry_skill,
        current_skill_target,
        lane,
        owner_uids,
        ..
    } = dispatch;
    let reaction_scope = multiplicity.as_ref().map(|multiplicity| {
        (
            multiplicity.event_batch_id,
            multiplicity.action_path.map(|path| path.to_vec()),
        )
    });
    let selected_completion = match (event, reentry_skill) {
        (BattleEvent::AllyAction(action), Some((owner_uid, skill_id, _)))
            if (owner_uid, skill_id) == (action.source_uid, action.skill_id)
                && action.mode.completes_action() =>
        {
            Some((owner_uid, skill_id))
        }
        _ => None,
    };
    let mut dispatched = dispatcher::dispatch_selected_event(
        pool,
        managers,
        catalog,
        determinism,
        event,
        dispatcher::DispatchSelection {
            owner_uids,
            publication: Some(publication),
            completing_skill: selected_completion,
            lane,
            timing,
        },
        multiplicity,
    )?;
    if matches!(lane, None | Some(ReactionLane::Skills))
        && event.kind() == crate::engine::event::kind::EventKind::BuffRejected
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
    let mut queued = queued_reactions(
        pool,
        catalog,
        dispatched,
        event,
        parent_path,
        reuse_path,
        action_path,
        reentry_skill.filter(|_| !matches!(event, BattleEvent::AllyAction(_))),
        current_skill_target,
        reaction_scope,
    )?;
    if matches!(lane, None | Some(ReactionLane::Skills))
        && publication == PublicationPhase::BeforePublish
        && timing == ReactionTiming::Immediate
        && let Some(owner_skill) = selected_completion
    {
        let selected = dispatcher::dispatch_selected_skill_event(
            pool,
            managers,
            catalog,
            determinism,
            event,
            dispatcher::DispatchSkillSelection {
                owner_skill,
                publication,
                timing,
            },
        )?;
        queued.extend(queued_reactions(
            pool,
            catalog,
            selected,
            event,
            parent_path,
            reuse_path,
            action_path,
            reentry_skill,
            current_skill_target,
            None,
        )?);
    }
    Ok(queued)
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
    let mut reactions = queued_reactions(
        pool, catalog, dispatched, event, None, None, None, None, None, None,
    )?;
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
