use super::*;

#[test]
fn status_skill_rate_counts_configured_statuses_up_to_its_limit() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![
                    BuffInfo {
                        uid: Some(1),
                        buff_id: Some(400401),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(2),
                        buff_id: Some(301),
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
    let capped = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(10002, "SkillRateUp1"),
        vec![1_000, 1],
        vec!["1000".into(), "1".into(), "1,5".into()],
    );
    let uncapped = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(10002, "SkillRateUp1"),
        vec![200, 99],
        vec!["200".into(), "99".into(), "1,5".into()],
    );

    assert_eq!(status_skill_rate(&managers.buff, 10, &capped), 1_000);
    assert_eq!(status_skill_rate(&managers.buff, 10, &uncapped), 400);

    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    assert!(emit(
        &mut modifiers,
        10,
        10,
        Some(&managers),
        30230134,
        0,
        &capped,
    ));
    assert_eq!(modifiers.rates[0].target_uid, 0);
}

#[test]
fn target_status_skill_rate_is_scoped_to_the_attacked_entity() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(301),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(10003, "SkillRateUp2"),
        vec![400, 1],
        vec!["400".into(), "1".into(), "2,4,6".into()],
    );
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = TargetContext {
        runtime_target_uid: -1,
        ..Default::default()
    };

    assert!(matches!(
        crate::engine::skill::behavior::rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 1163855041,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        ),
        Some(ops) if ops.is_empty()
    ));
    assert_eq!(modifiers.rates[0].fixed_value(), Some(400));
    assert_eq!(modifiers.rates[0].target_uid, -1);
}

#[test]
fn self_buff_type_rate_scales_by_the_configured_buff_amount() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(31490008),
                    layer: Some(3),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let behavior = ParsedBehavior::new(60182, "SkillRateUpBySelfBuffType", vec![31490008, 450]);
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();

    assert!(supports_self_buff_type_rate(&behavior));
    assert!(emit(
        &mut modifiers,
        10,
        10,
        Some(&managers),
        31490141,
        0,
        &behavior,
    ));
    assert_eq!(modifiers.rates[0].fixed_value(), Some(1_350));
    assert!(!supports_self_buff_type_rate(&ParsedBehavior::new(
        60182,
        "SkillRateUpBySelfBuffType",
        vec![31490008],
    )));
}

#[test]
fn card_rank_skill_rate_reads_the_selected_action_area_and_caps_the_total() {
    crate::test_support::init_config();
    let mut cards = crate::engine::manager::card::CardManager::new(vec![
        sonettobuf::CardInfo {
            uid: Some(10),
            skill_id: Some(30950111),
            ..Default::default()
        },
        sonettobuf::CardInfo {
            uid: Some(10),
            skill_id: Some(30950112),
            ..Default::default()
        },
        sonettobuf::CardInfo {
            uid: Some(10),
            skill_id: Some(30950113),
            ..Default::default()
        },
    ]);
    cards.play_card(0, None, None, None).unwrap();
    cards.play_card(0, None, None, None).unwrap();
    cards.play_card(0, None, None, None).unwrap();
    cards.queue_use_card(crate::engine::manager::card::QueuedUseCard {
        card_index: 4,
        card: sonettobuf::CardInfo {
            uid: Some(-1),
            skill_id: Some(370001002),
            ..Default::default()
        },
        team_type: 1,
        source_skill_id: 370001010,
        action: None,
    });
    let behavior = |cap| {
        ParsedBehavior::from_spec(
            crate::engine::skill::behavior::classify::BehaviorSpec::new(
                60067,
                "SkillRateUpCardLevel",
            ),
            vec![cap],
            vec!["2,3".into(), "300,500".into(), cap.to_string()],
        )
    };

    assert_eq!(card_rank_skill_rate(&cards, &behavior(2_500)), 1_300);
    assert_eq!(card_rank_skill_rate(&cards, &behavior(700)), 700);

    let mut managers = BattleManagers::default();
    managers.card = cards;
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    assert!(emit(
        &mut modifiers,
        10,
        10,
        Some(&managers),
        30950111,
        0,
        &behavior(2_500),
    ));
    assert_eq!(modifiers.rates[0].target_uid, 0);
}
