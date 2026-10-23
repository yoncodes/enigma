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
            let (target_skills, observer_skills) = subscribers.skills.into_iter().partition(
                |subscriber| {
                    crate::engine::skill::condition::registry::find_key(
                        subscriber.key.definition.opcode,
                        subscriber.key.definition.type_name,
                    )
                    .is_some_and(|definition| {
                        definition.reaction_observer_scope
                            == crate::engine::skill::condition::registry::ReactionObserverScope::EventTarget
                    })
                },
            );
            vec![
                subscriber::EventSubscribers {
                    skills: target_skills,
                    buff_acts: subscribers.buff_acts,
                },
                subscriber::EventSubscribers {
                    skills: observer_skills,
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
    let mut subscribers = if let Some(owner_uids) = selection.owner_uids {
        subscriber::for_compiled_owner_events(
            pool,
            managers,
            catalog,
            event.subscription_kinds(),
            owner_uids,
        )?
    } else {
        subscriber::for_compiled_events(pool, managers, catalog, event.subscription_kinds())?
    };
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
            subscriber.key.definition.opcode,
            subscriber.key.definition.type_name,
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
            .retain(|subscriber| subscriber.owner_uid == death.target_uid);
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
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    use super::*;
    use crate::engine::{
        event::{bus::EventBus, payload::HitEvent},
        manager::buff::{BuffCommand, CommandOrigin},
        runtime::executor::execute_rule_op,
        skill::{
            condition::{ParsedCondition, ParsedConditionKind, parse::BuffAddedScope},
            effect::{ParsedBehavior, ParsedSkillEffect, SkillEffectSlot},
            rule::{DefinitionKey, RuleDomain, output::BattleCommand},
            target::TargetRequest,
        },
    };

    #[test]
    fn reaction_timing_selects_subscribers_before_rule_outputs() {
        let skill = |timing| SkillSubscriber {
            owner_uid: 10,
            skill_id: 100,
            slot_index: Some(0),
            key: crate::engine::event::subscription::SubscriptionKey::new(
                EventKind::SkillAction,
                DefinitionKey::new(1, "StageTest"),
            )
            .with_timing(timing),
        };
        let mut subscribers = subscriber::EventSubscribers {
            skills: vec![
                skill(crate::engine::event::subscription::ReactionTiming::Immediate),
                skill(crate::engine::event::subscription::ReactionTiming::AfterSkill),
                skill(crate::engine::event::subscription::ReactionTiming::AfterHitSkills),
            ],
            buff_acts: Vec::new(),
        };

        retain_reaction_timing(
            &mut subscribers,
            crate::engine::event::subscription::ReactionTiming::AfterSkill,
            &BattleEvent::Kind(EventKind::SkillAction),
            false,
        );

        assert_eq!(subscribers.skills.len(), 1);
        assert_eq!(
            subscribers.skills[0].key.timing,
            crate::engine::event::subscription::ReactionTiming::AfterSkill
        );
    }

    #[test]
    fn shared_hit_delay_is_owned_by_the_subscription_timing() {
        let subscriber = || SkillSubscriber {
            owner_uid: 10,
            skill_id: 100,
            slot_index: Some(0),
            key: crate::engine::event::subscription::SubscriptionKey::new(
                EventKind::TargetAttacked,
                DefinitionKey::new(22209, "BeAttacked"),
            )
            .with_timing(crate::engine::event::subscription::ReactionTiming::AfterSharedHit),
        };
        let hit = |share_count| {
            BattleEvent::Hit(HitEvent {
                origin: CommandOrigin {
                    domain: RuleDomain::Skill,
                    key: DefinitionKey::new(1, "SkillDamage"),
                },
                source_uid: -1,
                target_uid: 10,
                skill_id: 1,
                amount: 100,
                resolved_damage: 100,
                shield_absorbed: 0,
                career_restraint: false,
                damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
                share_count,
                assassinate: false,
                ignore_riposte: false,
            })
        };

        let mut normal = subscriber::EventSubscribers {
            skills: vec![subscriber()],
            buff_acts: Vec::new(),
        };
        retain_reaction_timing(
            &mut normal,
            crate::engine::event::subscription::ReactionTiming::Immediate,
            &hit(0),
            true,
        );
        assert_eq!(normal.skills.len(), 1);

        let mut shared = subscriber::EventSubscribers {
            skills: vec![subscriber()],
            buff_acts: Vec::new(),
        };
        retain_reaction_timing(
            &mut shared,
            crate::engine::event::subscription::ReactionTiming::AfterSharedHit,
            &hit(3),
            true,
        );
        assert_eq!(shared.skills.len(), 1);
    }

    #[test]
    fn buff_act_subscription_selects_declared_reaction_timing_before_rule_outputs() {
        let subscriber = |opcode, type_name: &'static str| BuffActSubscriber {
            owner_uid: 10,
            source_uid: 10,
            buff_uid: i64::from(opcode),
            buff_id: opcode,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: crate::engine::event::subscription::SubscriptionKey::new(
                EventKind::SkillAction,
                DefinitionKey::new(opcode, type_name),
            )
            .with_timing(crate::engine::skill::buff_act::registry::reaction_timing(
                opcode, type_name,
            )),
            act_type: type_name.to_owned(),
            effect_time: 0,
            effect_condition: 0,
            args: Vec::new(),
            raw: String::new(),
        };
        let buff_acts = vec![
            subscriber(800, "TeammateInjuryCount"),
            subscriber(1023, "LostHpAddExtraBloodPoolValue"),
            subscriber(302, "BeatBack"),
            subscriber(870, "Shell"),
            subscriber(871, "ShellDebuff"),
        ];
        for (timing, expected) in [
            (
                crate::engine::event::subscription::ReactionTiming::Immediate,
                DefinitionKey::new(800, "TeammateInjuryCount"),
            ),
            (
                crate::engine::event::subscription::ReactionTiming::AfterSettlement,
                DefinitionKey::new(1023, "LostHpAddExtraBloodPoolValue"),
            ),
            (
                crate::engine::event::subscription::ReactionTiming::AfterAction,
                DefinitionKey::new(302, "BeatBack"),
            ),
            (
                crate::engine::event::subscription::ReactionTiming::AfterHitObservers,
                DefinitionKey::new(870, "Shell"),
            ),
            (
                crate::engine::event::subscription::ReactionTiming::BeforeAfterHitObservers,
                DefinitionKey::new(871, "ShellDebuff"),
            ),
        ] {
            let mut subscribers = subscriber::EventSubscribers {
                skills: Vec::new(),
                buff_acts: buff_acts.clone(),
            };

            retain_reaction_timing(
                &mut subscribers,
                timing,
                &BattleEvent::Kind(EventKind::SkillAction),
                false,
            );

            assert_eq!(
                subscribers
                    .buff_acts
                    .iter()
                    .map(|subscriber| subscriber.key.definition)
                    .collect::<Vec<_>>(),
                vec![expected]
            );
        }
    }

    #[test]
    fn completed_action_routes_same_and_opposing_team_thresholds_separately() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    passive_skill: vec![30660191],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(100),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);
        managers.eureka.add_max(10, 1, 5);
        managers.eureka.set(10, 1, 5);
        let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
        let dispatch_action = |source_uid| {
            dispatch_event(
                &pool,
                &managers,
                &catalog,
                &mut crate::engine::runtime::determinism::RoundDeterminism::default(),
                &BattleEvent::AllyAction(crate::engine::skill::action::ActionEvent {
                    source_uid,
                    ..Default::default()
                }),
            )
            .unwrap()
        };

        let allied = dispatch_action(10);
        assert!(matches!(
            allied.skills.as_slice(),
            [(subscriber, _)]
                if subscriber.key.definition
                    == DefinitionKey::new(180212999, "PowerCompare")
        ));

        let opposing = dispatch_action(-1);
        assert!(matches!(
            opposing.skills.as_slice(),
            [(subscriber, _)]
                if subscriber.key.definition
                    == DefinitionKey::new(180213999, "PowerCompare")
        ));
    }

    #[test]
    fn derived_skill_cast_does_not_require_a_skill_action_phase() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                assist_boss: Some(FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(999_999),
                    passive_skill: vec![12_720_012],
                    ..Default::default()
                }),
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-2),
                    current_hp: Some(100),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let managers = BattleManagers::seeded(&fight);
        let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
        let event = BattleEvent::SkillAction(crate::engine::skill::action::SkillActionEvent {
            source_uid: -1,
            skill_id: 370_001_002,
            target_uid: -2,
            target_uids: vec![-2],
            attacked_target_uids: vec![-2],
            phase: crate::engine::skill::action::SkillPhase::HitPassives,
            skill_slot: -1,
            is_attack: true,
            rank: 1,
            skill_type: 0,
            effect_tag: 2,
            assassinate: false,
            ignore_riposte: false,
            damage_amount: 1,
            kill_count: 0,
            crit_count: 0,
            guard_break_count: 0,
            additional_moxie: 0,
            extra_skill_kind: 0,
            mode: crate::engine::skill::action::SkillExecutionMode::Active,
            teammate_injury_count: 0,
            teammate_injury_count_not_reset: 0,
            team_injury_count_round: 0,
            card_enchants: Vec::new(),
            buff_additions: Vec::new(),
        });

        let dispatched = dispatch_event(
            &pool,
            &managers,
            &catalog,
            &mut crate::engine::runtime::determinism::RoundDeterminism::default(),
            &event,
        )
        .unwrap();

        assert!(dispatched.skills.iter().any(|(subscriber, _)| {
            subscriber.owner_uid == -1 && subscriber.skill_id == 12_720_012
        }));
    }

    #[test]
    fn ally_attacked_observer_uses_primary_hit_targets_once_per_action() {
        crate::test_support::init_config();
        let entity = |uid| FightEntityInfo {
            uid: Some(uid),
            current_hp: Some(100),
            ..Default::default()
        };
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![entity(10), entity(11), entity(12)],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![entity(-1)],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let subscriber = |owner_uid| SkillSubscriber {
            owner_uid,
            skill_id: 1,
            slot_index: Some(0),
            key: crate::engine::event::subscription::SubscriptionKey::at_phase(
                EventKind::SkillAction,
                DefinitionKey::new(22213, "BeAttacked"),
                Some(crate::engine::skill::action::SkillPhase::HitPassives),
            ),
        };
        let mut action = crate::engine::skill::action::SkillActionEvent {
            source_uid: -1,
            skill_id: 2,
            target_uid: 10,
            target_uids: vec![10, 11],
            attacked_target_uids: vec![10, 11],
            phase: crate::engine::skill::action::SkillPhase::HitPassives,
            skill_slot: 1,
            is_attack: true,
            rank: 1,
            skill_type: 1,
            effect_tag: 1,
            assassinate: false,
            ignore_riposte: false,
            damage_amount: 1,
            kill_count: 0,
            crit_count: 0,
            guard_break_count: 0,
            additional_moxie: 0,
            extra_skill_kind: 0,
            mode: crate::engine::skill::action::SkillExecutionMode::Active,
            teammate_injury_count: 0,
            teammate_injury_count_not_reset: 0,
            team_injury_count_round: 0,
            card_enchants: Vec::new(),
            buff_additions: Vec::new(),
        };

        assert!(skill_subscriber_observes_action(
            &pool,
            &subscriber(12),
            &action
        ));
        assert!(skill_subscriber_observes_action(
            &pool,
            &subscriber(10),
            &action
        ));
        assert!(!skill_subscriber_observes_action(
            &pool,
            &subscriber(-1),
            &action
        ));

        action.attacked_target_uids = vec![10];
        assert!(skill_subscriber_observes_action(
            &pool,
            &subscriber(10),
            &action
        ));
    }

    #[test]
    fn team_skill_observer_sees_allied_nested_actions() {
        crate::test_support::init_config();
        let entity = |uid, team_type| FightEntityInfo {
            uid: Some(uid),
            team_type: Some(team_type),
            current_hp: Some(100),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&Fight {
            attacker: Some(FightTeam {
                entitys: vec![entity(10, 1), entity(11, 1)],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![entity(-1, 2)],
                ..Default::default()
            }),
            ..Default::default()
        });
        let subscriber = SkillSubscriber {
            owner_uid: 10,
            skill_id: 1,
            slot_index: Some(0),
            key: crate::engine::event::subscription::SubscriptionKey::at_phase(
                EventKind::SkillAction,
                DefinitionKey::new(1001212, "Assassinate"),
                Some(crate::engine::skill::action::SkillPhase::AfterHit),
            ),
        };
        let action = |source_uid| crate::engine::skill::action::SkillActionEvent {
            source_uid,
            skill_id: 2,
            target_uid: -1,
            target_uids: vec![-1],
            attacked_target_uids: vec![-1],
            phase: crate::engine::skill::action::SkillPhase::AfterHit,
            skill_slot: 1,
            is_attack: true,
            rank: 1,
            skill_type: 1,
            effect_tag: 1,
            assassinate: true,
            ignore_riposte: false,
            damage_amount: 1,
            kill_count: 0,
            crit_count: 0,
            guard_break_count: 0,
            additional_moxie: 0,
            extra_skill_kind: crate::engine::skill::condition::extra::ExtraSkillKind::Riposte.id(),
            mode: crate::engine::skill::action::SkillExecutionMode::Nested,
            teammate_injury_count: 0,
            teammate_injury_count_not_reset: 0,
            team_injury_count_round: 0,
            card_enchants: Vec::new(),
            buff_additions: Vec::new(),
        };

        assert!(skill_subscriber_observes_action(
            &pool,
            &subscriber,
            &action(11)
        ));
        assert!(!skill_subscriber_observes_action(
            &pool,
            &subscriber,
            &action(-1)
        ));
    }

    #[test]
    fn team_actor_buff_act_observes_an_allied_skill_action() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![
                    FightEntityInfo {
                        uid: Some(10),
                        current_hp: Some(100),
                        buffs: vec![BuffInfo {
                            uid: Some(20),
                            buff_id: Some(433011),
                            from_uid: Some(10),
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(11),
                        current_hp: Some(100),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
        let event = BattleEvent::SkillAction(crate::engine::skill::action::SkillActionEvent {
            source_uid: 11,
            skill_id: 100,
            target_uid: -1,
            target_uids: vec![-1],
            attacked_target_uids: vec![-1],
            phase: crate::engine::skill::action::SkillPhase::AfterHit,
            skill_slot: 1,
            is_attack: true,
            rank: 1,
            skill_type: 1,
            effect_tag: 1,
            assassinate: false,
            ignore_riposte: false,
            damage_amount: 1,
            kill_count: 0,
            crit_count: 0,
            guard_break_count: 0,
            additional_moxie: 0,
            extra_skill_kind: 0,
            mode: crate::engine::skill::action::SkillExecutionMode::Active,
            teammate_injury_count: 1,
            teammate_injury_count_not_reset: 1,
            team_injury_count_round: 1,
            card_enchants: Vec::new(),
            buff_additions: Vec::new(),
        });

        let dispatched = dispatch_event(
            &TargetPool::from_fight(&fight),
            &managers,
            &catalog,
            &mut crate::engine::runtime::determinism::RoundDeterminism::default(),
            &event,
        )
        .unwrap();

        assert!(dispatched.buff_acts.iter().any(|(subscriber, ops)| {
            subscriber.key.definition == DefinitionKey::new(800, "TeammateInjuryCount")
                && matches!(
                    ops.as_deref(),
                    Some([buff_act::BuffActRuleOp {
                        op: RuleOp::Command(BattleCommand::Buff(BuffCommand::AccumulateActValue(
                            _
                        ))),
                        ..
                    }])
                )
        }));
    }

    #[test]
    fn committed_buff_addition_selects_its_registered_subscriber() {
        crate::test_support::init_config();
        let mut slot = SkillEffectSlot::new(
            ParsedBehavior::new(20002, "AddExPoint", vec![1]),
            TargetRequest::self_only(),
        );
        slot.conditions = vec![ParsedCondition {
            opcode: 583004,
            type_name: "AccTeamAddBuffCountByBuffId".to_owned(),
            kind: ParsedConditionKind::AccBuffAddedCount {
                buff_ids: vec![101],
                threshold: 1,
                scope: BuffAddedScope::Team,
            },
            raw_args: vec!["101".to_owned(), "1".to_owned()],
        }];
        slot.compiled_route =
            crate::engine::skill::rule::route::ConditionRoute::compile(&slot.conditions);
        let mut catalog = SkillEffectCatalog::default();
        catalog.insert(ParsedSkillEffect {
            skill_id: 100,
            slots: vec![slot],
        });
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1),
                    passive_skill: vec![100],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);
        let mut events = EventBus::default();
        execute_rule_op(
            &mut managers,
            &mut events,
            crate::engine::skill::rule::output::RuleOp::Command(BattleCommand::Buff(
                BuffCommand::Grant(crate::engine::manager::buff::BuffGrant {
                    origin: CommandOrigin {
                        domain: RuleDomain::Behavior,
                        key: DefinitionKey::new(1, "AddBuff"),
                    },
                    source_uid: 10,
                    target_uid: 10,
                    buff_id: 101,
                    amount: None,
                    occurrences: 1,
                    child_uid_reservations: 0,
                }),
            )),
        )
        .unwrap();
        let event = events.pop().expect("committed buff event");

        let dispatched = dispatch_event(
            &pool,
            &managers,
            &catalog,
            &mut crate::engine::runtime::determinism::RoundDeterminism::default(),
            &event,
        )
        .unwrap();

        assert_eq!(event.kind(), EventKind::BuffAdded);
        assert!(matches!(
            dispatched.skills.as_slice(),
            [(subscriber, RuleOp::Skill(invocation))] if subscriber.skill_id == 100
                && subscriber.key.event == EventKind::BuffAdded
                && invocation.condition_key
                    == Some(DefinitionKey::new(583004, "AccTeamAddBuffCountByBuffId"))
        ));
    }

    #[test]
    fn exact_linked_buff_act_emits_a_skill_output() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(30880131),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let catalog = SkillEffectCatalog::from_game_db(config::configs::get());

        let dispatched = dispatch(
            &TargetPool::from_fight(&fight),
            &managers,
            &catalog,
            EventKind::RoundEnd,
        );

        assert!(matches!(
            dispatched.buff_acts.as_slice(),
            [(
                subscriber,
                Some(ops)
            )] if subscriber.key.definition == DefinitionKey::new(759, "UseSkillToEnemy")
                && matches!(ops.as_slice(), [buff_act::BuffActRuleOp {
                    op: RuleOp::Skill(
                    crate::engine::skill::action::SkillInvocation {
                        plan: crate::engine::skill::action::SkillRequest {
                            source_uid: 10,
                            skill_id: 30880171,
                        },
                        ..
                    }
                ),
                    ..
                }])
        ));
    }

    #[test]
    fn setup_stage_is_not_encoded_as_a_runtime_event() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1),
                    passive_skill: vec![31340141],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let managers = BattleManagers::seeded(&fight);
        let catalog = SkillEffectCatalog::from_game_db(config::configs::get());

        let dispatched = dispatch_setup(&pool, &managers, &catalog, SetupStage::RoundStart, 1);

        assert!(matches!(
            dispatched.as_slice(),
            [(
                SetupSubscriber {
                    owner_uid: 10,
                    skill_id: 31340141,
                    slot_index: 1,
                    stage: SetupStage::RoundStart,
                    priority: 1,
                    key: setup_key,
                },
                RuleOp::Skill(SkillInvocation {
                    condition_key: Some(condition_key),
                    ..
                }),
            )] if *setup_key == DefinitionKey::new(103, "None")
                && *condition_key == DefinitionKey::new(103, "None")
        ));
    }

    #[test]
    fn compiled_setup_keeps_exact_routes_independent() {
        let subscribers = vec![
            SetupSubscriber {
                owner_uid: 10,
                skill_id: 100,
                slot_index: 0,
                stage: SetupStage::EnterFight,
                priority: 0,
                key: DefinitionKey::new(5, "EnterFight"),
            },
            SetupSubscriber {
                owner_uid: 10,
                skill_id: 100,
                slot_index: 1,
                stage: SetupStage::EnterFight,
                priority: 0,
                key: DefinitionKey::new(573002, "PerTeamOtherEntityDmgType"),
            },
        ];

        let outputs = compiled_setup_outputs(subscribers);

        assert!(matches!(
            outputs.as_slice(),
            [
                (
                    SetupSubscriber { key: first_key, .. },
                    RuleOp::Skill(SkillInvocation {
                        condition_key: Some(first_condition),
                        ..
                    })
                ),
                (
                    SetupSubscriber { key: second_key, .. },
                    RuleOp::Skill(SkillInvocation {
                        condition_key: Some(second_condition),
                        ..
                    })
                )
            ] if *first_key == DefinitionKey::new(5, "EnterFight")
                && *first_condition == *first_key
                && *second_key == DefinitionKey::new(573002, "PerTeamOtherEntityDmgType")
                && *second_condition == *second_key
        ));
    }

    #[test]
    fn committed_hit_discovers_target_attacked_skill_and_be_attacked_buff_act() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(100),
                    passive_skill: vec![530000151],
                    buffs: vec![
                        BuffInfo {
                            uid: Some(20),
                            buff_id: Some(530000111),
                            layer: Some(1),
                            ..Default::default()
                        },
                        BuffInfo {
                            uid: Some(21),
                            buff_id: Some(30620111),
                            layer: Some(1),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
        let event = BattleEvent::Hit(HitEvent {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "Damage"),
            },
            source_uid: 10,
            target_uid: -1,
            skill_id: 100,
            amount: 50,
            resolved_damage: 50,
            shield_absorbed: 0,
            career_restraint: false,
            damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
            share_count: 0,
            assassinate: false,
            ignore_riposte: false,
        });

        let dispatched = dispatch_event(
            &TargetPool::from_fight(&fight),
            &managers,
            &catalog,
            &mut crate::engine::runtime::determinism::RoundDeterminism::default(),
            &event,
        )
        .unwrap();

        assert!(
            dispatched
                .skills
                .iter()
                .any(|(subscriber, _)| subscriber.owner_uid == -1
                    && subscriber.skill_id == 530000151
                    && subscriber.key.event == EventKind::TargetAttacked)
        );
        assert!(
            dispatched
                .buff_acts
                .iter()
                .any(|(subscriber, ops)| subscriber.owner_uid == -1
                    && subscriber.key.event == EventKind::BeAttacked
                    && subscriber.key.definition == DefinitionKey::new(926, "ExPointAddByHit")
                    && ops.as_ref().is_some_and(|ops| matches!(
                        ops.as_slice(),
                        [
                            buff_act::BuffActRuleOp {
                                op: RuleOp::Command(
                                    crate::engine::skill::rule::output::BattleCommand::Buff(_)
                                ),
                                ..
                            },
                            buff_act::BuffActRuleOp {
                                op: RuleOp::BuffFeatureMarker {
                                    buff_act_id: 926,
                                    ..
                                },
                                ..
                            },
                            buff_act::BuffActRuleOp {
                                op: RuleOp::Command(
                                    crate::engine::skill::rule::output::BattleCommand::ExPoint(_)
                                ),
                                ..
                            }
                        ]
                    )))
        );
    }
}
