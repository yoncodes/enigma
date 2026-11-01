use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::BattleManagers,
    skill::{
        action::{SkillInvocation, SkillRequest},
        buff_act,
        effect::SkillEffectCatalog,
        rule::{SetupStage, output::RuleOp},
        subscriber::{self, BuffActSubscriber, SetupSubscriber, SkillSubscriber, SubscriberError},
        target::{TargetContext, TargetPool, TargetResolver},
    },
};

#[derive(Debug, Default)]
pub struct DispatchBatch {
    pub skills: Vec<(SkillSubscriber, RuleOp)>,
    pub buff_acts: Vec<(BuffActSubscriber, Option<Vec<buff_act::BuffActRuleOp>>)>,
    pub(crate) following: Vec<DispatchBatch>,
}

impl DispatchBatch {
    pub(crate) fn passes(&self) -> impl Iterator<Item = &Self> {
        std::iter::once(self).chain(&self.following)
    }

    pub(crate) fn into_passes(self) -> impl Iterator<Item = Self> {
        std::iter::once(Self {
            skills: self.skills,
            buff_acts: self.buff_acts,
            following: Vec::new(),
        })
        .chain(self.following)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactionLane {
    Skills,
    BuffActs,
    BuffActsBeforeSettlement,
    BuffActsAfterSettlement,
}

pub(crate) struct DispatchSelection<'a> {
    pub owner_uids: Option<&'a [i64]>,
    pub publication: Option<crate::engine::event::subscription::PublicationPhase>,
    pub lane: Option<ReactionLane>,
    pub timing: crate::engine::event::subscription::ReactionTiming,
}

pub(crate) type ReactionMultiplicityKey = (
    i64,
    i32,
    Option<usize>,
    crate::engine::skill::rule::DefinitionKey,
);
pub(crate) type OncePerTargetKey = (
    i64,
    i64,
    crate::engine::skill::rule::DefinitionKey,
    i64,
    i64,
);

pub(crate) struct DispatchMultiplicity<'a> {
    pub once_per_target: &'a mut std::collections::HashSet<OncePerTargetKey>,
    pub once_per_event_batch: &'a mut std::collections::HashSet<ReactionMultiplicityKey>,
    pub once_per_action: &'a mut std::collections::HashSet<ReactionMultiplicityKey>,
    pub event_batch_id: u64,
    pub action_path: Option<&'a [usize]>,
}

pub(crate) struct DispatchOwnerPhase<'a> {
    pub owner_uids: &'a [i64],
    pub publication: crate::engine::event::subscription::PublicationPhase,
}

pub(crate) struct DispatchSkillSelection {
    pub owner_skill: (i64, i32),
    pub publication: crate::engine::event::subscription::PublicationPhase,
    pub timing: crate::engine::event::subscription::ReactionTiming,
}

#[cfg(test)]
pub fn dispatch(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    event: EventKind,
) -> DispatchBatch {
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    outputs(
        subscriber::for_event(pool, managers, catalog, event),
        managers,
        pool,
        catalog,
        &mut determinism,
        None,
    )
}

fn outputs(
    subscribers: subscriber::EventSubscribers,
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut crate::engine::runtime::determinism::RoundDeterminism,
    event: Option<&BattleEvent>,
) -> DispatchBatch {
    let skills = subscribers
        .skills
        .into_iter()
        .map(|subscriber| {
            let plan = SkillRequest {
                source_uid: subscriber.owner_uid,
                skill_id: subscriber.skill_id,
            };
            let output = RuleOp::Skill(SkillInvocation {
                plan,
                condition_key: Some(subscriber.key.definition),
                reaction_multiplicity_key: Some(subscriber.key.multiplicity_policy),
                condition_slot: subscriber.slot_index,
                phase: subscriber.key.phase,
                target: crate::engine::skill::action::SkillTarget::Inherited,
                card_enchants: event.and_then(current_card_enchants).unwrap_or_default(),
                ..plan.into()
            });
            (subscriber, output)
        })
        .collect();
    let buff_acts = subscribers
        .buff_acts
        .into_iter()
        .filter(|subscriber| buff_act::subscriber_kind(subscriber).is_some())
        .map(|subscriber| {
            let output =
                buff_act::rule_ops(managers, pool, catalog, determinism, &subscriber, event);
            (subscriber, output)
        })
        .collect();
    DispatchBatch {
        skills,
        buff_acts,
        following: Vec::new(),
    }
}

