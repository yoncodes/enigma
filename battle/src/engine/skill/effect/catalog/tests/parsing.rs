use super::*;
use crate::engine::skill::behavior::registry::BehaviorConfigError;

#[test]
fn parses_hash_cells_as_opcode_and_args() {
    assert_eq!(parse_i32_list("60002#1#2"), vec![60002, 1, 2]);
    assert_eq!(parse_i32_list(""), Vec::<i32>::new());
}

#[test]
fn burn_return_keeps_exact_argument_arity() {
    init_config();
    let parse = |behavior| {
        parse_slot(
            config::configs::get(),
            RawSlot {
                behavior,
                target: "103",
                condition: "210",
                condition_target: "103",
                logic_target: "103",
                limit: 0,
                round_limit: 0,
            },
        )
    };
    for raw in ["60059#4150001", "60059#4150001#1"] {
        assert!(parse(raw).is_ok(), "{raw}");
    }
    for raw in ["60059#4150001#", "60059#4150001##1", "60059#4150001#2"] {
        assert_eq!(parse(raw), Err(SlotParseError::Behavior), "{raw}");
    }
}

#[test]
fn reserve_clear_requires_the_defeated_owner_as_both_targets() {
    init_config();
    let db = config::configs::get();
    for id in [9260002, 116362010] {
        let catalog = SkillEffectCatalog::from_roots(db, [id], []);
        assert!(
            catalog
                .get(id)
                .unwrap()
                .slots
                .iter()
                .any(|slot| slot.behavior.spec.key.opcode == 60178)
        );
    }

    for (condition, target, condition_target) in [
        ("8#", "103", "103"),
        ("", "103", "103"),
        ("8", "202", "103"),
        ("8", "103", "102"),
        ("8", "999", "103"),
    ] {
        let result = parse_slot(
            db,
            RawSlot {
                behavior: "60178",
                target,
                condition,
                condition_target,
                logic_target: "103",
                limit: 0,
                round_limit: 0,
            },
        );
        assert_eq!(
            result,
            Err(SlotParseError::BehaviorConfig(
                BehaviorConfigError::Unsupported
            ))
        );
    }
}

#[test]
fn scalar_progress_requires_complete_condition_arguments() {
    init_config();
    let db = config::configs::get();
    for valid in ["8", "502212#0", "502212#0&501212&500212#2"] {
        let parse = |condition| {
            parse_slot(
                db,
                RawSlot {
                    behavior: "60107#0#4",
                    target: "103",
                    condition,
                    condition_target: "103",
                    logic_target: "103",
                    limit: 0,
                    round_limit: 0,
                },
            )
        };
        assert!(parse(valid).is_ok(), "{valid}");
    }
    let mut unconditional = db.skill_effect.get(9260008).unwrap().clone();
    unconditional.condition1.clear();
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert_configured_effect(db, &unconditional);
    assert_eq!(
        catalog.issues(unconditional.id)[0].reason,
        RuleIssueReason::UnsupportedBehavior
    );

    for (id, condition) in [
        (9260008, "8#"),
        (117333503, "502212#0#"),
        (117333503, "502212#0&"),
        (117333503, "502212#0|"),
    ] {
        let mut row = db.skill_effect.get(id).unwrap().clone();
        row.condition1 = condition.to_owned();
        let mut catalog = SkillEffectCatalog::default();
        catalog.insert_configured_effect(db, &row);
        let [issue] = catalog.issues(id) else {
            panic!("expected one issue for {condition}");
        };
        assert_eq!(issue.reason, RuleIssueReason::MalformedCondition);
        assert_eq!(issue.raw, condition);
        assert!(issue.reason.is_blocking());
    }

    let mut altered = db.skill_effect.get(8300201).unwrap().clone();
    altered.condition2.push('!');
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert_configured_effect(db, &altered);
    let issue = catalog
        .issues(altered.id)
        .iter()
        .find(|issue| issue.slot == 2)
        .unwrap();
    assert_eq!(
        issue.reason,
        RuleIssueReason::StandaloneConditionRequired {
            condition: DefinitionKey::new(742209, "PerBloodPoolAddOrDec"),
            required: DefinitionKey::new(60107, "ProgressChange"),
        }
    );
    assert_eq!(issue.opcode, Some(60107));
    assert_eq!(issue.type_name.as_deref(), Some("ProgressChange"));
    assert_eq!(issue.raw, altered.condition2);
    assert!(issue.reason.is_blocking());

    altered.condition3.insert_str(0, "208&210&");
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert_configured_effect(db, &altered);
    let issue = catalog
        .issues(altered.id)
        .iter()
        .find(|issue| issue.slot == 3)
        .unwrap();
    assert_eq!(
        issue.reason,
        RuleIssueReason::StandaloneConditionRequired {
            condition: DefinitionKey::new(742210, "PerBloodPoolAddOrDec"),
            required: DefinitionKey::new(60107, "ProgressChange"),
        }
    );
    assert_eq!(issue.raw, altered.condition3);
    assert!(issue.reason.is_blocking());

    use crate::engine::{
        event::kind::EventKind,
        manager::BattleManagers,
        skill::{subscriber, target::TargetPool},
    };
    use sonettobuf::{Fight, FightEntityInfo, FightTeam};

    let mut malformed = db.skill_effect.get(9260008).unwrap().clone();
    malformed.condition1.push('#');
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                passive_skill: vec![malformed.id],
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert_configured_effect(db, &malformed);
    assert!(matches!(
        subscriber::for_compiled_events(&pool, &managers, &catalog, [EventKind::SkillAction]),
        Err(subscriber::SubscriberError::MalformedCondition {
            skill_id: 9260008,
            ..
        })
    ));
}

