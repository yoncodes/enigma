use super::*;
use crate::engine::{
    manager::{
        buff::{BuffCommand, BuffEntryHaloFanout},
        card::{CardCommand, CardSetUltimateAvailability},
    },
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain, output::BattleCommand},
};

const ULTIMATE_AVAILABILITY_ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::Lifecycle,
    key: DefinitionKey::new(0, "UltimateAvailability"),
};

fn queued_entity_entry_halo_fanout(
    event: &BattleEvent,
    entry_scope: Option<&[usize]>,
    parent_path: Option<&[usize]>,
) -> Option<QueuedOp> {
    let BattleEvent::EntityEntered { target_uid } = event else {
        return None;
    };
    let (frame_path, parent_path) = match entry_scope {
        Some(entry_scope) => (Some(entry_scope.to_vec()), None),
        None => (None, parent_path.map(<[usize]>::to_vec)),
    };
    Some(QueuedOp {
        op: RuleOp::Command(BattleCommand::Buff(BuffCommand::FanoutEntryHalos(
            BuffEntryHaloFanout::new(vec![*target_uid]),
        ))),
        trigger: SkillOpTrigger::Event(event.clone()),
        skill_execution: None,
        frame_path,
        parent_path,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: Some(FrameOwner::EventRule),
        subscriber: None,
        caster_frame: None,
    })
}

fn queued_invalid_ultimate_removal(
    pool: &TargetPool,
    managers: &BattleManagers,
    event: &BattleEvent,
    reuse_path: &[usize],
) -> Option<QueuedOp> {
    let BattleEvent::ExPointChanged(change) = event else {
        return None;
    };
    let entity = pool
        .attacker_main
        .iter()
        .find(|entity| entity.uid == change.target_uid)?;
    let mechanic = crate::engine::mechanic::card::CardMechanic;
    if managers.hp.current(entity.uid) <= 0
        || mechanic.ultimate_ignores_limit(managers, entity.uid, entity.ex_skill)
    {
        return None;
    }
    let current = managers
        .card
        .hand()
        .iter()
        .find(|card| mechanic.is_ultimate(managers, card, entity))
        .cloned();
    if mechanic.ultimate_ready(managers, entity) {
        return None;
    }
    Some(QueuedOp {
        op: RuleOp::Command(BattleCommand::Card(CardCommand::SetUltimateAvailability(
            CardSetUltimateAvailability {
                origin: ULTIMATE_AVAILABILITY_ORIGIN,
                card: current?,
                available: false,
            },
        ))),
        trigger: SkillOpTrigger::Event(event.clone()),
        skill_execution: None,
        frame_path: Some(reuse_path.to_vec()),
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: Some(FrameOwner::EventRule),
        subscriber: None,
        caster_frame: None,
    })
}