fn output_passes(
    subscribers: subscriber::EventSubscribers,
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut crate::engine::runtime::determinism::RoundDeterminism,
    event: &BattleEvent,
) -> DispatchBatch {
    let passes = ordered_subscriber_passes(subscribers, event);
    let mut passes = passes.into_iter().map(|subscribers| {
        outputs(
            subscribers,
            managers,
            pool,
            catalog,
            determinism,
            Some(event),
        )
    });
    let mut first = passes.next().unwrap_or_default();
    first.following.extend(passes);
    first
}

fn ordered_subscriber_passes(
    subscribers: subscriber::EventSubscribers,
    event: &BattleEvent,
) -> Vec<subscriber::EventSubscribers> {
    match event {
        BattleEvent::SkillEffectStarted(action) => {
            let (actor_skills, other_skills) = subscribers
                .skills
                .into_iter()
                .partition(|subscriber| subscriber.owner_uid == action.source_uid);
            let (actor_buff_acts, other_buff_acts) = subscribers
                .buff_acts
                .into_iter()
                .partition(|subscriber| subscriber.owner_uid == action.source_uid);
            vec![
                subscriber::EventSubscribers {
                    skills: other_skills,
                    buff_acts: other_buff_acts,
                },
                subscriber::EventSubscribers {
                    skills: actor_skills,
                    buff_acts: actor_buff_acts,
                },
            ]
        }
        BattleEvent::EntityDied(_) => {
            let (target_skills, observer_skills): (Vec<_>, Vec<_>) =
                subscribers.skills.into_iter().partition(|subscriber| {
                    crate::engine::skill::condition::registry::find_key(
                        subscriber.key.definition.opcode,
                        subscriber.key.definition.type_name,
                    )
                    .is_some_and(|definition| {
                        definition.reaction_observer_scope
                            == crate::engine::skill::condition::registry::ReactionObserverScope::EventTarget
                    })
                });
            let (subscriber_skills, side_skills) = observer_skills.into_iter().partition(
                |subscriber| {
                    crate::engine::skill::condition::registry::find_key(
                        subscriber.key.definition.opcode,
                        subscriber.key.definition.type_name,
                    )
                    .is_none_or(|definition| {
                        definition.reaction_frame_source
                            == crate::engine::skill::condition::registry::ReactionFrameSource::Subscriber
                    })
                },
            );
            vec![
                subscriber::EventSubscribers {
                    skills: target_skills,
                    buff_acts: subscribers.buff_acts,
                },
                subscriber::EventSubscribers {
                    skills: subscriber_skills,
                    buff_acts: Vec::new(),
                },
                subscriber::EventSubscribers {
                    skills: side_skills,
                    buff_acts: Vec::new(),
                },
            ]
        }
        _ => vec![subscribers],
    }
}

fn current_card_enchants(event: &BattleEvent) -> Option<Vec<i32>> {
    match event.skill_action_event() {
        Some(action) => Some(action.card_enchants.clone()),
        None => match event {
            BattleEvent::AllyAction(action) => Some(action.card_enchants.clone()),
            _ => None,
        },
    }
}

pub fn dispatch_event(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut crate::engine::runtime::determinism::RoundDeterminism,
    event: &BattleEvent,
) -> Result<DispatchBatch, SubscriberError> {
    subscriber::for_compiled_events(pool, managers, catalog, event.subscription_kinds()).map(
        |subscribers| {
            dispatch_subscribers(subscribers, pool, managers, catalog, determinism, event)
        },
    )
}