#[test]
fn parses_target_code_and_raw_args() {
    let target = parse_target("103#7").unwrap();

    assert_eq!(target.code, 103);
    assert_eq!(target.raw, vec![7]);
    assert_eq!(parse_target(""), Some(TargetRequest::default()));
    for raw in ["bad", "bad#103", "103#bad", "103##7", "103#"] {
        assert_eq!(parse_target(raw), None, "{raw}");
    }
}

#[test]
fn predicate_grants_after_an_after_damage_slot_follow_its_reactions() {
    init_config();
    let db = config::configs::get();
    let phases = |effect_id: i32| {
        let mut catalog = SkillEffectCatalog::default();
        catalog.insert_configured_effect(db, db.skill_effect.get(effect_id).unwrap());
        catalog
            .get(effect_id)
            .unwrap()
            .slots
            .iter()
            .map(|slot| slot.active_phases().unwrap())
            .collect::<Vec<_>>()
    };
    let after_hit = vec![crate::engine::skill::action::SkillPhase::AfterHit];

    assert_eq!(phases(30870331)[4], after_hit);
    assert_eq!(
        phases(308801211)[1..],
        [after_hit.clone(), after_hit.clone()]
    );
    assert_eq!(phases(30630181)[3..5], [after_hit.clone(), after_hit]);
    assert!(phases(30030209).iter().all(Vec::is_empty));
}

#[test]
fn delegated_card_settles_moxie_grant_in_the_cast_phase() {
    init_config();
    let db = config::configs::get();
    let skill = db.skill.get(311401511).unwrap();
    let row = db.skill_effect.get(skill.skill_effect).unwrap();

    let mut catalog = SkillEffectCatalog::default();
    catalog.insert_configured_effect(db, row);
    let [cast, reward] = catalog.get(row.id).unwrap().slots.as_slice() else {
        panic!("Flood of Fiction should parse as cast + grant");
    };
    let cast_phases = cast.active_phases().unwrap();
    assert_eq!(cast_phases.len(), 1);
    assert_eq!(reward.active_phases().unwrap(), cast_phases);

    let reward_phase = |edit: fn(&mut config::skill_effect::SkillEffect)| {
        let mut variant = row.clone();
        edit(&mut variant);
        let mut catalog = SkillEffectCatalog::default();
        catalog.insert_configured_effect(db, &variant);
        catalog.get(row.id).unwrap().slots[1].delegated_reward_phase
    };
    // The grant follows the cast whatever gates it or whom it targets.
    let bound = Some(cast_phases[0]);
    assert_eq!(reward_phase(|row| row.condition2.clear()), bound);
    assert_eq!(
        reward_phase(|row| row.behavior_target2.push_str("#7")),
        bound
    );
    // Without the delegated cast, or for a non-Moxie slot, nothing is bound.
    assert_eq!(
        reward_phase(|row| row.logic_target = "101".to_owned()),
        None
    );
    assert_eq!(reward_phase(|row| row.behavior2 = "60175".to_owned()), None);
}

