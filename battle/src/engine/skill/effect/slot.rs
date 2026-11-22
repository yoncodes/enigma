use crate::engine::{
    event::subscription::{PublicationPhase, SubscriptionKey},
    skill::{
        action::SkillPhase,
        behavior::classify::BehaviorSpec,
        condition::{ParsedCondition, registry},
        rule::{
            SetupStage,
            route::{ConditionDriver, ConditionRoute, RouteError},
        },
        target::TargetRequest,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSkillEffect {
    pub skill_id: i32,
    pub slots: Vec<SkillEffectSlot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct RuleLimitOwner(usize);

impl RuleLimitOwner {
    pub const fn from_slot(slot_index: usize) -> Self {
        Self(slot_index)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillEffectSlot {
    pub behavior: ParsedBehavior,
    pub conditions: Vec<ParsedCondition>,
    pub compiled_route: Result<ConditionRoute, RouteError>,
    pub condition_target: TargetRequest,
    pub target: TargetRequest,
    pub target_from_condition: bool,
    pub(crate) delegated_reward_phase: Option<SkillPhase>,
    pub(crate) ordered_phase: Option<SkillPhase>,
    pub limit: i32,
    pub round_limit: i32,
}

impl SkillEffectSlot {
    pub fn new(behavior: ParsedBehavior, target: TargetRequest) -> Self {
        Self {
            behavior,
            conditions: Vec::new(),
            compiled_route: ConditionRoute::compile(&[]),
            condition_target: TargetRequest::self_only(),
            target,
            target_from_condition: false,
            delegated_reward_phase: None,
            ordered_phase: None,
            limit: 0,
            round_limit: 0,
        }
    }

    pub fn subscriptions(&self) -> Vec<SubscriptionKey> {
        if crate::engine::skill::behavior::registry::find(&self.behavior).is_some_and(
            |definition| {
                definition
                    .attack_context
                    .is_some_and(|matches| matches(&self.conditions))
            },
        ) {
            return Vec::new();
        }
        self.conditions
            .iter()
            .map(ParsedCondition::subscriptions)
            .find(|subscriptions| !subscriptions.is_empty())
            .unwrap_or_default()
    }

    pub fn compiled_subscriptions(&self) -> Result<Vec<SubscriptionKey>, RouteError> {
        let Some(route) = self.usable_route()? else {
            return Ok(Vec::new());
        };
        if crate::engine::skill::behavior::registry::find(&self.behavior).is_some_and(
            |definition| {
                definition
                    .attack_context
                    .is_some_and(|matches| matches(&self.conditions))
                    || definition.round_modifier_only
                    || definition.card_play_role
                        == crate::engine::skill::behavior::registry::CardPlayRole::QueuePreparation
            },
        ) {
            return Ok(Vec::new());
        }
        let mut subscriptions = route
            .branches
            .iter()
            .flat_map(|branch| {
                let predicate_definitions = branch
                    .conditions
                    .iter()
                    .filter_map(|condition| {
                        registry::find_key(condition.key.opcode, condition.key.type_name)
                    })
                    .filter(|definition| definition.role == registry::ConditionRole::Predicate)
                    .collect::<Vec<_>>();
                branch.subscriptions().into_iter().map(move |trigger| {
                    let driver = registry::find_key(trigger.key.opcode, trigger.key.type_name);
                    let timing_policy = driver
                        .filter(|definition| {
                            definition.reaction_timing
                                == registry::ReactionTimingPolicy::Fixed(
                                    crate::engine::event::subscription::ReactionTiming::AttackedSkills,
                                )
                        })
                        .or_else(|| {
                            predicate_definitions.iter().copied().find(|definition| {
                                definition.reaction_timing
                                    != registry::ReactionTimingPolicy::Fixed(Default::default())
                            })
                        })
                        .or(driver);
                    let multiplicity_policy = predicate_definitions
                        .iter()
                        .copied()
                        .find(|definition| {
                            definition.reaction_multiplicity
                                != registry::ReactionMultiplicity::PerEvent
                        })
                        .or(driver);
                    SubscriptionKey::at_phase(trigger.event, trigger.key, trigger.phase)
                        .with_timing_policy(
                            timing_policy
                                .map(|definition| definition.key)
                                .unwrap_or(trigger.key),
                        )
                        .with_multiplicity_policy(
                            multiplicity_policy
                                .map(|definition| definition.key)
                                .unwrap_or(trigger.key),
                        )
                        .with_publication(
                            driver
                                .map(|definition| definition.publication)
                                .unwrap_or(PublicationPhase::AfterPublish),
                        )
                        .with_timing(
                            timing_policy
                                .map(|definition| {
                                    definition.reaction_timing.resolve(Default::default())
                                })
                                .unwrap_or_default(),
                        )
                })
            })
            .collect::<Vec<_>>();
        let entry = route
            .branches
            .iter()
            .filter_map(|branch| branch.driver_key())
            .filter_map(|key| registry::find_key(key.opcode, key.type_name))
            .flat_map(|definition| {
                definition
                    .reactivation_events
                    .iter()
                    .copied()
                    .filter(|event| event.is_entry())
                    .map(move |event| (definition, event))
            });
        let other = self
            .conditions
            .iter()
            .filter_map(|condition| registry::find_key(condition.opcode, &condition.type_name))
            .flat_map(|definition| {
                definition
                    .reactivation_events
                    .iter()
                    .copied()
                    .filter(|event| !event.is_entry())
                    .map(move |event| (definition, event))
            });
        for (definition, event) in entry.chain(other) {
            let key = SubscriptionKey::new(event, definition.key)
                .with_publication(definition.publication)
                .with_timing(definition.reaction_timing.resolve(Default::default()));
            if !subscriptions.contains(&key) {
                subscriptions.push(key);
            }
        }
        Ok(subscriptions)
    }

    pub(crate) fn entry_conditions(
        &self,
        subscriptions: &[SubscriptionKey],
        event: &crate::engine::event::payload::BattleEvent,
    ) -> Option<Vec<ParsedCondition>> {
        use crate::engine::skill::condition::{ParsedConditionKind, satisfied_conditions};

        let [condition] = self.conditions.as_slice() else {
            return None;
        };
        let ParsedConditionKind::Any(groups) = &condition.kind else {
            return None;
        };
        let route = self.compiled_route.as_ref().ok()?;
        let alternatives = groups
            .iter()
            .zip(&route.branches)
            .filter_map(|(conditions, branch)| {
                let driver = branch.driver_key()?;
                subscriptions
                    .iter()
                    .any(|key| {
                        key.definition == driver
                            && key.event.is_entry()
                            && event.subscription_kinds().any(|kind| kind == key.event)
                    })
                    .then(|| satisfied_conditions(conditions, driver))
            })
            .collect();
        Some(vec![ParsedCondition {
            kind: ParsedConditionKind::Any(alternatives),
            ..condition.clone()
        }])
    }

    pub fn active_phases(&self) -> Result<Vec<SkillPhase>, RouteError> {
        let fallback = self
            .delegated_reward_phase
            .or_else(|| self.active_presence_phase());
        let mut phases = Vec::new();
        for subscription in self.compiled_subscriptions()? {
            let phase = match subscription.event {
                crate::engine::event::kind::EventKind::SkillAction => {
                    subscription.phase.or(fallback)
                }
                crate::engine::event::kind::EventKind::SkillEffectStarted
                | crate::engine::event::kind::EventKind::SkillEffectsPrepared => subscription
                    .phase
                    .filter(|phase| *phase == SkillPhase::Immediate),
                _ => None,
            };
            if let Some(phase) = phase
                && !phases.contains(&phase)
            {
                phases.push(phase);
            }
        }
        if phases.is_empty()
            && let Some(phase) = self.ordered_phase
        {
            phases.push(phase);
        }
        Ok(phases)
    }

    fn active_presence_phase(&self) -> Option<SkillPhase> {
        use crate::engine::skill::{
            behavior::registry::{ConditionRouteOverride, find},
            condition::{ParsedConditionKind, buff::BuffConditionMode},
        };

        let ConditionRouteOverride::ActiveBuffPresence { key, phase } =
            find(&self.behavior)?.condition_route_override?
        else {
            return None;
        };
        let [condition] = self.conditions.as_slice() else {
            return None;
        };
        (key.matches(condition.opcode, &condition.type_name)
            && matches!(
                condition.kind,
                ParsedConditionKind::BuffId {
                    mode: BuffConditionMode::Present,
                    ..
                }
            ))
        .then_some(phase)
    }

    pub(crate) fn completion(&self) -> Result<Option<SubscriptionKey>, RouteError> {
        use crate::engine::skill::behavior::registry::{ConditionRouteOverride, find};

        let Some(ConditionRouteOverride::ActiveCompletion(trigger)) =
            find(&self.behavior).and_then(|definition| definition.condition_route_override)
        else {
            return Ok(None);
        };
        let [condition] = self.conditions.as_slice() else {
            return Ok(None);
        };
        if !matches!(
            condition.kind,
            crate::engine::skill::condition::ParsedConditionKind::None(_)
        ) || !condition.raw_args.is_empty()
            || condition.opcode != trigger.opcode
            || condition.type_name != trigger.type_name
        {
            return Ok(None);
        }
        let subscriptions = self.compiled_subscriptions()?;
        let [key] = subscriptions.as_slice() else {
            return Ok(None);
        };
        Ok((key.definition == trigger).then_some(*key))
    }

    pub fn setup_keys(
        &self,
        stage: SetupStage,
        priority: i32,
    ) -> Vec<crate::engine::skill::rule::DefinitionKey> {
        self.compiled_setup_keys(stage, priority)
            .unwrap_or_default()
    }

    pub fn compiled_setup_keys(
        &self,
        stage: SetupStage,
        priority: i32,
    ) -> Result<Vec<crate::engine::skill::rule::DefinitionKey>, RouteError> {
        let Some(route) = self.usable_route()? else {
            return Ok(Vec::new());
        };
        if crate::engine::skill::behavior::registry::find(&self.behavior).is_some_and(
            |definition| {
                definition
                    .attack_context
                    .is_some_and(|matches| matches(&self.conditions))
                    || definition.round_modifier_only
            },
        ) {
            return Ok(Vec::new());
        }
        let mut keys = route
            .branches
            .iter()
            .filter_map(|branch| match branch.driver {
                Some(ConditionDriver::Setup(setup))
                    if setup.stage == stage && setup.priority == priority =>
                {
                    Some(setup.key)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let needs_companion_setup = route.branches.iter().any(|branch| branch.driver.is_none());
        keys.extend(self.conditions.iter().filter_map(|condition| {
            let definition = registry::find_key(condition.opcode, &condition.type_name)?;
            ((definition.role != registry::ConditionRole::Predicate || needs_companion_setup)
                && definition.companion_setup.contains(&(stage, priority)))
            .then_some(definition.key)
        }));
        keys.dedup();
        Ok(keys)
    }

    fn usable_route(&self) -> Result<Option<&ConditionRoute>, RouteError> {
        match &self.compiled_route {
            Ok(route) => Ok(Some(route)),
            Err(RouteError::UnregisteredExactKey { .. }) => Ok(None),
            Err(error) => Err(error.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedBehavior {
    pub spec: BehaviorSpec,
    pub args: Vec<i32>,
    pub raw_args: Vec<String>,
    pub raw_arity: usize,
    pub config_effect: i32,
    pub effect_id: i32,
    pub slot: u8,
}

impl ParsedBehavior {
    #[cfg(test)]
    pub fn new(opcode: i32, type_name: &str, args: Vec<i32>) -> Self {
        Self {
            spec: BehaviorSpec::new(opcode, type_name),
            raw_arity: args.len(),
            args,
            raw_args: Vec::new(),
            config_effect: 0,
            effect_id: 0,
            slot: 0,
        }
    }

    pub fn from_spec(spec: BehaviorSpec, args: Vec<i32>, raw_args: Vec<String>) -> Self {
        Self {
            config_effect: spec.key.opcode,
            spec,
            args,
            raw_arity: raw_args.len(),
            raw_args,
            effect_id: 0,
            slot: 0,
        }
    }

    pub fn arg(&self, index: usize) -> Option<i32> {
        if let Some(raw) = self.raw_args.get(index) {
            return raw.parse().ok();
        }

        self.args.get(index).copied()
    }

    pub fn arg_list(&self, index: usize) -> Option<Vec<i32>> {
        if let Some(raw) = self.raw_args.get(index) {
            return parse_i32_list(raw);
        }

        self.args.get(index).copied().map(|value| vec![value])
    }
}

fn parse_i32_list(raw: &str) -> Option<Vec<i32>> {
    raw.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::parse)
        .collect::<Result<Vec<_>, _>>()
        .ok()
        .filter(|values| !values.is_empty())
}

#[cfg(test)]
mod tests;