pub(crate) fn dispatch_selected_event(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut crate::engine::runtime::determinism::RoundDeterminism,
    event: &BattleEvent,
    selection: DispatchSelection<'_>,
    multiplicity: Option<&mut DispatchMultiplicity<'_>>,
) -> Result<DispatchBatch, SubscriberError> {
    let mut subscribers = subscriber::for_compiled_selected_events(
        pool, managers, catalog, event.subscription_kinds(), selection.owner_uids,
        !matches!(selection.lane, Some(ReactionLane::BuffActs | ReactionLane::BuffActsBeforeSettlement | ReactionLane::BuffActsAfterSettlement)),
        !matches!(selection.lane, Some(ReactionLane::Skills)),
    )?;
    if let Some(publication) = selection.publication {
        retain_publication(&mut subscribers, publication);
    }
    retain_reaction_lane(&mut subscribers, selection.lane);
    retain_reaction_timing(
        &mut subscribers,
        selection.timing,
        event,
        matches!(selection.lane, Some(ReactionLane::Skills)),
    );
    Ok(if let Some(multiplicity) = multiplicity {
        dispatch_subscribers_with_multiplicity(
            subscribers,
            pool,
            managers,
            catalog,
            determinism,
            event,
            multiplicity,
        )
    } else {
        dispatch_subscribers(subscribers, pool, managers, catalog, determinism, event)
    })
}

pub fn dispatch_event_phase(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut crate::engine::runtime::determinism::RoundDeterminism,
    event: &BattleEvent,
    publication: crate::engine::event::subscription::PublicationPhase,
) -> Result<DispatchBatch, SubscriberError> {
    subscriber::for_compiled_events(pool, managers, catalog, event.subscription_kinds()).map(
        |mut subscribers| {
            retain_publication(&mut subscribers, publication);
            dispatch_subscribers(subscribers, pool, managers, catalog, determinism, event)
        },
    )
}

pub(crate) fn dispatch_event_phase_with_multiplicity(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut crate::engine::runtime::determinism::RoundDeterminism,
    event: &BattleEvent,
    publication: crate::engine::event::subscription::PublicationPhase,
    multiplicity: &mut DispatchMultiplicity<'_>,
) -> Result<DispatchBatch, SubscriberError> {
    subscriber::for_compiled_events(pool, managers, catalog, event.subscription_kinds()).map(
        |mut subscribers| {
            retain_publication(&mut subscribers, publication);
            dispatch_subscribers_with_multiplicity(
                subscribers,
                pool,
                managers,
                catalog,
                determinism,
                event,
                multiplicity,
            )
        },
    )
}

pub(crate) fn dispatch_selected_skill_event(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut crate::engine::runtime::determinism::RoundDeterminism,
    event: &BattleEvent,
    selection: DispatchSkillSelection,
) -> Result<DispatchBatch, SubscriberError> {
    let (owner_uid, skill_id) = selection.owner_skill;
    let mut skills = Vec::new();
    for event_kind in event.subscription_kinds() {
        for subscriber in subscriber::for_compiled_skill_event(
            managers, catalog, owner_uid, skill_id, event_kind,
        )? {
            if !skills.contains(&subscriber) {
                skills.push(subscriber);
            }
        }
    }
    let mut subscribers = subscriber::EventSubscribers {
        skills,
        buff_acts: Vec::new(),
    };
    retain_publication(&mut subscribers, selection.publication);
    retain_reaction_timing(&mut subscribers, selection.timing, event, false);
    Ok(dispatch_subscribers(
        subscribers,
        pool,
        managers,
        catalog,
        determinism,
        event,
    ))
}

fn dispatch_subscribers(
    mut subscribers: subscriber::EventSubscribers,
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut crate::engine::runtime::determinism::RoundDeterminism,
    event: &BattleEvent,
) -> DispatchBatch {
    retain_observing_subscribers(
        &mut subscribers,
        pool,
        managers,
        catalog,
        determinism,
        event,
    );
    if crate::engine::diagnostics::enabled(crate::engine::diagnostics::TraceArea::Event) {
        eprintln!(
            "event {event:?} skills={:?} buff_acts={:?}",
            subscribers
                .skills
                .iter()
                .map(|subscriber| (
                    subscriber.owner_uid,
                    subscriber.skill_id,
                    subscriber.key.definition.opcode
                ))
                .collect::<Vec<_>>(),
            subscribers
                .buff_acts
                .iter()
                .map(|subscriber| (subscriber.owner_uid, subscriber.buff_id))
                .collect::<Vec<_>>(),
        );
    }
    output_passes(subscribers, managers, pool, catalog, determinism, event)
}

