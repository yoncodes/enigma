use super::{
    BattleFighter, BattleRoster, BattleRosterPlan, configured_aid_ids, plan_roster,
    reserved_uid_offset, selected_support, validate_composition,
};
use crate::dungeon::FightOptions;
use crate::engine::entity::input::{EquipmentBuildInput, HeroBuildInput};

#[test]
fn selects_support_from_the_active_compose_plane() {
    assert_eq!(selected_support("30#40#50", 0), Some(30));
    assert_eq!(selected_support("31#41#51|32#42#52", 2), Some(32));
}

#[test]
fn explicit_roster_plan_matches_the_legacy_entry_point() {
    crate::test_support::init_config();
    let group = sonettobuf::FightGroup::default();
    let legacy = plan_roster(10101, 10101, false, &group, None).unwrap();
    let explicit = BattleRosterPlan::configured(
        crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
        10101,
        10101,
        false,
        &group,
        None,
    )
    .unwrap();

    assert_eq!(explicit.hero_uids(), legacy.hero_uids());
    assert_eq!(explicit.compose_support(), legacy.compose_support());
}

#[test]
fn validates_configured_team_slots_and_unique_hero_uids() {
    let valid = sonettobuf::FightGroup {
        hero_list: vec![10, 11, 12],
        sub_hero_list: vec![13],
        ..Default::default()
    };
    assert!(validate_composition(&valid, 4, 3, 0).is_ok());

    let placeholder_slots = sonettobuf::FightGroup {
        hero_list: vec![-1, -2],
        sub_hero_list: vec![0, 0],
        ..Default::default()
    };
    assert!(validate_composition(&placeholder_slots, 3, 2, 2).is_ok());

    let excessive_placeholders = sonettobuf::FightGroup {
        sub_hero_list: vec![0, 0, 0, 0],
        ..Default::default()
    };
    assert!(validate_composition(&excessive_placeholders, 3, 2, 0).is_err());

    let duplicate = sonettobuf::FightGroup {
        sub_hero_list: vec![12],
        ..valid.clone()
    };
    assert!(validate_composition(&duplicate, 4, 3, 0).is_err());

    let oversized_main = sonettobuf::FightGroup {
        hero_list: vec![10, 11, 12, 13],
        ..Default::default()
    };
    assert!(validate_composition(&oversized_main, 4, 3, 0).is_err());

    let oversized_reserve = sonettobuf::FightGroup {
        hero_list: vec![10, 11],
        sub_hero_list: vec![12, 13],
        ..Default::default()
    };
    assert!(validate_composition(&oversized_reserve, 3, 2, 0).is_err());

    let configured_aids = sonettobuf::FightGroup {
        hero_list: vec![-1, -2],
        ..Default::default()
    };
    assert!(validate_composition(&configured_aids, 3, 3, 2).is_ok());
    assert!(validate_composition(&configured_aids, 3, 3, 1).is_err());
    assert_eq!(
        configured_aid_ids("100102#100101").unwrap(),
        vec![100102, 100101]
    );
    assert!(configured_aid_ids("100102#bad#100101").is_err());
    assert!(reserved_uid_offset(&configured_aids, 0, true).is_err());
    assert_eq!(
        reserved_uid_offset(&sonettobuf::FightGroup::default(), 0, true).unwrap(),
        1
    );
    assert!(reserved_uid_offset(&sonettobuf::FightGroup::default(), 2, true).is_err());
}

#[test]
fn captured_tutorial_placeholders_build_the_configured_aid_team() {
    crate::test_support::init_config();
    let fight_group = sonettobuf::FightGroup {
        hero_list: vec![-1, -2],
        sub_hero_list: vec![0, 0],
        ..Default::default()
    };

    let built = super::super::build_fight(
        crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
        &BattleRoster {
            user_id: 1,
            player_level: 42,
            ..Default::default()
        },
        10103,
        11021,
        &fight_group,
        FightOptions::default(),
        None,
    )
    .unwrap();
    assert_eq!(built.player_level, 42);
    let attacker = built.fight.attacker.unwrap();

    assert_eq!(
        attacker
            .entitys
            .iter()
            .map(|entity| (entity.uid.unwrap(), entity.model_id.unwrap()))
            .collect::<Vec<_>>(),
        vec![(-1, 110205), (-2, 110204)]
    );
    assert!(attacker.sub_entitys.is_empty());
    assert!(
        attacker
            .entitys
            .iter()
            .all(|entity| entity.passive_skill.contains(&72_010))
    );
}