#[test]
fn slot_parser_rejects_malformed_targets() {
    init_config();
    let db = config::configs::get();
    let type_name = db.skill_behavior.get(60225).unwrap().r#type.clone();
    let parse = |target, condition_target, logic_target| {
        parse_slot(
            db,
            RawSlot {
                behavior: "60225#530000751:100&530000752:25",
                target,
                condition: "",
                condition_target,
                logic_target,
                limit: 0,
                round_limit: 0,
            },
        )
    };

    for result in [
        parse("bad", "103", "103"),
        parse("103", "bad", "103"),
        parse("999", "", "bad"),
    ] {
        assert_eq!(
            result,
            Err(SlotParseError::Target {
                raw: "bad".to_owned(),
                opcode: 60225,
                type_name: type_name.clone(),
            })
        );
    }

    for result in [
        parse("2147483647", "103", "103"),
        parse("103", "2147483647", "103"),
        parse("999", "", "2147483647"),
    ] {
        assert_eq!(
            result,
            Err(SlotParseError::Target {
                raw: "2147483647".to_owned(),
                opcode: 60225,
                type_name: type_name.clone(),
            })
        );
    }
}

#[test]
fn deck_top_rank_behavior_keeps_exact_config_identity() {
    init_config();

    let behavior = ParsedBehavior::new(60116, "CardDeckTopRankCorrect", vec![1, 1, 1]);

    assert!(crate::engine::skill::behavior::is_supported(&behavior));
    assert!(
        crate::engine::skill::behavior::registry::find_key(60116, "CardDeckTopRankCorrect")
            .is_some()
    );
}

#[test]
fn consume_buff_reset_device_requires_an_ally_action_route() {
    init_config();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());

    let everecho = &catalog.get(31450141).unwrap().slots[2];
    let event_free = &catalog.get(115380001).unwrap().slots[0];

    assert!(crate::engine::skill::behavior::is_slot_supported(everecho));
    assert!(crate::engine::skill::behavior::is_supported(
        &event_free.behavior
    ));
    assert!(!crate::engine::skill::behavior::is_slot_supported(
        event_free
    ));
}

#[test]
fn get_resolves_skill_id_alias_to_effect_id() {
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 200,
        slots: Vec::new(),
    });
    catalog.insert_alias(100, 200);

    assert_eq!(catalog.get(100).map(|effect| effect.skill_id), Some(200));
}

#[test]
fn get_prefers_skill_table_alias_over_same_id_effect_row() {
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: Vec::new(),
    });
    catalog.insert(ParsedSkillEffect {
        skill_id: 200,
        slots: Vec::new(),
    });
    catalog.insert_alias(100, 200);

    assert_eq!(catalog.get(100).map(|effect| effect.skill_id), Some(200));
}

#[test]
fn alternative_opcodes_share_one_event_lane_inside_their_parent_slot() {
    init_config();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let effect = catalog.get(30864156).unwrap();

    assert_eq!(effect.slots[0].compiled_subscriptions().unwrap().len(), 2);
    let lanes = catalog.compiled_subscription_lanes(30864156).unwrap();
    let ex_point_lanes = lanes
        .iter()
        .filter(|(_, key)| key.event == crate::engine::event::kind::EventKind::ExPointChanged)
        .collect::<Vec<_>>();

    assert_eq!(ex_point_lanes.len(), 2);
    assert_ne!(ex_point_lanes[0].0, ex_point_lanes[1].0);
}

