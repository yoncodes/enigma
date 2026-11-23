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
            Some(SkillPhase::SourceOpening),
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
fn active_buff_presence_routes_only_the_declared_grant() {
    crate::test_support::init_config();
    let slot = |raw, behavior: ParsedBehavior| {
        let mut slot = SkillEffectSlot::new(behavior, TargetRequest::self_only());
        slot.conditions =
            crate::engine::skill::condition::parse_conditions(config::configs::get(), raw);
        slot.compiled_route =
            ConditionRoute::compile_for_behavior(&slot.conditions, &slot.behavior.spec);
        slot
    };
    let grant = |raw| slot(raw, ParsedBehavior::new(1, "AddBuff", vec![302]));

    let standalone = grant("19210#4150001");
    assert_eq!(standalone.active_phases().unwrap(), [SkillPhase::AfterHit]);
    assert_eq!(standalone.compiled_subscriptions().unwrap()[0].phase, None);
    assert_eq!(
        grant("19210#4150001&208").active_phases().unwrap(),
        [SkillPhase::AfterDamage]
    );
    assert!(grant("19210#4150001!").active_phases().unwrap().is_empty());
    let consume = slot(
        "19210#4150001",
        ParsedBehavior::new(50014, "ConsumeBuffByTypeId", vec![4150001, 1]),
    );
    assert!(consume.active_phases().unwrap().is_empty());
    assert!(
        crate::engine::skill::behavior::registry::find(&consume.behavior)
            .unwrap()
            .phase
            == SkillPhase::AfterDamage
    );
}

#[test]
fn compound_hp_threshold_keeps_its_policy_on_the_attacked_driver() {
    crate::test_support::init_config();
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::new(1, "AddBuff", Vec::new()),
        TargetRequest::self_only(),
    );
    slot.conditions =
        crate::engine::skill::condition::parse_conditions(config::configs::get(), "1209#300&22209");
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);

    let subscription = slot.compiled_subscriptions().unwrap().remove(0);
    assert_eq!(
        subscription.definition,
        crate::engine::skill::rule::DefinitionKey::new(22209, "BeAttacked")
    );
    assert_eq!(
        subscription.timing_policy,
        crate::engine::skill::rule::DefinitionKey::new(1209, "LifeLess")
    );
    assert_eq!(subscription.multiplicity_policy, subscription.timing_policy);
    assert_eq!(
        subscription.timing,
        crate::engine::event::subscription::ReactionTiming::HpThresholds
    );
}

#[test]
fn attacked_compound_route_keeps_driver_timing_and_multiplicity() {
    crate::test_support::init_config();
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::new(1, "AddBuff", Vec::new()),
        TargetRequest::self_only(),
    );
    slot.conditions = crate::engine::skill::condition::parse_conditions(
        config::configs::get(),
        "19209#109380007&501209",
    );
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);

    let subscription = slot.compiled_subscriptions().unwrap().remove(0);
    assert_eq!(
        subscription.timing_policy,
        crate::engine::skill::rule::DefinitionKey::new(501209, "UseHurtSkill")
    );
    assert_eq!(
        subscription.multiplicity_policy,
        crate::engine::skill::rule::DefinitionKey::new(501209, "UseHurtSkill")
    );
    assert_eq!(
        subscription.timing,
        crate::engine::event::subscription::ReactionTiming::AttackedSkills
    );
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
    predicate_only.conditions =
        crate::engine::skill::condition::parse_conditions(config::configs::get(), "19002#437211");
    predicate_only.compiled_route = ConditionRoute::compile(&predicate_only.conditions);

    let mut driven = SkillEffectSlot::new(
        ParsedBehavior::new(1, "AddBuff", Vec::new()),
        TargetRequest::self_only(),
    );
    driven.conditions =
        crate::engine::skill::condition::parse_conditions(config::configs::get(), "5&19002#437211");
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
        vec![crate::engine::skill::rule::DefinitionKey::new(
            5,
            "EnterFight"
        )]
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