fn dispatch_subscribers_with_multiplicity(
    mut subscribers: subscriber::EventSubscribers,
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut crate::engine::runtime::determinism::RoundDeterminism,
    event: &BattleEvent,
    multiplicity: &mut DispatchMultiplicity<'_>,
) -> DispatchBatch {
    retain_observing_subscribers(
        &mut subscribers,
        pool,
        managers,
        catalog,
        determinism,
        event,
    );
    subscribers.skills.retain(|subscriber| {
        let Some(definition) = crate::engine::skill::condition::registry::find_key(
            subscriber.key.multiplicity_policy.opcode,
            subscriber.key.multiplicity_policy.type_name,
        ) else {
            return true;
        };
        let key = (
            subscriber.owner_uid,
            subscriber.skill_id,
            subscriber.slot_index,
            subscriber.key.definition,
        );
        match definition.reaction_multiplicity {
            crate::engine::skill::condition::registry::ReactionMultiplicity::PerEvent => true,
            crate::engine::skill::condition::registry::ReactionMultiplicity::OncePerEventBatch => {
                !multiplicity.once_per_event_batch.contains(&key)
            }
            crate::engine::skill::condition::registry::ReactionMultiplicity::OncePerAction => {
                !multiplicity.once_per_action.contains(&key)
            }
        }
    });
    subscribers.buff_acts.retain(|subscriber| {
        let BattleEvent::Hit(hit) = event else {
            return true;
        };
        let Some(definition) =
            buff_act::registry::find(subscriber.key.definition.opcode, &subscriber.act_type)
        else {
            return true;
        };
        if definition.runtime.event_multiplicity
            != buff_act::registry::RuntimeEventMultiplicity::OncePerActionTarget
        {
            return true;
        }
        multiplicity.once_per_target.insert((
            subscriber.owner_uid,
            subscriber.buff_uid,
            subscriber.key.definition,
            hit.source_uid,
            hit.target_uid,
        ))
    });
    output_passes(subscribers, managers, pool, catalog, determinism, event)
}