#[test]
fn configured_trial_hero_uses_its_loadout_and_reserved_position() {
    crate::test_support::init_config();
    let fight_group = sonettobuf::FightGroup {
        trial_hero_list: vec![sonettobuf::TrialHero {
            trial_id: Some(4301003),
            pos: Some(1),
            ..Default::default()
        }],
        ..Default::default()
    };

    let built = super::super::build_fight(
        crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
        &BattleRoster {
            user_id: 1,
            ..Default::default()
        },
        1102,
        201101,
        &fight_group,
        FightOptions::default(),
        None,
    )
    .unwrap();
    let attacker = built.fight.attacker.unwrap();
    let trial = &attacker.entitys[0];

    assert_eq!(
        (
            trial.uid,
            trial.model_id,
            trial.position,
            trial.trial_id,
            trial.trial_equip.as_ref().and_then(|equip| equip.equip_id),
        ),
        (Some(-1), Some(3126), Some(1), Some(4301003), Some(1553))
    );
    assert_eq!(
        trial.attr.as_ref().map(|attr| {
            (
                attr.hp,
                attr.attack,
                attr.defense,
                attr.mdefense,
                attr.technic,
            )
        }),
        Some((Some(6015), Some(735), Some(291), Some(296), Some(370)))
    );
    assert_eq!(
        trial.passive_skill,
        vec![31260141, 31260191, 435311, 530000151]
    );
    assert_eq!(trial.ex_point_max, Some(5));
    assert!(!trial.skill_group1.is_empty());
    assert!(!trial.passive_skill.is_empty());
}

#[test]
fn linked_trial_psychube_contributes_stats_and_passives() {
    crate::test_support::init_config();
    let (trial, stats) =
        crate::engine::entity::builder::EntityBuilder::trial(116385001, -1, 1, 1).unwrap();

    assert_eq!(
        (stats.hp, stats.atk, stats.def, stats.mdef),
        (14610, 2469, 1005, 1005)
    );
    assert!(trial.passive_skill.contains(&437115));
    assert!(trial.passive_skill.contains(&437215));
}

#[test]
fn configured_aids_build_as_allied_monsters_and_reserve_their_uids() {
    crate::test_support::init_config();
    let fight_group = sonettobuf::FightGroup::default();
    let roster = BattleRoster {
        user_id: 1,
        ..Default::default()
    };

    let built = super::super::build_fight(
        crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
        &roster,
        10002,
        1002,
        &fight_group,
        FightOptions::default(),
        None,
    )
    .unwrap();
    let mut runtime = crate::engine::runtime::BattleRuntime::new(
        crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
        built.fight.clone(),
    )
    .unwrap();
    let round = runtime.start_round().unwrap();
    let cards = runtime.card_info_push();
    let deal = round
        .team_a_cards1
        .iter()
        .map(|card| (card.uid.unwrap(), card.skill_id.unwrap()))
        .collect::<Vec<_>>();
    // Captured episode 10002: seven cards as dealt, composed to five.
    assert_eq!(
        deal,
        vec![
            (-1, 30250121),
            (-1, 30250121),
            (-1, 30250121),
            (-2, 30230111),
            (-2, 30230111),
            (-2, 30230121),
            (-1, 30250121),
        ]
    );
    assert_eq!(
        cards
            .deal_card_group
            .iter()
            .map(|card| (card.uid.unwrap(), card.skill_id.unwrap()))
            .collect::<Vec<_>>(),
        deal
    );
    let opening = cards
        .card_group
        .into_iter()
        .map(|card| (card.uid.unwrap(), card.skill_id.unwrap()))
        .collect::<Vec<_>>();
    assert_eq!(
        opening,
        vec![
            (-1, 30250122),
            (-1, 30250121),
            (-2, 30230112),
            (-2, 30230121),
            (-1, 30250121),
        ]
    );
    assert_eq!(opening[4], opening[1]);
    let attacker = built.fight.attacker.unwrap();
    let defender = built.fight.defender.unwrap();

    assert_eq!(
        attacker
            .entitys
            .iter()
            .map(|entity| (entity.uid, entity.model_id, entity.team_type))
            .collect::<Vec<_>>(),
        vec![
            (Some(-1), Some(100102), Some(1)),
            (Some(-2), Some(100101), Some(1)),
        ]
    );
    assert_eq!(
        defender
            .entitys
            .iter()
            .map(|entity| (entity.uid, entity.model_id))
            .collect::<Vec<_>>(),
        vec![(Some(-3), Some(100105)), (Some(-4), Some(100106))]
    );

    let built = super::super::build_fight(
        crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
        &roster,
        10003,
        1003,
        &fight_group,
        FightOptions::default(),
        None,
    )
    .unwrap();
    assert_eq!(
        built
            .fight
            .attacker
            .unwrap()
            .entitys
            .iter()
            .map(|entity| (entity.uid, entity.model_id))
            .collect::<Vec<_>>(),
        vec![(Some(-1), Some(100109))]
    );
    assert_eq!(built.fight.defender.unwrap().entitys[0].uid, Some(-2));
}

