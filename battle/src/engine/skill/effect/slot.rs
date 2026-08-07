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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillEffectSlot {
    pub behavior: ParsedBehavior,
    pub conditions: Vec<ParsedCondition>,
    pub compiled_route: Result<ConditionRoute, RouteError>,
    pub condition_target: TargetRequest,
    pub target: TargetRequest,
    pub target_from_condition: bool,
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
            limit: 0,
            round_limit: 0,
        }
    }

    pub fn subscriptions(&self) -> Vec<SubscriptionKey> {
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
                definition.round_modifier_only
                    || definition.card_play_role
                        == crate::engine::skill::behavior::registry::CardPlayRole::QueuePreparation
            },
        ) {
            return Ok(Vec::new());
        }
        let mut subscriptions = route
            .branches
            .iter()
            .flat_map(|branch| branch.subscriptions())
            .map(|trigger| {
                let publication = registry::find_key(trigger.key.opcode, trigger.key.type_name)
                    .map(|definition| definition.publication)
                    .unwrap_or(PublicationPhase::AfterPublish);
                let timing = registry::find_key(trigger.key.opcode, trigger.key.type_name)
                    .map(|definition| definition.reaction_timing)
                    .unwrap_or_default();
                SubscriptionKey::at_phase(trigger.event, trigger.key, trigger.phase)
                    .with_publication(publication)
                    .with_timing(timing)
            })
            .collect::<Vec<_>>();
        for condition in &self.conditions {
            let Some(definition) = registry::find_key(condition.opcode, &condition.type_name)
            else {
                continue;
            };
            for &event in definition.reactivation_events {
                let key = SubscriptionKey::new(event, definition.key)
                    .with_publication(definition.publication)
                    .with_timing(definition.reaction_timing);
                if !subscriptions.contains(&key) {
                    subscriptions.push(key);
                }
            }
        }
        Ok(subscriptions)
    }

    pub fn active_phases(&self) -> Result<Vec<SkillPhase>, RouteError> {
        let mut phases = Vec::new();
        for subscription in self.compiled_subscriptions()? {
            if subscription.event == crate::engine::event::kind::EventKind::SkillAction
                && let Some(phase) = subscription.phase
                && !phases.contains(&phase)
            {
                phases.push(phase);
            }
        }
        Ok(phases)
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
        if crate::engine::skill::behavior::registry::find(&self.behavior)
            .is_some_and(|definition| definition.round_modifier_only)
        {
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
    pub config_effect: i32,
}

impl ParsedBehavior {
    #[cfg(test)]
    pub fn new(opcode: i32, type_name: &str, args: Vec<i32>) -> Self {
        Self {
            spec: BehaviorSpec::new(opcode, type_name),
            args,
            raw_args: Vec::new(),
            config_effect: 0,
        }
    }

    pub fn from_spec(spec: BehaviorSpec, args: Vec<i32>, raw_args: Vec<String>) -> Self {
        Self {
            config_effect: spec.key.opcode,
            spec,
            args,
            raw_args,
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
mod tests {
    use super::*;
    use crate::engine::{
        event::kind::EventKind,
        skill::condition::{ParsedConditionKind, none::NoneMode},
    };

    #[test]
    fn arg_list_preserves_comma_group_from_raw_args() {
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(60142, "ConsumePowerAddBuff"),
            vec![2],
            vec!["2".to_owned(), "2240001,2240002".to_owned()],
        );

        assert_eq!(behavior.arg(0), Some(2));
        assert_eq!(behavior.arg_list(1), Some(vec![2240001, 2240002]));
    }

    #[test]
    fn malformed_arg_list_fails_closed() {
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(60142, "ConsumePowerAddBuff"),
            vec![2],
            vec!["2".to_owned(), "2240001,bad".to_owned()],
        );

        assert_eq!(behavior.arg_list(1), None);
    }

    #[test]
    fn compound_condition_line_subscribes_once_per_event() {
        let mut slot = SkillEffectSlot::new(
            ParsedBehavior::new(1, "AddBuff", Vec::new()),
            TargetRequest::self_only(),
        );
        slot.conditions = vec![
            ParsedCondition {
                opcode: 201,
                type_name: "None".to_owned(),
                kind: ParsedConditionKind::None(NoneMode::SkillAction),
                raw_args: Vec::new(),
            },
            ParsedCondition {
                opcode: 203,
                type_name: "None".to_owned(),
                kind: ParsedConditionKind::None(NoneMode::SkillAction),
                raw_args: Vec::new(),
            },
        ];

        assert_eq!(
            slot.subscriptions(),
            vec![SubscriptionKey::at_phase(
                EventKind::SkillAction,
                crate::engine::skill::rule::DefinitionKey::new(201, "None"),
                Some(SkillPhase::Immediate),
            )]
        );
    }

    #[test]
    fn first_event_condition_drives_compound_condition_line() {
        let mut slot = SkillEffectSlot::new(
            ParsedBehavior::new(1, "AddBuff", Vec::new()),
            TargetRequest::self_only(),
        );
        slot.conditions = vec![
            ParsedCondition {
                opcode: 212,
                type_name: "None".to_owned(),
                kind: ParsedConditionKind::None(NoneMode::AllyAction),
                raw_args: Vec::new(),
            },
            ParsedCondition {
                opcode: 203,
                type_name: "None".to_owned(),
                kind: ParsedConditionKind::None(NoneMode::SkillAction),
                raw_args: Vec::new(),
            },
        ];

        assert_eq!(
            slot.subscriptions(),
            vec![SubscriptionKey::new(
                EventKind::AllyAction,
                crate::engine::skill::rule::DefinitionKey::new(212, "None"),
            )]
        );
    }

    #[test]
    fn compiled_subscription_keeps_the_exact_phase() {
        let mut slot = SkillEffectSlot::new(
            ParsedBehavior::new(1, "AddBuff", Vec::new()),
            TargetRequest::self_only(),
        );
        slot.conditions = vec![ParsedCondition {
            opcode: 208,
            type_name: "None".to_owned(),
            kind: ParsedConditionKind::None(NoneMode::SkillAction),
            raw_args: Vec::new(),
        }];
        slot.compiled_route = ConditionRoute::compile(&slot.conditions);

        assert_eq!(
            slot.compiled_subscriptions().unwrap(),
            vec![SubscriptionKey::at_phase(
                EventKind::SkillAction,
                crate::engine::skill::rule::DefinitionKey::new(208, "None"),
                Some(crate::engine::skill::action::SkillPhase::AfterDamage),
            )]
        );
        assert_eq!(slot.active_phases().unwrap(), vec![SkillPhase::AfterDamage]);
    }

    #[test]
    fn exact_none_condition_keeps_event_and_enter_battle_static_routes() {
        let mut slot = SkillEffectSlot::new(
            ParsedBehavior::new(1, "AddBuff", Vec::new()),
            TargetRequest::self_only(),
        );
        slot.conditions = vec![ParsedCondition {
            opcode: 55,
            type_name: "None".to_owned(),
            kind: ParsedConditionKind::None(NoneMode::EnterBattle),
            raw_args: Vec::new(),
        }];
        slot.compiled_route = ConditionRoute::compile(&slot.conditions);

        assert_eq!(
            slot.compiled_subscriptions().unwrap(),
            vec![SubscriptionKey::new(
                EventKind::EntityEntered,
                crate::engine::skill::rule::DefinitionKey::new(55, "None"),
            )]
        );
        assert_eq!(
            slot.compiled_setup_keys(SetupStage::EnterBattleStatic, 0)
                .unwrap(),
            vec![crate::engine::skill::rule::DefinitionKey::new(55, "None")]
        );
    }

    #[test]
    fn predicate_companion_setup_only_drives_slots_without_an_explicit_driver() {
        crate::test_support::init_config();
        let mut predicate_only = SkillEffectSlot::new(
            ParsedBehavior::new(1, "AddBuff", Vec::new()),
            TargetRequest::self_only(),
        );
        predicate_only.conditions = crate::engine::skill::condition::parse_conditions(
            config::configs::get(),
            "19002#437211",
        );
        predicate_only.compiled_route = ConditionRoute::compile(&predicate_only.conditions);

        let mut driven = SkillEffectSlot::new(
            ParsedBehavior::new(1, "AddBuff", Vec::new()),
            TargetRequest::self_only(),
        );
        driven.conditions = crate::engine::skill::condition::parse_conditions(
            config::configs::get(),
            "5&19002#437211",
        );
        driven.compiled_route = ConditionRoute::compile(&driven.conditions);

        assert_eq!(
            predicate_only
                .compiled_setup_keys(SetupStage::EnterFight, 0)
                .unwrap(),
            vec![crate::engine::skill::rule::DefinitionKey::new(
                19002,
                "HasBuffId"
            )]
        );
        assert_eq!(
            driven
                .compiled_setup_keys(SetupStage::EnterFight, 0)
                .unwrap(),
            vec![crate::engine::skill::rule::DefinitionKey::new(5, "EnterFight")]
        );
    }

    #[test]
    fn unsupported_condition_disables_only_its_slot() {
        let mut slot = SkillEffectSlot::new(
            ParsedBehavior::new(1, "AddBuff", Vec::new()),
            TargetRequest::self_only(),
        );
        slot.compiled_route = Err(RouteError::UnregisteredExactKey {
            opcode: 999,
            type_name: "FutureCondition".to_owned(),
        });

        assert!(slot.compiled_subscriptions().unwrap().is_empty());
        assert!(
            slot.compiled_setup_keys(SetupStage::BattleStart, 0)
                .unwrap()
                .is_empty()
        );
    }
}