#[test]
fn alternative_hp_threshold_routes_share_their_configured_limit_owner() {
    init_config();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());

    for effect_id in [109320108, 118353131, 103360262, 109360007] {
        let effect = catalog.get(effect_id).unwrap();
        for buff_id in [109320114, 109320109, 109320110] {
            let owners = effect
                .slots
                .iter()
                .enumerate()
                .filter(|(_, slot)| slot.behavior.arg(0) == Some(buff_id))
                .map(|(slot_index, _)| catalog.rule_limit_owner(effect_id, slot_index))
                .collect::<Vec<_>>();

            assert_eq!(owners.len(), 3);
            assert!(owners.iter().all(|owner| *owner == owners[0]));
        }
    }
}

#[test]
fn sentience_threshold_routes_all_transformation_steps_to_round_end() {
    init_config();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let effect = catalog.get(26011).unwrap();

    assert_eq!(effect.slots.len(), 4);
    assert!(effect.slots.iter().all(|slot| {
        slot.compiled_subscriptions().unwrap()
            == vec![SubscriptionKey::new(
                crate::engine::event::kind::EventKind::RoundEnd,
                crate::engine::skill::rule::DefinitionKey::new(51302, "HasTypeIdBuffMoreThan"),
            )]
    }));
}

#[test]
fn static_action_point_modifier_has_no_runtime_event_lane() {
    init_config();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());

    assert!(
        catalog
            .compiled_subscription_lanes(23390162)
            .unwrap()
            .is_empty()
    );
    let battle_rule = catalog.get(72013).unwrap();
    assert!(
        battle_rule.slots[0]
            .compiled_setup_keys(crate::engine::skill::rule::SetupStage::RoundStart, -1)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn attack_tag_follows_skill_alias() {
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert_effect_tag(200, SkillEffectTag::RealityDamage as i32);
    catalog.insert_alias(100, 200);

    assert!(catalog.is_attack(100));
    assert!(!catalog.is_attack(300));
}

#[test]
fn debuff_tagged_damage_skill_is_still_an_attack() {
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert_effect_tag(100, 3);
    catalog.insert_damage_rate(100, 2000);

    assert!(catalog.is_attack(100));
}

#[test]
fn queue_preparation_does_not_grant_card_play_resource() {
    init_config();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());

    assert!(!catalog.grants_resource_on_card_play(31340168));
    assert!(catalog.grants_resource_on_card_play(31340163));
}

#[test]
fn from_game_db_preserves_behavior_comma_list_args() {
    init_config();

    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let slot = &catalog.get(2240010).unwrap().slots[0];
    let behavior = &slot.behavior;

    assert_eq!(behavior.spec.kind, BehaviorKind::ConsumePowerAddBuff);
    assert_eq!(behavior.args, vec![2]);
    assert_eq!(behavior.raw_args, vec!["2", "2240001,2240002"]);
    assert_eq!(behavior.arg_list(1), Some(vec![2240001, 2240002]));
    assert_eq!(slot.condition_target.code, 103);
}

#[test]
fn from_game_db_preserves_registered_weighted_skill_args() {
    init_config();

    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let effect = catalog.get(530000745).unwrap();
    let behavior = &effect.slots[1].behavior;

    assert_eq!(behavior.spec.kind, BehaviorKind::RandomUseSkill);
    assert_eq!(
        behavior.raw_args,
        vec!["530000751:100&530000752:100&530000753:100"]
    );
    assert_eq!(
        (crate::engine::skill::behavior::registry::find(behavior)
            .unwrap()
            .references)(behavior)
        .skills,
        vec![530000751, 530000752, 530000753]
    );
}