#[test]
fn unequipped_character_has_no_equip_record() {
    crate::test_support::init_config();
    let roster = BattleRoster {
        user_id: 1,
        fighters: vec![BattleFighter {
            hero: HeroBuildInput {
                uid: 20_000_001,
                user_id: 1,
                hero_id: 3028,
                skin: 302801,
                level: 1,
                ..Default::default()
            },
            equips: vec![],
        }],
        ..Default::default()
    };
    let group = sonettobuf::FightGroup {
        hero_list: vec![20_000_001],
        ..Default::default()
    };

    let built = super::super::build_fight(
        crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
        &roster,
        10101,
        10101,
        &group,
        FightOptions::default(),
        None,
    )
    .unwrap();
    let entity = &built.fight.attacker.unwrap().entitys[0];

    assert_eq!(entity.equip_uid, Some(0));
    assert!(entity.equips.is_empty());
}

#[test]
fn reserve_hero_uses_its_portrait_and_destiny_kit() {
    crate::test_support::init_config();
    let fighter = |uid, hero_id, skin| BattleFighter {
        hero: HeroBuildInput {
            uid,
            user_id: 1,
            hero_id,
            skin,
            level: 60,
            rank: 3,
            ex_skill_level: 3,
            destiny_stone: if hero_id == 3039 { 303902 } else { 0 },
            destiny_rank: if hero_id == 3039 { 4 } else { 0 },
            ..Default::default()
        },
        equips: vec![],
    };
    let roster = BattleRoster {
        user_id: 1,
        fighters: vec![
            fighter(20_000_001, 3028, 302801),
            fighter(20_000_002, 3039, 303901),
        ],
        ..Default::default()
    };
    let group = sonettobuf::FightGroup {
        hero_list: vec![20_000_001],
        sub_hero_list: vec![20_000_002],
        ..Default::default()
    };

    let built = super::super::build_fight(
        crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
        &roster,
        10101,
        10101,
        &group,
        FightOptions::default(),
        None,
    )
    .unwrap();
    let reserve = &built.fight.attacker.unwrap().sub_entitys[0];

    assert_eq!(reserve.skill_group1, vec![303901141, 303901151, 303901161]);
    assert_eq!(reserve.skill_group2, vec![303901271, 303901281, 303901291]);
}

#[test]
fn linked_psychube_is_loaded_with_its_refined_passive() {
    crate::test_support::init_config();
    assert_eq!(
        config::configs::get().linked_psychube_id(3149, 1571),
        Some(1572)
    );
    assert_eq!(config::configs::get().linked_psychube_id(3028, 1571), None);
    let hero_uid = 20_000_001;
    let primary_uid = 30_000_001;
    let linked_uid = 30_000_002;
    let roster = BattleRoster {
        user_id: 1,
        fighters: vec![BattleFighter {
            hero: HeroBuildInput {
                uid: hero_uid,
                user_id: 1,
                hero_id: 3149,
                skin: 314901,
                level: 1,
                ..Default::default()
            },
            equips: vec![
                EquipmentBuildInput {
                    uid: primary_uid,
                    equip_id: 1571,
                    level: 1,
                    break_level: 1,
                    refine_level: 1,
                },
                EquipmentBuildInput {
                    uid: linked_uid,
                    equip_id: 1572,
                    level: 1,
                    break_level: 1,
                    refine_level: 5,
                },
            ],
        }],
        ..Default::default()
    };
    let group = sonettobuf::FightGroup {
        hero_list: vec![hero_uid],
        ..Default::default()
    };

    let built = super::super::build_fight(
        crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
        &roster,
        10101,
        10101,
        &group,
        FightOptions::default(),
        None,
    )
    .unwrap();
    let entity = &built.fight.attacker.unwrap().entitys[0];

    assert_eq!(entity.equip_uid, Some(primary_uid));
    assert_eq!(
        entity
            .equips
            .iter()
            .map(|equip| (equip.equip_uid, equip.equip_id, equip.refine_lv))
            .collect::<Vec<_>>(),
        vec![
            (Some(primary_uid), Some(1571), Some(1)),
            (Some(linked_uid), Some(1572), Some(5)),
        ]
    );
    assert!(entity.passive_skill.contains(&437111));
    assert!(entity.passive_skill.contains(&437215));
}