fn retain_observing_subscribers(
    subscribers: &mut subscriber::EventSubscribers,
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &crate::engine::runtime::determinism::RoundDeterminism,
    event: &BattleEvent,
) {
    subscribers.skills.retain(|subscriber| {
        crate::engine::skill::condition::registry::find_key(
            subscriber.key.definition.opcode,
            subscriber.key.definition.type_name,
        )
        .is_none_or(|definition| {
            definition.reaction_observer_scope
                != crate::engine::skill::condition::registry::ReactionObserverScope::EventTarget
                || event.target_uid() == Some(subscriber.owner_uid)
        })
    });
    if let Some(action) = event.skill_action_event() {
        subscribers.skills.retain(|subscriber| {
            skill_subscriber_observes_action(pool, subscriber, action)
                && (subscriber.key.event != EventKind::SkillAction
                    || subscriber.key.phase == Some(action.phase)
                    || (subscriber.key.phase.is_none()
                        && action.phase == crate::engine::skill::action::SkillPhase::AfterHit))
        });
        subscribers.buff_acts.retain(|subscriber| {
            let observes_actor = match buff_act::registry::runtime_actor_scope(
                subscriber.key.definition.opcode,
                &subscriber.act_type,
            ) {
                buff_act::registry::RuntimeActorScope::Owner => {
                    subscriber.owner_uid == action.source_uid
                }
                buff_act::registry::RuntimeActorScope::Team => {
                    pool.source_is_attacker(subscriber.owner_uid)
                        == pool.source_is_attacker(action.source_uid)
                }
                buff_act::registry::RuntimeActorScope::OpposingTeam => {
                    pool.source_is_attacker(subscriber.owner_uid)
                        != pool.source_is_attacker(action.source_uid)
                }
                buff_act::registry::RuntimeActorScope::OwnerOrTarget => {
                    subscriber.owner_uid == action.source_uid
                        || action.target_uids.contains(&subscriber.owner_uid)
                }
            };
            observes_actor
                && subscriber
                    .key
                    .phase
                    .is_none_or(|phase| phase == action.phase)
        });
    }
    if let BattleEvent::AllyAction(action) = event {
        let attacker = pool.source_is_attacker(action.source_uid);
        subscribers.skills.retain(|subscriber| {
            if subscriber.key.event == EventKind::SkillCast {
                subscriber.owner_uid == action.source_uid
            } else {
                skill_subscriber_observes_completed_action(pool, subscriber, action)
            }
        });
        let team = if attacker { 1 } else { 2 };
        subscribers.buff_acts.retain(|subscriber| {
            if subscriber.key.event == EventKind::SkillCast {
                subscriber.owner_uid == action.source_uid
            } else {
                match buff_act::registry::runtime_team_scope(
                    subscriber.key.definition.opcode,
                    &subscriber.act_type,
                ) {
                    buff_act::registry::RuntimeTeamScope::Same => subscriber.team_type == team,
                    buff_act::registry::RuntimeTeamScope::Opposing => subscriber.team_type != team,
                    buff_act::registry::RuntimeTeamScope::Any => true,
                }
            }
        });
    }
    if let BattleEvent::BuffFeatureTriggered(trigger) = event {
        subscribers
            .skills
            .retain(|subscriber| subscriber.owner_uid == trigger.owner_uid);
        subscribers
            .buff_acts
            .retain(|subscriber| subscriber.owner_uid == trigger.owner_uid);
    }
    if let BattleEvent::EntityTransformed { target_uid } = event {
        subscribers
            .skills
            .retain(|subscriber| subscriber.owner_uid == *target_uid);
        subscribers
            .buff_acts
            .retain(|subscriber| subscriber.owner_uid == *target_uid);
    }
    if let BattleEvent::EntityDied(death) = event {
        subscribers.skills.retain(|subscriber| {
            let Some(definition) = crate::engine::skill::condition::registry::find_key(
                subscriber.key.definition.opcode,
                subscriber.key.definition.type_name,
            ) else {
                return true;
            };
            match definition.reaction_observer_scope {
                crate::engine::skill::condition::registry::ReactionObserverScope::AnySubscriber => {
                    true
                }
                crate::engine::skill::condition::registry::ReactionObserverScope::EventTarget => {
                    subscriber.owner_uid == death.target_uid
                }
                crate::engine::skill::condition::registry::ReactionObserverScope::ConditionTarget => {
                    let slot_index = subscriber
                        .slot_index
                        .expect("compiled death subscriber has a condition slot");
                    let slot = catalog
                        .get(subscriber.skill_id)
                        .and_then(|effect| effect.slots.get(slot_index))
                        .expect("compiled death subscriber references an existing effect slot");
                    let mut observer_determinism = determinism.clone();
                    TargetResolver::resolve_with_managers_and_context(
                        &slot.condition_target,
                        subscriber.skill_id,
                        subscriber.owner_uid,
                        pool,
                        &mut observer_determinism,
                        Some(managers),
                        TargetContext {
                            runtime_target_uid: death.target_uid,
                            event_source_uid: death.source_uid,
                            ..Default::default()
                        },
                    )
                    .contains(&death.target_uid)
                }
            }
        });
        subscribers
            .buff_acts
            .retain(|subscriber| subscriber.owner_uid == death.target_uid
                || buff_act::subscriber_kind(subscriber)
                    == Some(buff_act::registry::BuffActKind::EnemyExitNumAddBuff));
    }
}

