use super::*;

const ULTIMATE_AVAILABILITY_ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::Lifecycle,
    key: DefinitionKey::new(0, "UltimateAvailability"),
};

pub(super) fn queued_entity_entry_halo_fanout(
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
    let current = current?;
    if entity.model_id == 3088 {
        tracing::info!(moxie_before = change.before, moxie_after = change.after,
            moxie_now = managers.ex_point.get(entity.uid),
            required = mechanic.required_ultimate_cost(managers, entity),
            blocked = managers.buff.has_buff_act_kind(entity.uid,
                crate::engine::skill::buff_act::registry::BuffActKind::CantGetExskill),
            "Semmelweis ultimate card removal queued");
    }
    Some(QueuedOp {
        op: RuleOp::Command(BattleCommand::Card(CardCommand::SetUltimateAvailability(
            CardSetUltimateAvailability {
                origin: ULTIMATE_AVAILABILITY_ORIGIN,
                card: current,
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

pub(super) fn route_skill_start_settlement(
    event: &BattleEvent,
    reuse_path: Option<&[usize]>,
    reactions: &mut [QueuedOp],
) -> Option<QueuedOp> {
    let BattleEvent::SkillEffectStarted(started) = event else {
        return None;
    };
    let origin = started.hand_settlement?;
    let reaction = reactions.iter_mut().rev().find(|queued| {
        queued
            .subscriber
            .as_ref()
            .is_some_and(|subscriber| subscriber.owner_uid == started.source_uid)
            && matches!(queued.op, RuleOp::Skill(_))
    });
    let Some(reaction) = reaction else {
        let reuse_path = reuse_path?;
        return Some(QueuedOp {
            op: RuleOp::ActionHandSettlement { origin },
            trigger: SkillOpTrigger::Event(event.clone()),
            skill_execution: None,
            frame_path: Some(reuse_path.to_vec()),
            parent_path: None,
            frame_group: None,
            independent_parent_group: None,
            frame_owner: None,
            subscriber: None,
            caster_frame: None,
        });
    };
    let RuleOp::Skill(invocation) = &mut reaction.op else {
        unreachable!("actor reaction filter only selects skill invocations");
    };
    invocation.action_hand_settlement = Some(origin);
    None
}

#[allow(clippy::too_many_arguments)]
pub(in crate::engine::runtime::drain) fn dispatch_event_groups(
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
    let mut all_events = event_groups.iter().flatten().cloned().collect::<Vec<_>>();
    drain_state.record_action_shared_damage(action_path, &mut all_events);
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
    let mut event_offset = 0;
    for events in event_groups {
        let events = &all_events[event_offset..event_offset + events.len()];
        event_offset += events.len();
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
pub(in crate::engine::runtime::drain) fn dispatch_event_batch(
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
    let mut events = events.to_vec();
    DrainState::default().record_action_shared_damage(None, &mut events);
    let mut reactions = dispatch_event_batch_inner(
        pool,
        managers,
        catalog,
        determinism,
        &events,
        &events,
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
                    reactions
                        .before_after_hit_observers
                        .push(queued_buff_act_feature_op(feature, op, event, parent_path)?);
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

pub(in crate::engine::runtime::drain) fn ordered_hit_entities(
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