#[test]
fn slot_parser_rejects_partial_weighted_skill_groups() {
    init_config();
    let parse = |behavior| {
        parse_slot(
            config::configs::get(),
            RawSlot {
                behavior,
                target: "103",
                condition: "",
                condition_target: "103",
                logic_target: "103",
                limit: 0,
                round_limit: 0,
            },
        )
    };

    assert!(parse("60225#530000751:100&530000752:25").is_ok());
    assert_eq!(
        parse("60225#530000751:100&bad"),
        Err(SlotParseError::Behavior)
    );
    assert_eq!(parse("60225#-530000751:100"), Err(SlotParseError::Behavior));
}

#[test]
fn slot_parser_applies_registry_contract_to_numeric_behaviors() {
    init_config();
    let parse = |behavior| {
        parse_slot(
            config::configs::get(),
            RawSlot {
                behavior,
                target: "103",
                condition: "",
                condition_target: "103",
                logic_target: "103",
                limit: 0,
                round_limit: 0,
            },
        )
    };

    assert_eq!(parse("10001"), Err(SlotParseError::Behavior));
    assert!(parse("10001#100").is_ok());
    assert!(parse("60012").is_ok());
    for raw in ["60012#", "60012#1", "60012#bad"] {
        assert_eq!(parse(raw), Err(SlotParseError::Behavior), "{raw}");
    }
    assert!(parse("60175").is_ok());
    assert_eq!(parse("60175#bad"), Err(SlotParseError::Behavior));
    assert_eq!(parse("60175#1,2"), Err(SlotParseError::Behavior));
    assert!(parse("60231#100#40#1").is_ok());
    assert!(parse("60231#101#30#1").is_ok());
    assert_eq!(parse("60231#102#40#1"), Err(SlotParseError::Behavior));
    assert_eq!(parse("60231#100#40#1#bad"), Err(SlotParseError::Behavior));
    assert!(parse("60037#308606").is_ok());
    assert_eq!(parse("60037#0"), Err(SlotParseError::Behavior));
    assert_eq!(parse("60037#308606#bad"), Err(SlotParseError::Behavior));
    assert!(parse("50038#11450021").is_ok());
    assert!(parse("60019#0#102").is_ok());
    assert!(parse("60019#0#103").is_ok());
    assert!(parse("60056#151415").is_ok());
    assert!(parse("40006#119381111#1000#1#").is_ok());
    assert!(parse("60334#1").is_ok());
    for raw in ["60334##1", "60334#1#", "60334# #1"] {
        assert_eq!(parse(raw), Err(SlotParseError::Behavior), "{raw}");
    }
    assert!(parse("60056#151416").is_ok());
    for raw in [
        "60056",
        "60056#0",
        "60056#-1",
        "60056#151416#1",
        "60056#151416#bad",
    ] {
        assert_eq!(parse(raw), Err(SlotParseError::Behavior));
    }
    for raw in [
        "60019#0",
        "60019#0#999",
        "60019#1#151415",
        "60019#0#102#bad",
    ] {
        assert_eq!(parse(raw), Err(SlotParseError::Behavior));
    }
    for raw in ["50038", "50038#0", "50038#11450021#1", "50038#11450021#bad"] {
        assert_eq!(parse(raw), Err(SlotParseError::Behavior));
    }
    assert!(parse("60053#30860143#0#0#5").is_ok());
    assert_eq!(
        parse("60053#102011481#0#1#5"),
        Err(SlotParseError::Behavior)
    );
    assert_eq!(
        parse("60053#30860143#0#0#5#bad"),
        Err(SlotParseError::Behavior)
    );
    assert!(parse("60285#31420001#10011#1").is_ok());
    assert!(parse("60285#301#10011#2").is_ok());
    assert_eq!(
        parse("60285#31420001#10011#1#bad"),
        Err(SlotParseError::Behavior)
    );
    assert_eq!(
        parse("60285#31420001#10002#1"),
        Err(SlotParseError::Behavior)
    );
    assert_eq!(
        parse("60285#31420001#10011#3"),
        Err(SlotParseError::Behavior)
    );
    assert!(parse("60112#2#2#31040005#2").is_ok());
    assert_eq!(parse("60112#2#0#31040005#2"), Err(SlotParseError::Behavior));
    assert_eq!(parse("60112#3#2#31040005#2"), Err(SlotParseError::Behavior));
    assert_eq!(
        parse("60112#2#2#31040005#2#bad"),
        Err(SlotParseError::Behavior)
    );
    assert!(parse("60083#1#1#1000").is_ok());
    assert!(parse("60083#2#1#4000").is_ok());
    assert_eq!(parse("60083#3#1#1000"), Err(SlotParseError::Behavior));
    assert_eq!(parse("60083#1#0#1000"), Err(SlotParseError::Behavior));
    assert_eq!(parse("60083#1#1#1000#bad"), Err(SlotParseError::Behavior));
    assert!(parse("60034#90071#0#600").is_ok());
    assert_eq!(
        parse("60034#90071#0#600#bad"),
        Err(SlotParseError::Behavior)
    );
    assert!(parse("60261#30920101#80#30920102").is_ok());
    assert_eq!(
        parse("60261#30920101#0#30920102"),
        Err(SlotParseError::Behavior)
    );
    assert_eq!(
        parse("60261#30920101#80#30920102#bad"),
        Err(SlotParseError::Behavior)
    );
}

