use crate::engine::{
    event::kind::EventKind,
    skill::{
        action::SkillPhase,
        behavior::classify::{BehaviorKey, BehaviorSpec},
        condition::{
            ParsedCondition, ParsedConditionKind,
            registry::{self, ConditionRole},
        },
        effect::ParsedBehavior,
        rule::{DefinitionKey, RuleDescriptor, RuleDomain, SetupStage},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConditionTrigger {
    pub key: crate::engine::skill::rule::DefinitionKey,
    pub event: EventKind,
    pub phase: Option<SkillPhase>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConditionSetup {
    pub key: crate::engine::skill::rule::DefinitionKey,
    pub stage: SetupStage,
    pub priority: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionDriver {
    Trigger(ConditionTrigger),
    Setup(ConditionSetup),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionBranchRoute {
    pub driver: Option<ConditionDriver>,
    pub conditions: Vec<RuleDescriptor>,
}

impl ConditionBranchRoute {
    pub fn driver_key(&self) -> Option<crate::engine::skill::rule::DefinitionKey> {
        match self.driver? {
            ConditionDriver::Trigger(trigger) => Some(trigger.key),
            ConditionDriver::Setup(setup) => Some(setup.key),
        }
    }

    pub fn subscriptions(&self) -> Vec<ConditionTrigger> {
        match self.driver {
            Some(ConditionDriver::Trigger(trigger)) => vec![trigger],
            Some(ConditionDriver::Setup(_)) => Vec::new(),
            None => self
                .conditions
                .iter()
                .find_map(|descriptor| {
                    let definition =
                        registry::find_key(descriptor.key.opcode, descriptor.key.type_name)?;
                    (!definition.dependencies.is_empty()).then(|| {
                        definition
                            .dependencies
                            .iter()
                            .copied()
                            .map(|event| ConditionTrigger {
                                key: definition.key,
                                event,
                                phase: definition.dependency_phase,
                            })
                            .collect()
                    })
                })
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionRoute {
    pub branches: Vec<ConditionBranchRoute>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteError {
    UnregisteredExactKey {
        opcode: i32,
        type_name: String,
    },
    ConflictingConditionDrivers {
        first: ConditionDriver,
        second: ConditionDriver,
    },
    StandaloneConditionRequired {
        condition: DefinitionKey,
        actual: BehaviorKey,
        required: DefinitionKey,
    },
}

impl ConditionRoute {
    pub fn compile(conditions: &[ParsedCondition]) -> Result<Self, RouteError> {
        Self::compile_with_driver(conditions, None, false)
    }

    pub fn compile_for_behavior(
        conditions: &[ParsedCondition],
        behavior: &BehaviorSpec,
    ) -> Result<Self, RouteError> {
        Self::compile_slot(conditions, behavior, None)
    }

    pub(crate) fn compile_configured(
        db: &config::GameDB,
        conditions: &[ParsedCondition],
        behavior: &ParsedBehavior,
    ) -> Result<Self, RouteError> {
        let setup = crate::engine::skill::buff_act::riposte::grant_setup(db, behavior, conditions);
        Self::compile_slot(conditions, &behavior.spec, setup)
    }

    fn compile_slot(
        conditions: &[ParsedCondition],
        behavior: &BehaviorSpec,
        configured_setup: Option<ConditionSetup>,
    ) -> Result<Self, RouteError> {
        use crate::engine::skill::behavior::registry::ConditionRouteOverride;

        validate_behavior(conditions, behavior, conditions.len() == 1)?;
        let definition = crate::engine::skill::behavior::registry::find_key(
            behavior.key.opcode,
            &behavior.key.type_name,
        );
        let driver = definition
            .and_then(|definition| definition.condition_route_override)
            .and_then(|route| match route {
                ConditionRouteOverride::ActiveCompletion(_)
                | ConditionRouteOverride::OwnerFrame(_)
                | ConditionRouteOverride::ActiveBuffPresence { .. } => None,
                ConditionRouteOverride::Trigger { key, event, phase } => {
                    Some(ConditionDriver::Trigger(ConditionTrigger {
                        key,
                        event,
                        phase,
                    }))
                }
                ConditionRouteOverride::Setup {
                    key,
                    stage,
                    priority,
                } => Some(ConditionDriver::Setup(ConditionSetup {
                    key,
                    stage,
                    priority,
                })),
            })
            .or_else(|| configured_setup.map(ConditionDriver::Setup));
        let predicate_only = driver.is_none()
            && definition.is_some_and(|definition| definition.attack_modifier_only)
            && registry::attack_modifier_side(conditions).is_some();
        Self::compile_with_driver(conditions, driver, predicate_only)
    }

    fn compile_with_driver(
        conditions: &[ParsedCondition],
        driver: Option<ConditionDriver>,
        predicate_only: bool,
    ) -> Result<Self, RouteError> {
        let branches = if let [
            ParsedCondition {
                kind: ParsedConditionKind::Any(groups),
                ..
            },
        ] = conditions
        {
            groups
                .iter()
                .map(|group| compile_branch(group, driver, predicate_only))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            vec![compile_branch(conditions, driver, predicate_only)?]
        };

        Ok(Self { branches })
    }
}

fn validate_behavior(
    conditions: &[ParsedCondition],
    behavior: &BehaviorSpec,
    standalone: bool,
) -> Result<(), RouteError> {
    for condition in conditions {
        if let ParsedConditionKind::Any(groups) = &condition.kind {
            for group in groups {
                validate_behavior(group, behavior, false)?;
            }
        } else if let Some(definition) = registry::find_key(condition.opcode, &condition.type_name)
            && let Some(required) = definition.standalone_behavior
            && (!standalone
                || matches!(condition.kind, ParsedConditionKind::Not(_))
                || behavior.key.opcode != required.opcode
                || behavior.key.type_name != required.type_name)
        {
            return Err(RouteError::StandaloneConditionRequired {
                condition: definition.key,
                actual: behavior.key.clone(),
                required,
            });
        }
    }
    Ok(())
}

fn compile_branch(
    conditions: &[ParsedCondition],
    contextual_driver: Option<ConditionDriver>,
    predicate_only: bool,
) -> Result<ConditionBranchRoute, RouteError> {
    let mut driver = None;
    let mut descriptors = Vec::with_capacity(conditions.len());

    for condition in conditions {
        let definition =
            registry::find_key(condition.opcode, &condition.type_name).ok_or_else(|| {
                RouteError::UnregisteredExactKey {
                    opcode: condition.opcode,
                    type_name: condition.type_name.clone(),
                }
            })?;
        descriptors.push(RuleDescriptor::new(RuleDomain::Condition, definition.key));

        if predicate_only {
            continue;
        }

        let contextual = contextual_driver.is_some_and(|driver| match driver {
            ConditionDriver::Trigger(trigger) => trigger.key == definition.key,
            ConditionDriver::Setup(setup) => setup.key == definition.key,
        });
        let candidate = match (contextual, contextual_driver) {
            (true, Some(driver)) => driver,
            _ => match definition.role {
                ConditionRole::Predicate => continue,
                ConditionRole::Trigger { event, phase } => {
                    ConditionDriver::Trigger(ConditionTrigger {
                        key: definition.key,
                        event,
                        phase,
                    })
                }
                ConditionRole::Setup { stage, priority } => {
                    ConditionDriver::Setup(ConditionSetup {
                        key: definition.key,
                        stage,
                        priority,
                    })
                }
            },
        };
        if let Some(first) = driver {
            driver = Some(merge_driver(first, candidate)?);
            continue;
        }
        driver = Some(candidate);
    }

    Ok(ConditionBranchRoute {
        driver,
        conditions: descriptors,
    })
}

fn merge_driver(
    first: ConditionDriver,
    second: ConditionDriver,
) -> Result<ConditionDriver, RouteError> {
    match (first, second) {
        (ConditionDriver::Trigger(first), ConditionDriver::Trigger(second)) => {
            if first.event != second.event {
                // Both events carry the same action context; the pre-effect event owns
                // a combined route when they describe the same explicit phase.
                match (first.event, second.event) {
                    (EventKind::ConduitActivated, EventKind::SkillAction)
                        if second.phase == Some(SkillPhase::AfterDamage) =>
                    {
                        Some(ConditionDriver::Trigger(first))
                    }
                    (EventKind::SkillAction, EventKind::ConduitActivated)
                        if first.phase == Some(SkillPhase::AfterDamage) =>
                    {
                        Some(ConditionDriver::Trigger(second))
                    }
                    (EventKind::SkillEffectStarted, EventKind::SkillAction)
                        if first.phase.is_some() && first.phase == second.phase =>
                    {
                        Some(ConditionDriver::Trigger(first))
                    }
                    (EventKind::SkillAction, EventKind::SkillEffectStarted)
                        if first.phase.is_some() && first.phase == second.phase =>
                    {
                        Some(ConditionDriver::Trigger(second))
                    }
                    _ => None,
                }
            } else {
                match (first.phase, second.phase) {
                    (Some(first_phase), Some(second_phase)) if first_phase != second_phase => None,
                    (None, Some(_)) => Some(ConditionDriver::Trigger(second)),
                    _ => Some(ConditionDriver::Trigger(first)),
                }
            }
        }
        (ConditionDriver::Setup(first), ConditionDriver::Setup(second)) => {
            (first.stage == second.stage).then_some(ConditionDriver::Setup(
                if second.priority > first.priority {
                    second
                } else {
                    first
                },
            ))
        }
        _ => None,
    }
    .ok_or(RouteError::ConflictingConditionDrivers { first, second })
}

#[cfg(test)]
mod tests;