fn skill_subscriber_observes_action(
    pool: &TargetPool,
    subscriber: &SkillSubscriber,
    action: &crate::engine::skill::action::SkillActionEvent,
) -> bool {
    match crate::engine::skill::condition::registry::find_key(
        subscriber.key.definition.opcode,
        subscriber.key.definition.type_name,
    )
    .map(|definition| definition.skill_action_observer)
    .unwrap_or_default()
    {
        crate::engine::skill::condition::registry::SkillActionObserver::Actor => {
            subscriber.owner_uid == action.source_uid
        }
        crate::engine::skill::condition::registry::SkillActionObserver::AttackTarget => {
            action.is_attack && action.target_uids.contains(&subscriber.owner_uid)
        }
        crate::engine::skill::condition::registry::SkillActionObserver::Team => {
            pool.source_is_attacker(subscriber.owner_uid)
                == pool.source_is_attacker(action.source_uid)
        }
        crate::engine::skill::condition::registry::SkillActionObserver::OpposingTeam => {
            pool.source_is_attacker(subscriber.owner_uid)
                != pool.source_is_attacker(action.source_uid)
        }
        crate::engine::skill::condition::registry::SkillActionObserver::AllyOfAttackedTarget => {
            action.is_attack
                && action.attacked_target_uids.iter().any(|target_uid| {
                    pool.entity(*target_uid).is_some()
                        && pool.source_is_attacker(*target_uid)
                            == pool.source_is_attacker(subscriber.owner_uid)
                })
        }
    }
}

fn skill_subscriber_observes_completed_action(
    pool: &TargetPool,
    subscriber: &SkillSubscriber,
    action: &crate::engine::skill::action::ActionEvent,
) -> bool {
    let same_team =
        pool.source_is_attacker(subscriber.owner_uid) == pool.source_is_attacker(action.source_uid);
    match crate::engine::skill::condition::registry::find_key(
        subscriber.key.definition.opcode,
        subscriber.key.definition.type_name,
    )
    .map(|definition| definition.skill_action_observer)
    .unwrap_or_default()
    {
        crate::engine::skill::condition::registry::SkillActionObserver::OpposingTeam => !same_team,
        _ => same_team,
    }
}

pub fn dispatch_owner_event(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut crate::engine::runtime::determinism::RoundDeterminism,
    event: &BattleEvent,
    owner_uids: &[i64],
) -> Result<DispatchBatch, SubscriberError> {
    subscriber::for_compiled_owner_events(
        pool,
        managers,
        catalog,
        event.subscription_kinds(),
        owner_uids,
    )
    .map(|subscribers| {
        dispatch_subscribers(subscribers, pool, managers, catalog, determinism, event)
    })
}

pub fn dispatch_owner_event_phase(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut crate::engine::runtime::determinism::RoundDeterminism,
    event: &BattleEvent,
    owner_uids: &[i64],
    publication: crate::engine::event::subscription::PublicationPhase,
) -> Result<DispatchBatch, SubscriberError> {
    subscriber::for_compiled_owner_events(
        pool,
        managers,
        catalog,
        event.subscription_kinds(),
        owner_uids,
    )
    .map(|mut subscribers| {
        retain_publication(&mut subscribers, publication);
        dispatch_subscribers(subscribers, pool, managers, catalog, determinism, event)
    })
}

pub(crate) fn dispatch_owner_event_phase_with_multiplicity(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut crate::engine::runtime::determinism::RoundDeterminism,
    event: &BattleEvent,
    scope: DispatchOwnerPhase<'_>,
    multiplicity: &mut DispatchMultiplicity<'_>,
) -> Result<DispatchBatch, SubscriberError> {
    subscriber::for_compiled_owner_events(
        pool,
        managers,
        catalog,
        event.subscription_kinds(),
        scope.owner_uids,
    )
    .map(|mut subscribers| {
        retain_publication(&mut subscribers, scope.publication);
        dispatch_subscribers_with_multiplicity(
            subscribers,
            pool,
            managers,
            catalog,
            determinism,
            event,
            multiplicity,
        )
    })
}

fn retain_publication(
    subscribers: &mut subscriber::EventSubscribers,
    publication: crate::engine::event::subscription::PublicationPhase,
) {
    use crate::engine::event::subscription::{PublicationPhase, ReactionTiming};

    subscribers.skills.retain(|subscriber| match publication {
        PublicationPhase::BeforePublish => {
            subscriber.key.timing == ReactionTiming::Immediate
                && subscriber.key.publication == PublicationPhase::BeforePublish
        }
        PublicationPhase::AfterPublish => {
            subscriber.key.timing != ReactionTiming::Immediate
                || subscriber.key.publication == PublicationPhase::AfterPublish
        }
    });
    subscribers
        .buff_acts
        .retain(|subscriber| subscriber.key.publication == publication);
}

