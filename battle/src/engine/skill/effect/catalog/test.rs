use super::parse::{RawSlot, parse_slot};
use super::*;
use crate::test_support::init_config;

#[test]
fn fight_catalog_excludes_unrelated_heroes() {
    init_config();
    let fight = Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                passive_skill: vec![31270148],
                skill_group1: vec![31270111],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };

    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    assert!(catalog.get(31270148).is_some());
    assert!(catalog.get(31270111).is_some());
    assert!(catalog.get(31430141).is_none());
}

#[test]
fn fight_catalog_includes_assist_boss_active_skills() {
    init_config();
    let fight = Fight {
        attacker: Some(sonettobuf::FightTeam {
            assist_boss_info: Some(sonettobuf::AssistBossInfo {
                skills: vec![sonettobuf::AssistBossSkillInfo {
                    skill_id: Some(116331205),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    assert!(catalog.get(116331205).is_some());
}

#[test]
fn fight_catalog_compiles_enemy_condition_routes_after_registration() {
    init_config();
    let fight = Fight {
        defender: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                passive_skill: vec![31430149],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };

    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let effect = catalog.get(31430149).unwrap();

    assert!(catalog.issues(31430149).is_empty());
    assert!(effect.slots.iter().all(|slot| slot.compiled_route.is_ok()));
}

#[test]
fn dynamic_current_battle_roots_compile_registered_condition_routes() {
    init_config();
    let mut catalog = SkillEffectCatalog::default();

    catalog.extend_roots_and_warn(config::configs::get(), [31430149], []);

    assert!(catalog.issues(31430149).is_empty());
    assert!(
        catalog
            .get(31430149)
            .unwrap()
            .slots
            .iter()
            .all(|slot| slot.compiled_route.is_ok())
    );
    assert!(catalog.get(31430149).is_some());
    assert!(catalog.get(31270148).is_none());
}

#[test]
fn entering_entities_extend_the_scoped_catalog_from_their_own_roots() {
    init_config();
    let mut catalog = SkillEffectCatalog::default();
    let entrant = sonettobuf::FightEntityInfo {
        passive_skill: vec![2531, 2370],
        ..Default::default()
    };

    catalog.extend_entities_and_warn(config::configs::get(), [&entrant]);

    assert!(catalog.get(2531).is_some());
    assert!(catalog.get(2370).is_some());
    assert!(catalog.get(31270148).is_none());
}

#[test]
fn fight_catalog_follows_heat_scale_cast_skills() {
    init_config();

    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [], [31340021]);

    assert!(catalog.get(31345153).is_some());
}

#[test]
fn fight_catalog_follows_master_halo_linked_passives() {
    init_config();

    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31260143], []);

    assert!(catalog.get(31260183).is_some());
}

#[test]
fn fight_catalog_follows_paper_circle_continuation_skill() {
    init_config();

    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [], [31050131]);

    assert!(catalog.get(31050151).is_some());
}

#[test]
fn fight_catalog_follows_buff_act_linked_skill_aliases() {
    init_config();

    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [], [31260131]);

    assert!(catalog.get(31260171).is_some());
    assert_eq!(catalog.damage_rate(31260171), 1300);

    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [], [31200184]);
    assert!(catalog.get(31200192).is_some());
}

#[test]
fn fight_catalog_follows_shield_counter_skill() {
    init_config();

    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [], [30940181]);

    assert!(catalog.get(30940171).is_some());
}

#[test]
fn scoped_catalog_follows_configured_reinforced_skill_effects() {
    init_config();

    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [30860143], []);

    assert_eq!(catalog.reinforced_skill(30860143), Some(30861143));
    assert!(catalog.get(30861143).is_some());
}

#[test]
fn parses_hash_cells_as_opcode_and_args() {
    assert_eq!(parse_i32_list("60002#1#2"), vec![60002, 1, 2]);
    assert_eq!(parse_i32_list(""), Vec::<i32>::new());
}

#[test]
fn parses_target_code_and_raw_args() {
    let target = parse_target("103#7");

    assert_eq!(target.code, 103);
    assert_eq!(target.raw, vec![7]);
}

#[test]
fn unsupported_behavior_issue_keeps_exact_config_identity() {
    init_config();

    let issue = rule_issue(config::configs::get(), 99, 3, "60116#1#1#1");

    assert_eq!(issue.effect_id, 99);
    assert_eq!(issue.slot, 3);
    assert_eq!(issue.opcode, Some(60116));
    assert_eq!(issue.type_name.as_deref(), Some("CardDeckTopRankCorrect"));
    assert_eq!(issue.reason, RuleIssueReason::UnsupportedBehavior);
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

    assert!(parse("60225#530000751:100&530000752:25").is_some());
    assert!(parse("60225#530000751:100&bad").is_none());
    assert!(parse("60225#-530000751:100").is_none());
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
fn active_skill_filters_share_the_exact_skill_action_driver() {
    init_config();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());

    let subscriptions = catalog.compiled_subscriptions(435011).unwrap();
    assert!(
        subscriptions.contains(&SubscriptionKey::at_phase_and_publication(
            crate::engine::event::kind::EventKind::SkillAction,
            crate::engine::skill::rule::DefinitionKey::new(34210, "UseSkillEffectTag"),
            Some(crate::engine::skill::action::SkillPhase::HitPassives),
            crate::engine::event::subscription::PublicationPhase::BeforePublish,
        ))
    );
    assert!(!subscriptions.iter().any(|subscription| {
        subscription.event == crate::engine::event::kind::EventKind::SkillAction
            && subscription.definition.opcode == 16210
    }));
}