#[test]
fn from_game_db_keeps_slot_round_limits() {
    init_config();

    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());

    assert_eq!(
        catalog.round_limit_for_condition(
            30630151,
            crate::engine::skill::rule::DefinitionKey::new(508212, "CareerCheck"),
        ),
        1
    );
}

#[test]
fn team_entity_exit_conditions_supply_their_round_limit() {
    init_config();

    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let base = catalog.get(30865171).unwrap();
    let upgraded = catalog.get(30865175).unwrap();
    let euphoria = catalog.get(30865186).unwrap();

    assert_eq!(base.slots[4].round_limit, 99);
    assert_eq!(base.slots[5].round_limit, 2);
    assert_eq!(upgraded.slots[5].round_limit, 4);
    assert_eq!(euphoria.slots[5].round_limit, 4);
}

#[test]
fn active_skill_filters_share_the_exact_skill_action_driver() {
    init_config();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());

    let subscriptions = catalog.compiled_subscriptions(435011).unwrap();
    assert!(
        subscriptions.contains(
            &SubscriptionKey::at_phase_and_publication(
                crate::engine::event::kind::EventKind::SkillAction,
                crate::engine::skill::rule::DefinitionKey::new(34210, "UseSkillEffectTag"),
                Some(crate::engine::skill::action::SkillPhase::HitPassives),
                crate::engine::event::subscription::PublicationPhase::BeforePublish,
            )
            .with_timing(crate::engine::event::subscription::ReactionTiming::AfterSkill)
        )
    );
    assert!(!subscriptions.iter().any(|subscription| {
        subscription.event == crate::engine::event::kind::EventKind::SkillAction
            && subscription.definition.opcode == 16210
    }));
}

#[test]
fn replacing_an_effect_updates_its_aliased_subscription_lanes() {
    init_config();
    let skill_id = 30864156;
    let alias = i32::MAX;
    let mut catalog = SkillEffectCatalog::from_roots(config::configs::get(), [skill_id], []);
    let mut effect = catalog.get(skill_id).unwrap().clone();
    catalog.insert_alias(alias, effect.skill_id);
    let lanes = catalog.compiled_subscription_lanes(alias).unwrap();
    assert!(!lanes.is_empty());
    assert_eq!(
        lanes,
        catalog.compiled_subscription_lanes(skill_id).unwrap()
    );

    effect.slots.clear();
    catalog.insert(effect);

    assert!(
        catalog
            .compiled_subscription_lanes(alias)
            .unwrap()
            .is_empty()
    );
    assert!(
        catalog
            .compiled_subscription_lanes(skill_id)
            .unwrap()
            .is_empty()
    );
}