#[test]
fn configured_balance_floors_only_the_battle_loadout() {
    crate::test_support::init_config();
    let hero_uid = 20_000_001;
    let equip_uid = 30_000_001;
    let hero = HeroBuildInput {
        uid: hero_uid,
        user_id: 1,
        hero_id: 3127,
        skin: 312701,
        level: 121,
        rank: 4,
        talent: 10,
        ..Default::default()
    };
    let roster = BattleRoster {
        user_id: 1,
        fighters: vec![BattleFighter {
            hero: hero.clone(),
            equips: vec![EquipmentBuildInput {
                uid: equip_uid,
                equip_id: 1502,
                level: 50,
                break_level: 1,
                refine_level: 1,
            }],
        }],
        ..Default::default()
    };
    let group = sonettobuf::FightGroup {
        hero_list: vec![hero_uid],
        equips: vec![sonettobuf::FightEquip {
            hero_uid: Some(hero_uid),
            equip_uid: vec![equip_uid],
            ..Default::default()
        }],
        trial_hero_list: vec![sonettobuf::TrialHero {
            trial_id: Some(116385001),
            pos: Some(1),
            ..Default::default()
        }],
        ..Default::default()
    };

    let built = super::super::build_fight(
        crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
        &roster,
        38510113,
        116385108,
        &group,
        FightOptions {
            is_balance: true,
            ..Default::default()
        },
        None,
    )
    .unwrap();
    let entity = built
        .fight
        .attacker
        .unwrap()
        .entitys
        .into_iter()
        .find(|entity| entity.uid == Some(hero_uid))
        .unwrap();

    assert_eq!(entity.level, Some(140));
    assert_eq!(
        entity.attr.map(|attr| {
            (
                attr.hp,
                attr.attack,
                attr.defense,
                attr.mdefense,
                attr.technic,
            )
        }),
        Some((Some(9628), Some(1737), Some(820), Some(858), Some(504)))
    );
    assert_eq!(entity.equips[0].equip_lv, Some(60));
    assert_eq!(roster.fighters[0].hero, hero);
}

#[test]
fn requested_balance_requires_valid_battle_config() {
    crate::test_support::init_config();

    let error = super::super::build_fight(
        crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
        &BattleRoster {
            user_id: 1,
            ..Default::default()
        },
        10101,
        10101,
        &sonettobuf::FightGroup::default(),
        FightOptions {
            is_balance: true,
            ..Default::default()
        },
        None,
    )
    .err()
    .unwrap();

    assert!(error.to_string().contains("invalid balance config"));
}

#[test]
fn compose_support_is_built_before_defender_uids() {
    crate::test_support::init_config();
    let hero_uid = 20_000_001;
    let support = config::configs::get()
        .tower_compose_support
        .get(306600)
        .unwrap();
    let hero = HeroBuildInput {
        uid: hero_uid,
        user_id: 2,
        hero_id: 3066,
        skin: 306601,
        level: 1,
        ex_skill_level: support.lv,
        ..Default::default()
    };
    let group = sonettobuf::FightGroup {
        params: Some(format!(
            r#"{{"heroList":["0","0","0","0"],"assistDataMap":[{{"0":{{"level":{},"skin":{},"exSkillLevel":{},"heroUid":{},"heroId":3066,"assistType":6}}}}],"buffParamsStr":"306600#0#0"}}"#,
            hero.level, hero.skin, hero.ex_skill_level, hero_uid
        )),
        ..Default::default()
    };
    let params = format!(
        r#"{{"supportAssistUid":{},"layerId":1,"supportId":306600,"supportAssistType":6,"themeId":1,"planeId":0}}"#,
        hero_uid
    );

    let built = super::super::build_fight(
        crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
        &BattleRoster {
            user_id: 1,
            compose_support: Some(hero),
            ..Default::default()
        },
        90400101,
        9001101,
        &group,
        FightOptions::default(),
        Some(params.as_str()),
    )
    .unwrap();
    let attacker = built.fight.attacker.unwrap();
    assert_eq!(attacker.assist_boss.unwrap().model_id, Some(306600));
    assert_eq!(built.fight.defender.unwrap().entitys[0].uid, Some(-2),);
}