#[test]
fn target_count_and_effect_tag_share_the_effect_tag_driver() {
    init_config();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let subscriptions = catalog.compiled_subscriptions(432511).unwrap();

    assert!(subscriptions.iter().any(|subscription| {
        subscription.definition
            == crate::engine::skill::rule::DefinitionKey::new(34210, "UseSkillEffectTag")
            && subscription.phase == Some(crate::engine::skill::action::SkillPhase::HitPassives)
    }));
    assert!(
        !subscriptions
            .iter()
            .any(|subscription| subscription.definition.opcode == 500210)
    );
}

#[test]
fn psychube_buff_category_gate_keeps_threshold_then_status_order() {
    init_config();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());

    assert_eq!(
        catalog.condition_kind(
            432715,
            crate::engine::skill::rule::DefinitionKey::new(511201, "HasTypeBuffIdsMoreThan",),
        ),
        Some(&ParsedConditionKind::BuffStatusCount {
            status_ids: vec![8],
            compare: crate::engine::skill::condition::ConditionCompare::GreaterThanOrEqual,
            threshold: 2,
        })
    );
    assert_eq!(
        catalog.condition_kind(
            432715,
            crate::engine::skill::rule::DefinitionKey::new(701201, "HasMasterHalo"),
        ),
        Some(&ParsedConditionKind::MasterHalo)
    );
}

#[test]
fn master_halo_immediate_gate_uses_the_skill_extra_type_driver() {
    init_config();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let effect = catalog.get(435811).unwrap();
    let slot = &effect.slots[1];

    assert!(slot.conditions.iter().any(|condition| {
        condition.opcode == 701203 && condition.kind == ParsedConditionKind::MasterHalo
    }));
    assert_eq!(
        slot.compiled_subscriptions().unwrap(),
        vec![SubscriptionKey::at_phase(
            crate::engine::event::kind::EventKind::SkillAction,
            crate::engine::skill::rule::DefinitionKey::new(403203, "SkillExtraType"),
            Some(crate::engine::skill::action::SkillPhase::Immediate),
        )]
    );
}

#[test]
fn from_the_depths_keeps_its_once_per_battle_limit() {
    init_config();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());

    assert_eq!(
        catalog.limit_for_condition(
            433011,
            crate::engine::skill::rule::DefinitionKey::new(629210, "TeammateInjuryCount",),
        ),
        1
    );
    assert_eq!(
        catalog.limit_for_condition(
            433011,
            crate::engine::skill::rule::DefinitionKey::new(630212, "TeammateInjuryCountNotReset",),
        ),
        0
    );
}

#[test]
fn from_game_db_keeps_skill_effect_logic_target() {
    init_config();

    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());

    assert_eq!(catalog.logic_target(31140151), 112);
}

#[test]
fn from_game_db_resolves_behavior_target_999_like_lua() {
    init_config();

    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let effect = catalog.get(530000421).unwrap();

    assert_eq!(effect.slots[0].target.code, 202);
}

#[test]
fn behavior_target_999_keeps_per_target_conditions_when_condition_uses_logic_target() {
    init_config();

    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let effect = catalog.get(30980121).unwrap();

    assert_eq!(effect.slots[1].condition_target.code, 0);
    assert_eq!(effect.slots[1].target.code, 202);
    assert!(effect.slots[1].target_from_condition);
}

#[test]
fn from_game_db_compiles_lifecycle_conditions_to_setup_stages() {
    init_config();

    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());

    let enter = catalog.get(31140142).unwrap();
    assert!(
        !enter.slots[0]
            .setup_keys(crate::engine::skill::rule::SetupStage::EnterFight, 0)
            .is_empty()
    );

    let battle_start = catalog.get(31200146).unwrap();
    assert!(
        !battle_start.slots[0]
            .setup_keys(crate::engine::skill::rule::SetupStage::BattleStart, 0)
            .is_empty()
    );

    let career_static = catalog.get(434511).unwrap();
    assert!(
        !career_static.slots[1]
            .setup_keys(crate::engine::skill::rule::SetupStage::EnterFight, 0)
            .is_empty()
    );

    let per_career_static = catalog.get(31280143).unwrap();
    assert!(
        !per_career_static.slots[1]
            .setup_keys(crate::engine::skill::rule::SetupStage::EnterFight, 0)
            .is_empty()
    );
}

#[test]
fn active_and_passive_use_skill_type_not_damage_type() {
    init_config();

    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());

    assert!(catalog.is_active(30610111));
    assert!(!catalog.is_passive(30610111));
    assert!(catalog.is_passive(30610141));
    assert_eq!(catalog.skill_type(30950127), 2);
}
