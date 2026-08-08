use super::*;
use crate::engine::{
    manager::card::{CardCommand, CardSetUltimateAvailability},
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain, output::BattleCommand},
};

const ULTIMATE_AVAILABILITY_ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::Lifecycle,
    key: DefinitionKey::new(0, "UltimateAvailability"),
};

fn queued_ultimate_availability_sync(
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
        .find(|card| mechanic.is_ultimate(card, entity))
        .cloned();
    let (card, available) = if mechanic.can_add_normal_ultimate(managers, entity) {
        (
            crate::engine::manager::card::pool::card_for_target(entity, entity.ex_skill)?,
            true,
        )
    } else if !mechanic.ultimate_ready(managers, entity) {
        (current?, false)
    } else {
        return None;
    };
    Some(QueuedOp {
        op: RuleOp::Command(BattleCommand::Card(CardCommand::SetUltimateAvailability(
            CardSetUltimateAvailability {
                origin: ULTIMATE_AVAILABILITY_ORIGIN,
                card,
                available,
            },
        ))),
        trigger: SkillOpTrigger::Event(event.clone()),
        skill_execution: None,
        frame_path: Some(reuse_path.to_vec()),
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: Some(FrameOwner::EventRule),
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
    })
}

#[allow(clippy::too_many_arguments)]
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
    let scoped_owner_uids = terminal_owner_scope(pool, managers, owner_uids);
    let owner_uids = scoped_owner_uids.as_deref();
    let after_publish =
        publication_phase == crate::engine::event::subscription::PublicationPhase::AfterPublish;
    let attack_sources = if include_attack_consumption && after_publish {
        ordered_hit_entities(events, |hit| hit.source_uid)
    } else {
        Vec::new()
    };
    let attacked_targets = if include_attack_consumption && after_publish {
        ordered_hit_entities(events, |hit| hit.target_uid)
    } else {
        Vec::new()
    };
    let mut queued_attack_consumption = false;
    let mut fired_once_per_target = std::collections::HashSet::new();
    let mut reactions = ReactionBatch::default();
    for event in events {
        if after_publish
            && owner_uids.is_none_or(|owners| {
                event
                    .target_uid()
                    .is_none_or(|target_uid| owners.contains(&target_uid))
            })
            && let Some(sync) = queued_ultimate_availability_sync(pool, managers, event, reuse_path)
        {
            reactions.after_publish.push(sync);
        }
        if after_publish && !queued_attack_consumption && matches!(event, BattleEvent::Hit(_)) {
            for source_uid in attack_sources
                .iter()
                .filter(|uid| owner_uids.is_none_or(|owners| owners.contains(uid)))
            {
                for (feature, op) in crate::engine::skill::buff_act::attack_consumption_rule_ops(
                    managers,
                    *source_uid,
                    current_skill.is_some_and(|(_, skill_id, _)| catalog.is_big_skill(skill_id)),
                ) {
                    reactions.after_publish.push(queued_buff_act_feature_op(
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
                let damage_types = events
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
            queued_attack_consumption = true;
        }
        let reentry_skill = current_skill.filter(|_| {
            matches!(
                event.kind(),
                crate::engine::event::kind::EventKind::BuffAdded
                    | crate::engine::event::kind::EventKind::BuffRejected
            )
        });
        let mut dispatched = dispatch_reactions(
            pool,
            managers,
            catalog,
            determinism,
            event,
            Some(parent_path),
            Some(reuse_path),
            action_path,
            reentry_skill,
            None,
            owner_uids,
            execute_unscoped_after_action,
            Some(publication_phase),
        )?;
        retain_event_multiplicity(&mut dispatched, event, &mut fired_once_per_target);
        reactions.before_publish.extend(dispatched.before_publish);
        reactions.after_publish.extend(dispatched.after_publish);
        reactions.after_skill.extend(dispatched.after_skill);
        reactions.after_hit.extend(dispatched.after_hit);
        reactions.after_action.extend(dispatched.after_action);
    }
    plan_raw_gauge_contributions(managers, &mut reactions.before_publish)?;
    plan_raw_gauge_contributions(managers, &mut reactions.after_publish)?;
    plan_raw_gauge_contributions(managers, &mut reactions.after_skill)?;
    plan_raw_gauge_contributions(managers, &mut reactions.after_hit)?;
    plan_raw_gauge_contributions(managers, &mut reactions.after_action)?;
    Ok(reactions)
}

type OncePerTargetKey = (
    i64,
    i64,
    crate::engine::skill::rule::DefinitionKey,
    i64,
    i64,
);

fn retain_event_multiplicity(
    reactions: &mut ReactionBatch,
    event: &BattleEvent,
    fired: &mut std::collections::HashSet<OncePerTargetKey>,
) {
    let current = reactions
        .before_publish
        .iter()
        .chain(&reactions.after_publish)
        .chain(&reactions.after_hit)
        .chain(&reactions.after_action)
        .filter_map(|queued| once_per_target_key(queued, event))
        .collect::<std::collections::HashSet<_>>();
    reactions.before_publish.retain(|queued| {
        once_per_target_key(queued, event).is_none_or(|key| !fired.contains(&key))
    });
    reactions.after_publish.retain(|queued| {
        once_per_target_key(queued, event).is_none_or(|key| !fired.contains(&key))
    });
    reactions.after_hit.retain(|queued| {
        once_per_target_key(queued, event).is_none_or(|key| !fired.contains(&key))
    });
    reactions.after_action.retain(|queued| {
        once_per_target_key(queued, event).is_none_or(|key| !fired.contains(&key))
    });
    fired.extend(current);
}

fn once_per_target_key(queued: &QueuedOp, event: &BattleEvent) -> Option<OncePerTargetKey> {
    let BattleEvent::Hit(hit) = event else {
        return None;
    };
    let Some(FrameOwner::BuffAct {
        owner_uid,
        buff_uid,
        key,
        ..
    }) = queued.frame_owner
    else {
        return None;
    };
    let definition = crate::engine::skill::buff_act::registry::find(key.opcode, key.type_name)?;
    (definition.runtime.event_multiplicity
        == crate::engine::skill::buff_act::registry::RuntimeEventMultiplicity::OncePerActionTarget)
        .then_some((owner_uid, buff_uid, key, hit.source_uid, hit.target_uid))
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

fn plan_raw_gauge_contributions(
    managers: &BattleManagers,
    queued: &mut [QueuedOp],
) -> Result<(), DrainError> {
    let mut keys = Vec::new();
    for item in queued.iter() {
        let RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Gauge(command)) =
            &item.op
        else {
            continue;
        };
        if matches!(
            command.operation,
            crate::engine::manager::gauge::GaugeOperation::AccumulateRawValue { .. }
        ) && !keys.contains(&command.key)
        {
            keys.push(command.key);
        }
    }

    for key in keys {
        let raw_amounts = queued
            .iter()
            .filter_map(|item| {
                let RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Gauge(
                    command,
                )) = &item.op
                else {
                    return None;
                };
                match command.operation {
                    crate::engine::manager::gauge::GaugeOperation::AccumulateRawValue {
                        amount,
                        ..
                    } if command.key == key => Some(amount),
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        if raw_amounts.len() < 2 {
            continue;
        }
        let Some(deltas) = managers.gauge.plan_raw_contributions(key, &raw_amounts) else {
            continue;
        };
        if deltas.len() != raw_amounts.len() {
            return Err(DrainError::InvalidGaugeContributionPlan {
                key,
                expected: raw_amounts.len(),
                actual: deltas.len(),
            });
        }
        let actual = deltas.len();
        let mut deltas = deltas.into_iter();
        for item in queued.iter_mut() {
            let RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Gauge(command)) =
                &mut item.op
            else {
                continue;
            };
            let crate::engine::manager::gauge::GaugeOperation::AccumulateRawValue {
                amount, ..
            } = command.operation
            else {
                continue;
            };
            if command.key == key {
                let value_delta =
                    deltas
                        .next()
                        .ok_or(DrainError::InvalidGaugeContributionPlan {
                            key,
                            expected: raw_amounts.len(),
                            actual,
                        })?;
                command.operation =
                    crate::engine::manager::gauge::GaugeOperation::ApplyRawContribution {
                        raw_amount: amount,
                        value_delta,
                    };
            }
        }
    }
    Ok(())
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
    reentry_skill: Option<(i64, i32, Option<i64>)>,
    lane: Option<ReactionLane>,
    owner_uids: Option<&[i64]>,
    execute_unscoped_after_action: bool,
    publication_phase: Option<crate::engine::event::subscription::PublicationPhase>,
) -> Result<ReactionBatch, DrainError> {
    let mut reactions = ReactionBatch::default();
    let transformed_owner = match event {
        BattleEvent::EntityTransformed { target_uid } => Some([*target_uid]),
        _ => None,
    };
    let requested_owner_uids =
        owner_uids.or_else(|| transformed_owner.as_ref().map(|owners| owners.as_slice()));
    let scoped_owner_uids = terminal_owner_scope(pool, managers, requested_owner_uids);
    let owner_uids = scoped_owner_uids.as_deref();
    let transaction_ops = if !matches!(lane, Some(ReactionLane::Skills)) {
        crate::engine::skill::buff_act::transaction_rule_ops(managers, event)
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
        let timing = definition.runtime.execution_timing;
        let publication =
            crate::engine::skill::buff_act::transaction_publication(&feature, &op, event.kind())
                .ok_or_else(|| DrainError::MissingBuffActDefinition {
                    opcode: feature.act_id(),
                    type_name: feature.act_type.clone(),
                })?;
        let selected_for_pass = publication_phase.is_none_or(|phase| {
            if timing
                == crate::engine::skill::buff_act::registry::RuntimeExecutionTiming::AfterAction
            {
                phase == crate::engine::event::subscription::PublicationPhase::AfterPublish
            } else {
                publication == phase
            }
        });
        if !selected_for_pass {
            continue;
        }
        if timing == crate::engine::skill::buff_act::registry::RuntimeExecutionTiming::AfterAction
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
                let parent = if timing
                    == crate::engine::skill::buff_act::registry::RuntimeExecutionTiming::AfterAction
                {
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

        let frame_group = (frame_scope
            == crate::engine::skill::buff_act::registry::RuntimeFrameScope::SubscriberFrame)
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
        };
        if timing == crate::engine::skill::buff_act::registry::RuntimeExecutionTiming::AfterAction {
            reactions.after_action.push(queued);
        } else if publication == crate::engine::event::subscription::PublicationPhase::BeforePublish
        {
            reactions.before_publish.push(queued);
        } else {
            reactions.after_publish.push(queued);
        }
    }
    let mut dispatched = match (owner_uids, publication_phase) {
        (Some(owner_uids), Some(publication)) => dispatcher::dispatch_owner_event_phase(
            pool,
            managers,
            catalog,
            determinism,
            event,
            owner_uids,
            publication,
        )?,
        (None, Some(publication)) => dispatcher::dispatch_event_phase(
            pool,
            managers,
            catalog,
            determinism,
            event,
            publication,
        )?,
        (Some(owner_uids), None) => dispatcher::dispatch_owner_event(
            pool,
            managers,
            catalog,
            determinism,
            event,
            owner_uids,
        )?,
        (None, None) => dispatcher::dispatch_event(pool, managers, catalog, determinism, event)?,
    };
    if event.kind() == crate::engine::event::kind::EventKind::BuffRejected
        && let (Some((owner_uid, skill_id, _)), Some(publication)) =
            (reentry_skill, publication_phase)
    {
        let current = dispatcher::dispatch_skill_event_phase(
            pool,
            managers,
            catalog,
            determinism,
            (owner_uid, skill_id),
            event,
            publication,
        )?;
        for skill in current.skills {
            if !dispatched.skills.contains(&skill) {
                dispatched.skills.push(skill);
            }
        }
    }
    match lane {
        Some(ReactionLane::Skills) => dispatched.buff_acts.clear(),
        Some(ReactionLane::BuffActs) => dispatched.skills.clear(),
        Some(ReactionLane::BuffActsBeforeSettlement) => {
            dispatched.skills.clear();
            dispatched.buff_acts.retain(|(subscriber, _)| {
                reaction_lane_accepts_buff_act(
                    lane,
                    subscriber.key.definition.opcode,
                    &subscriber.act_type,
                )
            });
        }
        Some(ReactionLane::BuffActsAfterSettlement) => {
            dispatched.skills.clear();
            dispatched.buff_acts.retain(|(subscriber, _)| {
                reaction_lane_accepts_buff_act(
                    lane,
                    subscriber.key.definition.opcode,
                    &subscriber.act_type,
                )
            });
        }
        None => {}
    }
    let (before_publish, after_publish, after_skill) = split_skills_by_timing(dispatched);
    reactions.before_publish.extend(queued_reactions(
        pool,
        before_publish,
        event,
        parent_path,
        reuse_path,
        action_path,
        reentry_skill,
    )?);
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
                    }),
            );
    }
    reactions.after_publish.extend(queued_reactions(
        pool,
        after_publish,
        event,
        parent_path,
        reuse_path,
        action_path,
        reentry_skill,
    )?);
    reactions.after_skill.extend(queued_reactions(
        pool,
        after_skill,
        event,
        parent_path,
        reuse_path,
        action_path,
        reentry_skill,
    )?);
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
            }));
    }
    Ok(reactions)
}

