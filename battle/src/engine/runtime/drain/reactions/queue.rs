use super::*;

#[allow(clippy::too_many_arguments)]
pub(in crate::engine::runtime::drain) fn queued_reactions(
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
            let multiplicity_policy = crate::engine::skill::condition::registry::find_key(
                subscriber.key.multiplicity_policy.opcode,
                subscriber.key.multiplicity_policy.type_name,
            )
            .ok_or_else(|| {
                DrainError::Subscriber(
                    crate::engine::skill::subscriber::SubscriberError::UncompiledRoute {
                        skill_id: subscriber.skill_id,
                        route:
                            crate::engine::skill::rule::route::RouteError::UnregisteredExactKey {
                                opcode: subscriber.key.multiplicity_policy.opcode,
                                type_name: subscriber.key.multiplicity_policy.type_name.to_owned(),
                            },
                    },
                )
            })?;
            Ok((subscriber, op, definition, multiplicity_policy))
        })
        .collect::<Result<Vec<_>, DrainError>>()?;
    let mut reactions = skills
        .into_iter()
        .map(|(subscriber, mut op, definition, multiplicity_policy)| {
            if multiplicity_policy.reaction_multiplicity
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
                    | crate::engine::skill::condition::registry::ReactionFrameScope::EventSibling
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
        BattleEvent::SkillEffectStarted(started) => Some(started.target_uid),
        BattleEvent::SkillAction(action) => Some(action.target_uid),
        BattleEvent::AllyAction(action) => Some(action.target_uid),
        BattleEvent::BuffFeatureTriggered(trigger) => Some(trigger.target_uid),
        _ => None,
    }
}

pub(in crate::engine::runtime::drain) fn reaction_counterparty(
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
        BattleEvent::SkillEffectStarted(started) => (started.source_uid, started.target_uid),
        BattleEvent::SkillAction(action) => (action.source_uid, action.target_uid),
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
    if crate::engine::fight::rules::is_side_uid(target_uid) {
        return Some(source_uid);
    }
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

pub(in crate::engine::runtime::drain) fn reaction_skill_target(
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
