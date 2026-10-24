use super::*;

#[test]
fn parses_hash_cells_as_opcode_and_args() {
    assert_eq!(parse_i32_list("60002#1#2"), vec![60002, 1, 2]);
    assert_eq!(parse_i32_list(""), Vec::<i32>::new());
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