fn reaction_lane_accepts_buff_act(lane: Option<ReactionLane>, act_id: i32, act_type: &str) -> bool {
    use crate::engine::skill::buff_act::registry::RuntimeSettlementPhase;

    match lane {
        Some(ReactionLane::BuffActsBeforeSettlement) => {
            crate::engine::skill::buff_act::registry::runtime_settlement_phase(act_id, act_type)
                == RuntimeSettlementPhase::Before
        }
        Some(ReactionLane::BuffActsAfterSettlement) => {
            crate::engine::skill::buff_act::registry::runtime_settlement_phase(act_id, act_type)
                == RuntimeSettlementPhase::After
        }
        _ => true,
    }
}

fn split_skills_by_timing(
    dispatched: dispatcher::DispatchBatch,
) -> (
    dispatcher::DispatchBatch,
    dispatcher::DispatchBatch,
    dispatcher::DispatchBatch,
) {
    use crate::engine::event::subscription::{PublicationPhase, ReactionTiming};

    let mut before = dispatcher::DispatchBatch::default();
    let mut after = dispatcher::DispatchBatch::default();
    let mut after_skill = dispatcher::DispatchBatch::default();
    for skill in dispatched.skills {
        match (skill.0.key.timing, skill.0.key.publication) {
            (ReactionTiming::AfterSkill, _) => after_skill.skills.push(skill),
            (_, PublicationPhase::BeforePublish) => before.skills.push(skill),
            (_, PublicationPhase::AfterPublish) => after.skills.push(skill),
        }
    }
    for buff_act in dispatched.buff_acts {
        match buff_act.0.key.publication {
            PublicationPhase::BeforePublish => before.buff_acts.push(buff_act),
            PublicationPhase::AfterPublish => after.buff_acts.push(buff_act),
        }
    }
    (before, after, after_skill)
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
    queued_reactions(pool, dispatched, event, None, None, None, None)
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

pub(super) fn queued_reactions(
    pool: &TargetPool,
    dispatched: dispatcher::DispatchBatch,
    event: &BattleEvent,
    parent_path: Option<&[usize]>,
    reuse_path: Option<&[usize]>,
    action_path: Option<&[usize]>,
    reentry_skill: Option<(i64, i32, Option<i64>)>,
) -> Result<Vec<QueuedOp>, DrainError> {
    let mut skill_groups = HashMap::<(i64, i32), Rc<RefCell<Option<FramePath>>>>::new();
    let mut reactions = dispatched
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
            let frame_scope = definition.reaction_frame_scope;
            let reentry_target = reentry_skill.and_then(|(owner_uid, skill_id, target_uid)| {
                (owner_uid == subscriber.owner_uid && skill_id == subscriber.skill_id)
                    .then_some(target_uid)
                    .flatten()
            });
            let reenters_current_skill = reentry_skill.is_some_and(|(owner_uid, skill_id, _)| {
                owner_uid == subscriber.owner_uid && skill_id == subscriber.skill_id
            });
            let (frame_path, parent_path) = if frame_scope
                == crate::engine::skill::condition::registry::ReactionFrameScope::Causing
            {
                (reuse_path.or(parent_path).map(|path| path.to_vec()), None)
            } else if reenters_current_skill {
                let parent = reuse_path
                    .and_then(|path| (path.len() > 1).then(|| path[..path.len() - 1].to_vec()));
                (None, parent)
            } else {
                (None, parent_path.map(|path| path.to_vec()))
            };
            let frame_group = (frame_scope
                == crate::engine::skill::condition::registry::ReactionFrameScope::Subscriber)
                .then(|| {
                    skill_groups
                        .entry((subscriber.owner_uid, subscriber.skill_id))
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
                independent_parent_group: None,
                frame_owner: Some(FrameOwner::Skill {
                    source_uid: subscriber.owner_uid,
                    skill_id: subscriber.skill_id,
                    card_index: 0,
                    target_uid: reentry_target.or_else(|| {
                        reaction_skill_target(
                            pool,
                            event,
                            subscriber.owner_uid,
                            definition.reaction_frame_target,
                        )
                    }),
                }),
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
                frame_group: (!causing_frame && scoped.group_with_siblings)
                    .then(|| frame_group.clone()),
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
) -> Option<i64> {
    use crate::engine::skill::condition::registry::ReactionFrameTarget;

    match target {
        ReactionFrameTarget::Counterparty => reaction_counterparty(pool, event, owner_uid),
        ReactionFrameTarget::Owner => Some(owner_uid),
    }
}
