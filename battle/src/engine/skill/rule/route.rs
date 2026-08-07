use crate::engine::{
    event::kind::EventKind,
    skill::{
        action::SkillPhase,
        condition::{
            ParsedCondition, ParsedConditionKind,
            registry::{self, ConditionRole},
        },
        rule::{RuleDescriptor, RuleDomain, SetupStage},
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
                                phase: None,
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
}

impl ConditionRoute {
    pub fn compile(conditions: &[ParsedCondition]) -> Result<Self, RouteError> {
        Self::compile_with_driver(conditions, None, false)
    }

    pub fn compile_for_behavior(
        conditions: &[ParsedCondition],
        behavior: &crate::engine::skill::behavior::classify::BehaviorSpec,
    ) -> Result<Self, RouteError> {
        use crate::engine::skill::behavior::registry::ConditionRouteOverride;

        let definition = crate::engine::skill::behavior::registry::find_key(
            behavior.key.opcode,
            &behavior.key.type_name,
        );
        let driver = definition
            .and_then(|definition| definition.condition_route_override)
            .map(|route| match route {
                ConditionRouteOverride::Trigger { key, event, phase } => {
                    ConditionDriver::Trigger(ConditionTrigger { key, event, phase })
                }
                ConditionRouteOverride::Setup {
                    key,
                    stage,
                    priority,
                } => ConditionDriver::Setup(ConditionSetup {
                    key,
                    stage,
                    priority,
                }),
            });
        let predicate_only = driver.is_none()
            && definition.is_some_and(|definition| definition.collect_attack_modifier.is_some())
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
mod tests {
    use super::*;
    use crate::engine::skill::condition::parse_conditions;
    use crate::test_support::init_config;

    #[test]
    fn predicate_order_does_not_change_the_after_hit_route() {
        init_config();
        let expected = Some(ConditionDriver::Trigger(ConditionTrigger {
            key: crate::engine::skill::rule::DefinitionKey::new(210, "None"),
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::AfterHit),
        }));

        for raw in ["210&19004#30631", "19004#30631&210"] {
            let conditions = parse_conditions(config::configs::get(), raw);
            let route = ConditionRoute::compile(&conditions).unwrap();

            assert_eq!(route.branches[0].driver, expected);
            assert_eq!(route.branches[0].subscriptions().len(), 1);
            assert_eq!(
                route.branches[0].subscriptions()[0].event,
                EventKind::SkillAction
            );
            assert_eq!(route.branches[0].conditions.len(), 2);
        }
    }

    #[test]
    fn team_roster_predicate_leaves_assassination_as_the_compound_driver() {
        init_config();
        let conditions = parse_conditions(config::configs::get(), "1000212#3122,3123&1001212");
        let route = ConditionRoute::compile(&conditions).unwrap();

        assert_eq!(
            route.branches[0].driver,
            Some(ConditionDriver::Trigger(ConditionTrigger {
                key: crate::engine::skill::rule::DefinitionKey::new(1001212, "Assassinate",),
                event: EventKind::SkillAction,
                phase: Some(SkillPhase::AfterHit),
            }))
        );
        assert_eq!(route.branches[0].conditions.len(), 2);
    }

    #[test]
    fn leading_reactive_predicate_drives_all_of_its_declared_events() {
        init_config();
        let conditions = parse_conditions(config::configs::get(), "583004#90071#20");
        let route = ConditionRoute::compile(&conditions).unwrap();

        assert_eq!(
            route.branches[0].subscriptions(),
            vec![
                ConditionTrigger {
                    key: crate::engine::skill::rule::DefinitionKey::new(
                        583004,
                        "AccTeamAddBuffCountByBuffId",
                    ),
                    event: EventKind::BuffAdded,
                    phase: None,
                },
                ConditionTrigger {
                    key: crate::engine::skill::rule::DefinitionKey::new(
                        583004,
                        "AccTeamAddBuffCountByBuffId",
                    ),
                    event: EventKind::BuffChanged,
                    phase: None,
                },
            ]
        );
    }

    #[test]
    fn standalone_action_threshold_owns_the_ally_action_route() {
        init_config();
        let conditions = parse_conditions(config::configs::get(), "535212#31060004#3");
        let route = ConditionRoute::compile(&conditions).unwrap();

        assert_eq!(
            route.branches[0].driver,
            Some(ConditionDriver::Trigger(ConditionTrigger {
                key: crate::engine::skill::rule::DefinitionKey::new(
                    535212,
                    "TypeIdBuffCountMoreThan",
                ),
                event: EventKind::AllyAction,
                phase: None,
            }))
        );
    }

    #[test]
    fn incompatible_drivers_fail_instead_of_using_the_first_condition() {
        init_config();
        let conditions = parse_conditions(config::configs::get(), "208&210");

        assert!(matches!(
            ConditionRoute::compile(&conditions),
            Err(RouteError::ConflictingConditionDrivers { .. })
        ));
    }

    #[test]
    fn pre_effect_publication_owns_same_phase_action_conditions() {
        init_config();
        let conditions = parse_conditions(config::configs::get(), "502203#0&34203#1#2&500203#1");

        let route = ConditionRoute::compile(&conditions).unwrap();

        assert_eq!(
            route.branches[0].driver,
            Some(ConditionDriver::Trigger(ConditionTrigger {
                key: crate::engine::skill::rule::DefinitionKey::new(34203, "UseSkillEffectTag"),
                event: EventKind::SkillEffectStarted,
                phase: Some(SkillPhase::Immediate),
            }))
        );
    }

    #[test]
    fn exact_condition_214_keeps_parent_specific_timing() {
        init_config();
        let conditions = parse_conditions(config::configs::get(), "214");

        let shell = ConditionRoute::compile(&conditions).unwrap();
        let synchronization = ConditionRoute::compile_for_behavior(
            &conditions,
            &crate::engine::skill::behavior::classify::BehaviorSpec::new(
                100022,
                "EzioBigSkillCheckTimes",
            ),
        )
        .unwrap();

        assert_eq!(
            shell.branches[0].driver,
            Some(ConditionDriver::Trigger(ConditionTrigger {
                key: crate::engine::skill::rule::DefinitionKey::new(214, "None"),
                event: EventKind::ShellDeployed,
                phase: None,
            }))
        );
        assert_eq!(
            synchronization.branches[0].driver,
            Some(ConditionDriver::Trigger(ConditionTrigger {
                key: crate::engine::skill::rule::DefinitionKey::new(214, "None"),
                event: EventKind::SkillAction,
                phase: Some(SkillPhase::AfterHit),
            }))
        );
    }

    #[test]
    fn incoming_attack_conditions_only_subscribe_transactional_behaviors() {
        init_config();
        let conditions = parse_conditions(config::configs::get(), "33204&25204");

        let transaction = ConditionRoute::compile_for_behavior(
            &conditions,
            &crate::engine::skill::behavior::classify::BehaviorSpec::new(
                50014,
                "ConsumeBuffByTypeId",
            ),
        )
        .unwrap();
        let modifier = ConditionRoute::compile_for_behavior(
            &conditions,
            &crate::engine::skill::behavior::classify::BehaviorSpec::new(10004, "AttrFix"),
        )
        .unwrap();

        assert_eq!(
            transaction.branches[0].driver,
            Some(ConditionDriver::Trigger(ConditionTrigger {
                key: crate::engine::skill::rule::DefinitionKey::new(33204, "HurtRestraint"),
                event: EventKind::SkillAction,
                phase: Some(SkillPhase::Immediate),
            }))
        );
        assert_eq!(modifier.branches[0].driver, None);
        assert!(modifier.branches[0].subscriptions().is_empty());
    }

    #[test]
    fn compatible_drivers_keep_the_first_exact_key() {
        init_config();
        let conditions = parse_conditions(config::configs::get(), "66203#1#0&180203#1#1#2");

        let route = ConditionRoute::compile(&conditions).unwrap();

        assert_eq!(
            route.branches[0].driver,
            Some(ConditionDriver::Trigger(ConditionTrigger {
                key: crate::engine::skill::rule::DefinitionKey::new(66203, "UseSpecificSkill"),
                event: EventKind::SkillAction,
                phase: Some(SkillPhase::Immediate),
            }))
        );
        assert_eq!(route.branches[0].conditions.len(), 2);
    }

    #[test]
    fn career_predicate_preserves_the_exact_skill_action_driver() {
        init_config();
        let expected = Some(ConditionDriver::Trigger(ConditionTrigger {
            key: crate::engine::skill::rule::DefinitionKey::new(629210, "TeammateInjuryCount"),
            event: EventKind::SkillAction,
            phase: None,
        }));

        for raw in ["16210#1&629210#3", "629210#3&16210#1"] {
            let conditions = parse_conditions(config::configs::get(), raw);
            let route = ConditionRoute::compile(&conditions).unwrap();

            assert_eq!(route.branches[0].driver, expected);
            assert_eq!(route.branches[0].conditions.len(), 2);
        }
    }

    #[test]
    fn buff_sync_drives_its_companion_enemy_buff_count_predicate() {
        init_config();
        let conditions =
            parse_conditions(config::configs::get(), "19104#30810102&565104#4150001#6");

        let route = ConditionRoute::compile(&conditions).unwrap();

        assert_eq!(
            route.branches[0].driver,
            Some(ConditionDriver::Setup(ConditionSetup {
                key: crate::engine::skill::rule::DefinitionKey::new(19104, "HasBuffId"),
                stage: SetupStage::BuffSync,
                priority: 0,
            }))
        );
        assert_eq!(route.branches[0].conditions.len(), 2);
    }

    #[test]
    fn setup_condition_compiles_to_its_stage() {
        init_config();
        let conditions = parse_conditions(config::configs::get(), "595002#3091");
        let route = ConditionRoute::compile(&conditions).unwrap();

        assert_eq!(
            route.branches[0].driver,
            Some(ConditionDriver::Setup(ConditionSetup {
                key: crate::engine::skill::rule::DefinitionKey::new(595002, "TargetIncludeHero",),
                stage: SetupStage::EnterFight,
                priority: 0,
            }))
        );
    }

    #[test]
    fn entity_count_event_comes_from_its_exact_definition() {
        init_config();
        let conditions = parse_conditions(config::configs::get(), "24102");
        let route = ConditionRoute::compile(&conditions).unwrap();

        assert_eq!(
            route.branches[0].driver,
            Some(ConditionDriver::Trigger(ConditionTrigger {
                key: crate::engine::skill::rule::DefinitionKey::new(24102, "TeammateAlive"),
                event: EventKind::RoundStart,
                phase: None,
            }))
        );
    }

    #[test]
    fn common_lifecycle_and_buff_gate_routes_compile_exactly() {
        init_config();
        let setup =
            ConditionRoute::compile(&parse_conditions(config::configs::get(), "5")).unwrap();
        let round_start = ConditionRoute::compile(&parse_conditions(
            config::configs::get(),
            "57104#530000111&57104#530000112",
        ))
        .unwrap();
        let action =
            ConditionRoute::compile(&parse_conditions(config::configs::get(), "208&19208#30631"))
                .unwrap();

        assert_eq!(
            setup.branches[0].driver,
            Some(ConditionDriver::Setup(ConditionSetup {
                key: crate::engine::skill::rule::DefinitionKey::new(5, "EnterFight"),
                stage: SetupStage::EnterFight,
                priority: 0,
            }))
        );
        assert_eq!(round_start.branches[0].driver, None);
        assert_eq!(
            round_start.branches[0].subscriptions(),
            vec![ConditionTrigger {
                key: crate::engine::skill::rule::DefinitionKey::new(57104, "NoBuffId"),
                event: EventKind::RoundStart,
                phase: None,
            }]
        );
        assert_eq!(
            action.branches[0].driver,
            Some(ConditionDriver::Trigger(ConditionTrigger {
                key: crate::engine::skill::rule::DefinitionKey::new(208, "None"),
                event: EventKind::SkillAction,
                phase: Some(SkillPhase::AfterDamage),
            }))
        );
        assert_eq!(action.branches[0].conditions.len(), 2);
    }

    #[test]
    fn bloodtithe_spend_owns_the_contextual_no_buff_setup_route() {
        init_config();
        let route = ConditionRoute::compile_for_behavior(
            &parse_conditions(config::configs::get(), "57104#31200143"),
            &crate::engine::skill::behavior::classify::BehaviorSpec::new(
                60211,
                "ConsumeBloodAddBuff2",
            ),
        )
        .unwrap();

        assert_eq!(
            route.branches[0].driver,
            Some(ConditionDriver::Setup(ConditionSetup {
                key: crate::engine::skill::rule::DefinitionKey::new(57104, "NoBuffId"),
                stage: SetupStage::RoundStart,
                priority: 3,
            }))
        );
    }

    #[test]
    fn active_skill_tag_stays_a_predicate_on_the_ally_action_driver() {
        init_config();
        let route = ConditionRoute::compile(&parse_conditions(
            config::configs::get(),
            "502212#1&34212#3",
        ))
        .unwrap();

        assert_eq!(
            route.branches[0].driver,
            Some(ConditionDriver::Trigger(ConditionTrigger {
                key: crate::engine::skill::rule::DefinitionKey::new(502212, "ActiveUseSkill"),
                event: EventKind::AllyAction,
                phase: None,
            }))
        );
        assert_eq!(route.branches[0].conditions.len(), 2);
    }

    #[test]
    fn can_use_skill_stays_a_predicate_on_its_exact_skill_action_driver() {
        init_config();
        let route = ConditionRoute::compile(&parse_conditions(
            config::configs::get(),
            "507201#308801711&615201#308801711&578#20",
        ))
        .unwrap();

        assert_eq!(
            route.branches[0].driver,
            Some(ConditionDriver::Trigger(ConditionTrigger {
                key: crate::engine::skill::rule::DefinitionKey::new(507201, "UseSkillId"),
                event: EventKind::SkillAction,
                phase: None,
            }))
        );
        assert_eq!(route.branches[0].conditions.len(), 3);
        assert_eq!(
            route.branches[0].conditions[1].key,
            crate::engine::skill::rule::DefinitionKey::new(615201, "CanUseSkill")
        );
    }

    #[test]
    fn hp_threshold_stays_a_predicate_on_the_attacked_driver() {
        init_config();
        let route =
            ConditionRoute::compile(&parse_conditions(config::configs::get(), "1209#300&22209"))
                .unwrap();

        assert_eq!(
            route.branches[0].driver,
            Some(ConditionDriver::Trigger(ConditionTrigger {
                key: crate::engine::skill::rule::DefinitionKey::new(22209, "BeAttacked"),
                event: EventKind::TargetAttacked,
                phase: None,
            }))
        );
        assert_eq!(route.branches[0].conditions.len(), 2);
    }

    #[test]
    fn mirror_rule_compounds_keep_one_exact_driver() {
        init_config();

        let cases = [
            (
                "45100#2#1&57100#11790011",
                ConditionDriver::Setup(ConditionSetup {
                    key: crate::engine::skill::rule::DefinitionKey::new(45100, "HeroRoundInterval"),
                    stage: SetupStage::RoundStart,
                    priority: -1,
                }),
            ),
            (
                "22209&19209#11790012",
                ConditionDriver::Trigger(ConditionTrigger {
                    key: crate::engine::skill::rule::DefinitionKey::new(22209, "BeAttacked"),
                    event: EventKind::TargetAttacked,
                    phase: None,
                }),
            ),
            (
                "51213#11790022#5&19213#11790012",
                ConditionDriver::Trigger(ConditionTrigger {
                    key: crate::engine::skill::rule::DefinitionKey::new(19213, "HasBuffId"),
                    event: EventKind::SkillAction,
                    phase: Some(SkillPhase::HitPassives),
                }),
            ),
            (
                "51213#11790022#5&57213#11790012",
                ConditionDriver::Trigger(ConditionTrigger {
                    key: crate::engine::skill::rule::DefinitionKey::new(57213, "NoBuffId"),
                    event: EventKind::SkillAction,
                    phase: Some(SkillPhase::HitPassives),
                }),
            ),
        ];

        for (raw, expected) in cases {
            let route =
                ConditionRoute::compile(&parse_conditions(config::configs::get(), raw)).unwrap();
            assert_eq!(route.branches[0].driver, Some(expected));
            assert_eq!(route.branches[0].conditions.len(), 2);
        }
    }

    #[test]
    fn captured_conditions_keep_their_exact_drivers() {
        init_config();

        let unconditional =
            ConditionRoute::compile(&parse_conditions(config::configs::get(), "0")).unwrap();
        let bloodtithe = ConditionRoute::compile(&parse_conditions(
            config::configs::get(),
            "1203#800&740203#50#999",
        ))
        .unwrap();
        let critical_field = ConditionRoute::compile(&parse_conditions(
            config::configs::get(),
            "30210&542210#30003",
        ))
        .unwrap();

        assert_eq!(unconditional.branches[0].driver, None);
        assert_eq!(
            bloodtithe.branches[0].driver,
            Some(ConditionDriver::Trigger(ConditionTrigger {
                key: crate::engine::skill::rule::DefinitionKey::new(740203, "BloodPoolMax"),
                event: EventKind::SkillAction,
                phase: Some(SkillPhase::Immediate),
            }))
        );
        assert_eq!(
            critical_field.branches[0].driver,
            Some(ConditionDriver::Trigger(ConditionTrigger {
                key: crate::engine::skill::rule::DefinitionKey::new(542210, "InMagicCircleId"),
                event: EventKind::SkillAction,
                phase: Some(SkillPhase::AfterHit),
            }))
        );
        assert_eq!(bloodtithe.branches[0].conditions.len(), 2);
        assert_eq!(critical_field.branches[0].conditions.len(), 2);
    }

    #[test]
    fn parsed_but_unregistered_conditions_still_fail() {
        let condition = ParsedCondition {
            opcode: 999_999,
            type_name: "LifeLess".into(),
            kind: ParsedConditionKind::HpPermille {
                compare: crate::engine::skill::condition::ConditionCompare::LessThan,
                threshold: 500,
            },
            raw_args: vec!["500".into()],
        };

        assert!(matches!(
            ConditionRoute::compile(&[condition]),
            Err(RouteError::UnregisteredExactKey {
                opcode: 999_999,
                ..
            })
        ));
    }

    #[test]
    fn exact_active_skill_aliases_keep_their_phases() {
        init_config();

        for (raw, opcode, phase) in [
            ("662208#30630121,30630122", 662208, SkillPhase::AfterDamage),
            ("34210#9", 34210, SkillPhase::HitPassives),
        ] {
            let route =
                ConditionRoute::compile(&parse_conditions(config::configs::get(), raw)).unwrap();
            assert_eq!(
                route.branches[0].driver,
                Some(ConditionDriver::Trigger(ConditionTrigger {
                    key: crate::engine::skill::rule::DefinitionKey::new(
                        opcode,
                        if opcode == 662208 {
                            "ActiveUseSkillId"
                        } else {
                            "UseSkillEffectTag"
                        },
                    ),
                    event: EventKind::SkillAction,
                    phase: Some(phase),
                }))
            );
        }
    }

    #[test]
    fn repeated_cast_keeps_both_exact_after_hit_gates() {
        init_config();
        let route = ConditionRoute::compile(&parse_conditions(
            config::configs::get(),
            "552402#250&620402#2,3",
        ))
        .unwrap();

        assert_eq!(
            route.branches[0].driver,
            Some(ConditionDriver::Trigger(ConditionTrigger {
                key: crate::engine::skill::rule::DefinitionKey::new(552402, "Random"),
                event: EventKind::SkillAction,
                phase: Some(SkillPhase::AfterHit),
            }))
        );
        assert_eq!(route.branches[0].conditions.len(), 2);
    }
}