fn queued_buff_act_feature_op(
    feature: crate::engine::manager::buff::ActiveBuffFeature,
    op: RuleOp,
    event: &BattleEvent,
    parent_path: &[usize],
) -> Result<QueuedOp, DrainError> {
    let key = required_buff_act_definition(feature.act_id(), &feature.act_type)?.key;
    Ok(QueuedOp {
        op,
        trigger: SkillOpTrigger::Event(event.clone()),
        skill_execution: None,
        frame_path: None,
        parent_path: Some(parent_path.to_vec()),
        frame_group: None,
        independent_parent_group: None,
        frame_owner: Some(FrameOwner::BuffAct {
            owner_uid: feature.owner_uid,
            source_uid: feature.source_uid,
            buff_uid: feature.buff_uid,
            buff_id: feature.buff_id,
            key,
        }),
        subscriber: Some(SubscriberContext::new(feature.owner_uid)),
        caster_frame: None,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn dispatch_event_groups(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    drain_state: &mut DrainState,
    event_groups: &[Vec<BattleEvent>],
    parent_path: &[usize],
    reuse_path: &[usize],
    action_path: Option<&[usize]>,
    entry_halo_scope: &[usize],
    current_skill: Option<(i64, i32, Option<i64>)>,
    include_attack_consumption: bool,
    execute_unscoped_after_action: bool,
    publication_phase: crate::engine::event::subscription::PublicationPhase,
    owner_uids: Option<&[i64]>,
    lane: Option<ReactionLane>,
    pending_deaths: Option<&[crate::engine::manager::hp::PendingDeath]>,
) -> Result<ReactionBatch, DrainError> {
    let all_events = event_groups.iter().flatten().cloned().collect::<Vec<_>>();
    let shared_damage_counts = drain_state.record_action_shared_damage(action_path, &all_events);
    let mut state = EventBatchState {
        batch_id: drain_state.next_event_batch_id(),
        ..Default::default()
    };
    let mut unscoped_reactions = std::collections::HashSet::new();
    let fired_once_per_action = action_path.map_or(&mut unscoped_reactions, |action_path| {
        drain_state.action_reactions(action_path)
    });
    let mut reactions = ReactionBatch::default();
    let mut scheduled_death_completions = Vec::<crate::engine::manager::hp::PendingDeath>::new();
    for events in event_groups {
        let mut group = dispatch_event_batch_inner(
            pool,
            managers,
            catalog,
            determinism,
            events,
            &all_events,
            parent_path,
            reuse_path,
            action_path,
            Some(entry_halo_scope),
            current_skill,
            include_attack_consumption,
            execute_unscoped_after_action,
            publication_phase,
            owner_uids,
            lane,
            &mut state,
            fired_once_per_action,
        )?;
        apply_shared_damage_count(&mut group, &shared_damage_counts);
        if publication_phase == crate::engine::event::subscription::PublicationPhase::AfterPublish
            && lane.is_none()
            && let Some(pending_deaths) = pending_deaths
        {
            for event in events {
                let BattleEvent::EntityDied(event) = event else {
                    continue;
                };
                let Some(death) = pending_deaths
                    .iter()
                    .find(|death| {
                        let transition = death.transition();
                        !scheduled_death_completions.contains(*death)
                            && transition.source_uid == event.source_uid
                            && transition.target_uid == event.target_uid
                    })
                    .copied()
                else {
                    continue;
                };
                scheduled_death_completions.push(death);
                group
                    .after_publish
                    .push(queued_death_event_group_completion(
                        death,
                        event,
                        action_path.unwrap_or(parent_path),
                    ));
            }
        }
        reactions.append(group);
    }
    reactions.assign_subscriber_death_groups(action_path);
    Ok(reactions)
}

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub(super) fn dispatch_event_batch(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    events: &[BattleEvent],
    parent_path: &[usize],
    reuse_path: &[usize],
    action_path: Option<&[usize]>,
    current_skill: Option<(i64, i32, Option<i64>)>,
    include_attack_consumption: bool,
    execute_unscoped_after_action: bool,
    publication_phase: crate::engine::event::subscription::PublicationPhase,
    owner_uids: Option<&[i64]>,
) -> Result<ReactionBatch, DrainError> {
    let shared_damage_counts = DrainState::default().record_action_shared_damage(None, events);
    let mut reactions = dispatch_event_batch_inner(
        pool,
        managers,
        catalog,
        determinism,
        events,
        events,
        parent_path,
        reuse_path,
        action_path,
        None,
        current_skill,
        include_attack_consumption,
        execute_unscoped_after_action,
        publication_phase,
        owner_uids,
        None,
        &mut EventBatchState::default(),
        &mut std::collections::HashSet::new(),
    )?;
    apply_shared_damage_count(&mut reactions, &shared_damage_counts);
    reactions.assign_subscriber_death_groups(action_path);
    Ok(reactions)
}

#[derive(Default)]
struct EventBatchState {
    batch_id: u64,
    queued_attack_consumption: bool,
    fired_once_per_target:
        std::collections::HashSet<crate::engine::event::dispatcher::OncePerTargetKey>,
    fired_once_per_event_batch:
        std::collections::HashSet<crate::engine::event::dispatcher::ReactionMultiplicityKey>,
}

#[allow(clippy::too_many_arguments)]
fn dispatch_event_batch_inner(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    events: &[BattleEvent],
    attack_events: &[BattleEvent],
    parent_path: &[usize],
    reuse_path: &[usize],
    action_path: Option<&[usize]>,
    entry_halo_scope: Option<&[usize]>,
    current_skill: Option<(i64, i32, Option<i64>)>,
    include_attack_consumption: bool,
    execute_unscoped_after_action: bool,
    publication_phase: crate::engine::event::subscription::PublicationPhase,
    owner_uids: Option<&[i64]>,
    lane: Option<ReactionLane>,
    state: &mut EventBatchState,
    fired_once_per_action: &mut std::collections::HashSet<
        crate::engine::event::dispatcher::ReactionMultiplicityKey,
    >,
) -> Result<ReactionBatch, DrainError> {
    let scoped_owner_uids = terminal_owner_scope(pool, managers, owner_uids);
    let owner_uids = scoped_owner_uids.as_deref();
    let current_skill_target = current_skill.and_then(|(_, _, target_uid)| target_uid);
    let after_publish =
        publication_phase == crate::engine::event::subscription::PublicationPhase::AfterPublish;
    let runs_buff_acts = !matches!(lane, Some(ReactionLane::Skills));
    let attack_sources = if include_attack_consumption && after_publish && runs_buff_acts {
        ordered_hit_entities(attack_events, |hit| hit.source_uid)
    } else {
        Vec::new()
    };
    let attacked_targets = if include_attack_consumption && after_publish && runs_buff_acts {
        ordered_hit_entities(attack_events, |hit| hit.target_uid)
    } else {
        Vec::new()
    };
    let mut reactions = ReactionBatch::default();
    for event in events {
        if after_publish
            && lane.is_none()
            && owner_uids.is_none_or(|owners| {
                event
                    .target_uid()
                    .is_none_or(|target_uid| owners.contains(&target_uid))
            })
            && let Some(sync) = queued_invalid_ultimate_removal(pool, managers, event, reuse_path)
        {
            reactions.after_publish.push(sync);
        }
        if after_publish && !state.queued_attack_consumption && matches!(event, BattleEvent::Hit(_))
        {
            for source_uid in attack_sources
                .iter()
                .filter(|uid| owner_uids.is_none_or(|owners| owners.contains(uid)))
            {
                for (feature, op) in crate::engine::skill::buff_act::attack_consumption_rule_ops(
                    managers,
                    *source_uid,
                    current_skill.is_some_and(|(_, skill_id, _)| catalog.is_big_skill(skill_id)),
                ) {
                    reactions.after_hit.push(queued_buff_act_feature_op(
                        feature,
                        op,
                        event,
                        parent_path,
                    )?);
                }
            }
            for target_uid in attacked_targets
                .iter()
                .filter(|uid| owner_uids.is_none_or(|owners| owners.contains(uid)))
            {
                let damage_types = attack_events
                    .iter()
                    .filter_map(|event| match event {
                        BattleEvent::Hit(hit) if hit.target_uid == *target_uid => {
                            pool.entity(hit.source_uid).map(|entity| entity.damage_type)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                for (feature, op) in
                    crate::engine::skill::buff_act::be_attacked_consumption_rule_ops(
                        managers,
                        *target_uid,
                        &damage_types,
                    )
                {
                    reactions.after_publish.push(queued_buff_act_feature_op(
                        feature,
                        op,
                        event,
                        parent_path,
                    )?);
                }
            }
            state.queued_attack_consumption = true;
        }
        let reentry_skill = current_skill.filter(|_| {
            matches!(
                event.kind(),
                crate::engine::event::kind::EventKind::BuffAdded
                    | crate::engine::event::kind::EventKind::BuffRejected
            )
        });
        let mut multiplicity = crate::engine::event::dispatcher::DispatchMultiplicity {
            once_per_target: &mut state.fired_once_per_target,
            once_per_event_batch: &mut state.fired_once_per_event_batch,
            once_per_action: fired_once_per_action,
            event_batch_id: state.batch_id,
            action_path,
        };
        let dispatched = dispatch_reactions(
            pool,
            managers,
            catalog,
            determinism,
            event,
            Some(parent_path),
            Some(reuse_path),
            action_path,
            entry_halo_scope,
            reentry_skill,
            current_skill_target,
            lane,
            owner_uids,
            execute_unscoped_after_action,
            Some(publication_phase),
            Some(&mut multiplicity),
        )?;
        reactions.append(dispatched);
    }
    Ok(reactions)
}

fn apply_shared_damage_count(
    reactions: &mut ReactionBatch,
    counts: &std::collections::HashMap<i64, i32>,
) {
    for queued in reactions.queued_mut() {
        let RuleOp::Skill(invocation) = &queued.op else {
            continue;
        };
        if invocation.condition_key != Some(DefinitionKey::new(695213, "ShareDamage")) {
            continue;
        }
        let SkillOpTrigger::Event(BattleEvent::DamageShared { share_count, .. }) =
            &mut queued.trigger
        else {
            continue;
        };
        *share_count = counts
            .get(&invocation.plan.source_uid)
            .copied()
            .unwrap_or(*share_count);
    }
}

pub(super) fn ordered_hit_entities(
    events: &[BattleEvent],
    select: impl Fn(&crate::engine::event::payload::HitEvent) -> i64,
) -> Vec<i64> {
    let mut entities = Vec::new();
    for event in events {
        let BattleEvent::Hit(hit) = event else {
            continue;
        };
        let uid = select(hit);
        if !entities.contains(&uid) {
            entities.push(uid);
        }
    }
    entities
}

#[allow(clippy::too_many_arguments)]
pub(super) fn dispatch_reactions(
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
        reactions.after_publish.extend(queued_selected_timing(
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
        )?);
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

pub(super) fn dispatch_owner_reactions(
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

fn terminal_owner_scope(
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

#[allow(clippy::too_many_arguments)]
pub(super) fn queued_reactions(
    pool: &TargetPool,
    dispatched: dispatcher::DispatchBatch,
    event: &BattleEvent,
    parent_path: Option<&[usize]>,
    reuse_path: Option<&[usize]>,
    action_path: Option<&[usize]>,
    reentry_skill: Option<(i64, i32, Option<i64>)>,
    current_skill_target: Option<i64>,
    reaction_scope: Option<(u64, Option<Vec<usize>>)>,
) -> Result<Vec<QueuedOp>, DrainError> {
    let mut reactions = Vec::new();
    for pass in dispatched.into_passes() {
        reactions.extend(queued_reaction_pass(
            pool,
            pass,
            event,
            parent_path,
            reuse_path,
            action_path,
            reentry_skill,
            current_skill_target,
            reaction_scope.clone(),
        )?);
    }
    Ok(reactions)
}

#[allow(clippy::too_many_arguments)]
fn queued_reaction_pass(
    pool: &TargetPool,
    dispatched: dispatcher::DispatchBatch,
    event: &BattleEvent,
    parent_path: Option<&[usize]>,
    reuse_path: Option<&[usize]>,
    action_path: Option<&[usize]>,
    reentry_skill: Option<(i64, i32, Option<i64>)>,
    current_skill_target: Option<i64>,
    reaction_scope: Option<(u64, Option<Vec<usize>>)>,
) -> Result<Vec<QueuedOp>, DrainError> {
    let mut skill_groups = HashMap::<(i64, i32), Rc<RefCell<Option<FramePath>>>>::new();
    let mut independent_skill_groups = HashMap::<(i64, i32), Rc<RefCell<Option<FramePath>>>>::new();
    let skills = dispatched
        .skills
        .into_iter()
        .map(|(subscriber, op)| {
            let definition = crate::engine::skill::condition::registry::find_key(
                subscriber.key.definition.opcode,
                subscriber.key.definition.type_name,
            )
            .ok_or_else(|| {
                DrainError::Subscriber(
                    crate::engine::skill::subscriber::SubscriberError::UncompiledRoute {
                        skill_id: subscriber.skill_id,
                        route:
                            crate::engine::skill::rule::route::RouteError::UnregisteredExactKey {
                                opcode: subscriber.key.definition.opcode,
                                type_name: subscriber.key.definition.type_name.to_owned(),
                            },
                    },
                )
            })?;
            Ok((subscriber, op, definition))
        })
        .collect::<Result<Vec<_>, DrainError>>()?;
    let mut reactions = skills
        .into_iter()
        .map(|(subscriber, mut op, definition)| {
            if definition.reaction_multiplicity
                != crate::engine::skill::condition::registry::ReactionMultiplicity::PerEvent
                && let RuleOp::Skill(invocation) = &mut op
                && let Some((batch_id, action_path)) = &reaction_scope
            {
                invocation.reaction_batch_id = Some(*batch_id);
                invocation.reaction_action_path = action_path.clone();
            }
            let frame_scope = definition.reaction_frame_scope;
            let frame_source_uid = match definition.reaction_frame_source {
                crate::engine::skill::condition::registry::ReactionFrameSource::Subscriber => {
                    subscriber.owner_uid
                }
                crate::engine::skill::condition::registry::ReactionFrameSource::Side => {
                    match pool.team_type(subscriber.owner_uid) {
                        Some(1) => crate::engine::fight::rules::ATTACKER_SIDE_UID,
                        Some(2) => crate::engine::fight::rules::DEFENDER_SIDE_UID,
                        _ => return Err(DrainError::MissingReactionSide(subscriber.owner_uid)),
                    }
                }
            };
            let reentry_target = reentry_skill.and_then(|(owner_uid, skill_id, target_uid)| {
                (owner_uid == subscriber.owner_uid && skill_id == subscriber.skill_id)
                    .then_some(target_uid)
                    .flatten()
            });
            let reenters_current_skill = reentry_skill.is_some_and(|(owner_uid, skill_id, _)| {
                owner_uid == subscriber.owner_uid && skill_id == subscriber.skill_id
            });
            let (frame_path, parent_path) = match frame_scope {
                crate::engine::skill::condition::registry::ReactionFrameScope::Causing => {
                    (reuse_path.or(parent_path).map(|path| path.to_vec()), None)
                }
                crate::engine::skill::condition::registry::ReactionFrameScope::EventSibling => {
                    let parent = parent_path
                        .and_then(|path| (path.len() > 1).then(|| path[..path.len() - 1].to_vec()));
                    (None, parent)
                }
                crate::engine::skill::condition::registry::ReactionFrameScope::Independent => {
                    (None, None)
                }
                crate::engine::skill::condition::registry::ReactionFrameScope::Subscriber
                    if reenters_current_skill =>
                {
                    let parent = reuse_path
                        .and_then(|path| (path.len() > 1).then(|| path[..path.len() - 1].to_vec()));
                    (None, parent)
                }
                crate::engine::skill::condition::registry::ReactionFrameScope::Subscriber => {
                    (None, parent_path.map(|path| path.to_vec()))
                }
            };
            let frame_group = matches!(
                frame_scope,
                crate::engine::skill::condition::registry::ReactionFrameScope::Causing
                    | crate::engine::skill::condition::registry::ReactionFrameScope::Subscriber
            )
            .then(|| {
                skill_groups
                    .entry((frame_source_uid, subscriber.skill_id))
                    .or_default()
                    .clone()
            });
            Ok(QueuedOp {
                op,
                trigger: SkillOpTrigger::Event(event.clone()),
                skill_execution: None,
                frame_path,
                parent_path,
                frame_group,
                independent_parent_group: (frame_scope
                    == crate::engine::skill::condition::registry::ReactionFrameScope::Independent)
                    .then(|| {
                        independent_skill_groups
                            .entry((subscriber.owner_uid, subscriber.skill_id))
                            .or_default()
                            .clone()
                    }),
                frame_owner: Some(FrameOwner::Skill {
                    source_uid: frame_source_uid,
                    skill_id: subscriber.skill_id,
                    card_index: 0,
                    target_uid: reaction_skill_target(
                        pool,
                        event,
                        subscriber.owner_uid,
                        definition.reaction_frame_target,
                        current_skill_target,
                    )
                    .or(reentry_target),
                }),
                subscriber: Some(SubscriberContext::for_skill(
                    subscriber.owner_uid,
                    subscriber.skill_id,
                )),
                caster_frame: None,
            })
        })
        .collect::<Result<Vec<_>, DrainError>>()?;
    for (subscriber, ops) in dispatched.buff_acts {
        let ops = ops.ok_or(DrainError::MissingBuffActOp(
            subscriber.key.definition.opcode,
        ))?;
        let key = required_buff_act_definition(
            Some(subscriber.key.definition.opcode),
            &subscriber.act_type,
        )?
        .key;
        let frame_group = Rc::new(RefCell::new(None));
        let independent_parent_group = Rc::new(RefCell::new(None));
        reactions.extend(ops.into_iter().map(|scoped| {
            let causing_frame =
                scoped.scope == crate::engine::skill::buff_act::BuffActFrameScope::CausingFrame;
            let action_frame =
                scoped.scope == crate::engine::skill::buff_act::BuffActFrameScope::ActionFrame;
            let independent_event = scoped.scope
                == crate::engine::skill::buff_act::BuffActFrameScope::IndependentEvent;
            let source_uid = match scoped.source {
                crate::engine::skill::buff_act::BuffActFrameSource::Counterparty => {
                    reaction_counterparty(pool, event, subscriber.owner_uid)
                        .unwrap_or(subscriber.source_uid)
                }
                crate::engine::skill::buff_act::BuffActFrameSource::EventTarget => {
                    event_target(event).unwrap_or(subscriber.owner_uid)
                }
                crate::engine::skill::buff_act::BuffActFrameSource::Owner => subscriber.owner_uid,
                crate::engine::skill::buff_act::BuffActFrameSource::Applier => {
                    subscriber.source_uid
                }
            };
            QueuedOp {
                op: scoped.op,
                trigger: SkillOpTrigger::Event(event.clone()),
                skill_execution: None,
                frame_path: causing_frame
                    .then(|| reuse_path.or(parent_path).map(|path| path.to_vec()))
                    .flatten(),
                parent_path: (!causing_frame && !independent_event)
                    .then(|| {
                        if action_frame {
                            action_path
                        } else {
                            parent_path
                        }
                        .map(|path| path.to_vec())
                    })
                    .flatten(),
                frame_group: scoped.group_with_siblings.then(|| frame_group.clone()),
                independent_parent_group: (independent_event && scoped.group_with_siblings)
                    .then(|| independent_parent_group.clone()),
                frame_owner: Some(
                    if scoped.frame_owner
                        == crate::engine::skill::buff_act::BuffActFrameOwner::Command
                    {
                        FrameOwner::Command
                    } else if scoped.frame_owner
                        == crate::engine::skill::buff_act::BuffActFrameOwner::Event
                        || scoped.frame_owner
                            == crate::engine::skill::buff_act::BuffActFrameOwner::UntargetedEvent
                        || independent_event && !scoped.group_with_siblings
                    {
                        FrameOwner::EventEffect {
                            source_uid,
                            target_uid: if scoped.frame_owner
                                == crate::engine::skill::buff_act::BuffActFrameOwner::UntargetedEvent
                            {
                                0
                            } else {
                                source_uid
                            },
                        }
                    } else {
                        FrameOwner::BuffAct {
                            owner_uid: subscriber.owner_uid,
                            source_uid,
                            buff_uid: subscriber.buff_uid,
                            buff_id: subscriber.buff_id,
                            key,
                        }
                    },
                ),
                subscriber: Some(SubscriberContext::new(subscriber.owner_uid)),
                caster_frame: None,
            }
        }));
    }
    Ok(reactions)
}

fn event_target(event: &BattleEvent) -> Option<i64> {
    match event {
        BattleEvent::HpLost { target_uid, .. } | BattleEvent::HpHealed { target_uid, .. } => {
            Some(*target_uid)
        }
        BattleEvent::Hit(hit) => Some(hit.target_uid),
        BattleEvent::EntityTransformed { target_uid } => Some(*target_uid),
        BattleEvent::EntityDied(death) => Some(death.target_uid),
        BattleEvent::BuffAdded(change)
        | BattleEvent::BuffChanged(change)
        | BattleEvent::BuffRemoved(change) => Some(change.target_uid),
        BattleEvent::BuffRejected(change) => Some(change.target_uid),
        BattleEvent::BuffStateChanged(change) => Some(change.target_uid),
        BattleEvent::ExPointChanged(change) | BattleEvent::ExPointOverflow(change) => {
            Some(change.target_uid)
        }
        BattleEvent::EurekaChanged(change) => Some(change.target_uid),
        BattleEvent::ConduitActivated(change) => Some(change.source_uid),
        BattleEvent::SkillEffectStarted(action) | BattleEvent::SkillAction(action) => {
            Some(action.target_uid)
        }
        BattleEvent::AllyAction(action) => Some(action.target_uid),
        BattleEvent::BuffFeatureTriggered(trigger) => Some(trigger.target_uid),
        _ => None,
    }
}

pub(super) fn reaction_counterparty(
    pool: &TargetPool,
    event: &BattleEvent,
    owner_uid: i64,
) -> Option<i64> {
    let observer_inherits_target = matches!(
        event,
        BattleEvent::SkillEffectStarted(_)
            | BattleEvent::SkillAction(_)
            | BattleEvent::AllyAction(_)
            | BattleEvent::BuffAdded(_)
            | BattleEvent::BuffChanged(_)
            | BattleEvent::BuffStateChanged(_)
            | BattleEvent::BuffRemoved(_)
            | BattleEvent::BuffRejected(_)
    );
    let (source_uid, target_uid) = match event {
        BattleEvent::SkillEffectStarted(action) | BattleEvent::SkillAction(action) => {
            (action.source_uid, action.target_uid)
        }
        BattleEvent::AllyAction(action) => (action.source_uid, action.target_uid),
        BattleEvent::BuffFeatureTriggered(trigger) => (trigger.owner_uid, trigger.target_uid),
        BattleEvent::BuffAdded(change)
        | BattleEvent::BuffChanged(change)
        | BattleEvent::BuffRemoved(change) => (change.source_uid, change.target_uid),
        BattleEvent::BuffRejected(change) => (change.source_uid, change.target_uid),
        BattleEvent::BuffStateChanged(change) => (change.source_uid, change.target_uid),
        BattleEvent::HpLost {
            source_uid,
            target_uid,
            ..
        } => (*source_uid, *target_uid),
        BattleEvent::HpHealed {
            source_uid,
            target_uid,
            ..
        } => (*source_uid, *target_uid),
        BattleEvent::Hit(hit) => (hit.source_uid, hit.target_uid),
        BattleEvent::DamageShared {
            source_uid,
            target_uid,
            ..
        } => (*source_uid, *target_uid),
        BattleEvent::EntityDied(death) => (death.source_uid, death.target_uid),
        BattleEvent::ExPointChanged(change) | BattleEvent::ExPointOverflow(change) => {
            (change.source_uid, change.target_uid)
        }
        BattleEvent::EurekaChanged(change) => (change.source_uid, change.target_uid),
        BattleEvent::ConduitActivated(change) => (change.source_uid, change.source_uid),
        _ => return None,
    };
    if owner_uid == source_uid {
        Some(target_uid)
    } else if owner_uid == target_uid {
        Some(source_uid)
    } else if pool
        .team_type(owner_uid)
        .is_some_and(|team| pool.team_type(source_uid) == Some(team))
    {
        Some(target_uid)
    } else if pool
        .team_type(owner_uid)
        .is_some_and(|team| pool.team_type(target_uid) == Some(team))
    {
        Some(source_uid)
    } else {
        observer_inherits_target.then_some(target_uid)
    }
}

pub(super) fn reaction_skill_target(
    pool: &TargetPool,
    event: &BattleEvent,
    owner_uid: i64,
    target: crate::engine::skill::condition::registry::ReactionFrameTarget,
    current_skill_target: Option<i64>,
) -> Option<i64> {
    use crate::engine::skill::condition::registry::ReactionFrameTarget;

    match target {
        ReactionFrameTarget::Counterparty => reaction_counterparty(pool, event, owner_uid),
        ReactionFrameTarget::Owner => Some(owner_uid),
        ReactionFrameTarget::EventTarget => event.target_uid(),
        ReactionFrameTarget::CausingFrame => current_skill_target.or_else(|| event.target_uid()),
    }
}