fn retain_reaction_lane(
    subscribers: &mut subscriber::EventSubscribers,
    lane: Option<ReactionLane>,
) {
    use crate::engine::event::subscription::ReactionTiming;

    match lane {
        Some(ReactionLane::Skills) => subscribers.buff_acts.clear(),
        Some(ReactionLane::BuffActs) => subscribers.skills.clear(),
        Some(ReactionLane::BuffActsBeforeSettlement) => {
            subscribers.skills.clear();
            subscribers
                .buff_acts
                .retain(|subscriber| subscriber.key.timing != ReactionTiming::AfterSettlement);
        }
        Some(ReactionLane::BuffActsAfterSettlement) => {
            subscribers.skills.clear();
            subscribers
                .buff_acts
                .retain(|subscriber| subscriber.key.timing == ReactionTiming::AfterSettlement);
        }
        None => {}
    }
}

fn retain_reaction_timing(
    subscribers: &mut subscriber::EventSubscribers,
    timing: crate::engine::event::subscription::ReactionTiming,
    event: &BattleEvent,
    delay_shared_hit: bool,
) {
    use crate::engine::event::subscription::ReactionTiming;

    let shared_hit =
        delay_shared_hit && matches!(event, BattleEvent::Hit(hit) if hit.share_count > 0);
    let selected = |declared| match declared {
        ReactionTiming::AfterSharedHit if shared_hit => timing == ReactionTiming::AfterSharedHit,
        ReactionTiming::AfterSharedHit => timing == ReactionTiming::Immediate,
        declared => declared == timing,
    };
    subscribers
        .skills
        .retain(|subscriber| selected(subscriber.key.timing));
    subscribers
        .buff_acts
        .retain(|subscriber| selected(subscriber.key.timing));
}

#[cfg(test)]
pub fn dispatch_setup(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    stage: SetupStage,
    priority: i32,
) -> Vec<(SetupSubscriber, RuleOp)> {
    setup_outputs(subscriber::for_setup_stage(
        pool, managers, catalog, stage, priority,
    ))
}

pub fn dispatch_compiled_setup(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    stage: SetupStage,
    priority: i32,
) -> Result<Vec<(SetupSubscriber, RuleOp)>, SubscriberError> {
    subscriber::for_compiled_setup_stage(pool, managers, catalog, stage, priority)
        .map(compiled_setup_outputs)
}

pub fn dispatch_buff_act_setup(
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    stage: SetupStage,
    priority: i32,
) -> Vec<(subscriber::BuffActSetupSubscriber, Option<Vec<RuleOp>>)> {
    subscriber::buff_acts_for_setup_stage(managers, stage, priority)
        .into_iter()
        .map(|subscriber| {
            let ops = buff_act::setup_rule_ops(managers, catalog, &subscriber);
            (subscriber, ops)
        })
        .collect()
}

fn compiled_setup_outputs(subscribers: Vec<SetupSubscriber>) -> Vec<(SetupSubscriber, RuleOp)> {
    subscribers
        .into_iter()
        .map(|subscriber| {
            let plan = SkillRequest {
                source_uid: subscriber.owner_uid,
                skill_id: subscriber.skill_id,
            };
            (
                subscriber,
                RuleOp::Skill(SkillInvocation {
                    plan,
                    condition_key: Some(subscriber.key),
                    condition_slot: Some(subscriber.slot_index),
                    ..plan.into()
                }),
            )
        })
        .collect()
}

#[cfg(test)]
fn setup_outputs(subscribers: Vec<SetupSubscriber>) -> Vec<(SetupSubscriber, RuleOp)> {
    subscribers
        .into_iter()
        .map(|subscriber| {
            let plan = SkillRequest {
                source_uid: subscriber.owner_uid,
                skill_id: subscriber.skill_id,
            };
            let output = RuleOp::Skill(SkillInvocation {
                plan,
                condition_key: Some(subscriber.key),
                condition_slot: Some(subscriber.slot_index),
                ..plan.into()
            });
            (subscriber, output)
        })
        .collect()
}

#[cfg(test)]
mod tests;
