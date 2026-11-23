use super::*;

#[test]
fn capture_timeline_supplies_missing_battle_roster() {
    let directory = test_directory("capture-timeline");
    let common = directory.join("decoded/common");
    let battle = directory.join("decoded/Dungeon/Battle1");
    fs::create_dir_all(&common).unwrap();
    fs::create_dir_all(&battle).unwrap();
    let uid = 42;
    fs::write(
        common.join("capture_000001_HeroInfoListReply.json"),
        serde_json::to_vec(&HeroInfoListReply {
            heros: vec![hero(uid)],
            ..Default::default()
        })
        .unwrap(),
    )
    .unwrap();
    fs::write(
        common.join("capture_000002_StartDungeonReply.json"),
        b"captured battle",
    )
    .unwrap();
    let battle_path = battle.join("StartDungeonReply.json");
    fs::write(&battle_path, b"captured battle").unwrap();
    fs::write(
        battle.join("HeroUpdatePush.json"),
        serde_json::to_vec(&HeroUpdatePush {
            hero_updates: vec![hero(99)],
        })
        .unwrap(),
    )
    .unwrap();

    assert_eq!(
        battle_build_metadata(&battle_path).unwrap()[&uid].hero_id,
        3149
    );
    assert_eq!(build_metadata_source(&battle_path).unwrap(), "timeline");
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn capture_timeline_supplies_selected_assist_build_metadata() {
    crate::init_test_config();
    let directory = test_directory("capture-assist");
    let common = directory.join("decoded/common");
    let battle = directory.join("decoded/Dungeon/Battle1");
    fs::create_dir_all(&common).unwrap();
    fs::create_dir_all(&battle).unwrap();
    fs::write(
        common.join("capture_000001_HeroInfoListReply.json"),
        serde_json::to_vec(&HeroInfoListReply {
            heros: vec![hero(42)],
            ..Default::default()
        })
        .unwrap(),
    )
    .unwrap();
    let selected_uid = 99;
    let unselected_uid = 100;
    let assist = |uid, user_id, style, cube_id| sonettobuf::AssistHeroInfo {
        hero_uid: Some(uid),
        user_id: Some(user_id),
        hero_id: Some(3146),
        level: Some(180),
        rank: Some(4),
        talent: Some(15),
        talent_cube_infos: vec![TalentCubeInfo {
            cube_id: Some(cube_id),
            ..Default::default()
        }],
        style: Some(style),
        ..Default::default()
    };
    let write_assist_roster = |name, assist_hero_infos| {
        fs::write(
            common.join(name),
            serde_json::to_vec(&sonettobuf::RefreshAssistReply {
                assist_hero_careers: vec![sonettobuf::AssistHeroCareerNo {
                    assist_hero_infos,
                    ..Default::default()
                }],
                ..Default::default()
            })
            .unwrap(),
        )
        .unwrap();
    };
    write_assist_roster(
        "capture_000002_RefreshAssistReply.json",
        vec![
            assist(selected_uid, 8, 1, 61),
            assist(selected_uid, 7, 2, 64),
            assist(unselected_uid, 8, 1, 61),
        ],
    );
    fs::write(
        common.join("capture_000004_StartDungeonReply.json"),
        b"captured battle",
    )
    .unwrap();
    let battle_path = battle.join("StartDungeonReply.json");
    fs::write(&battle_path, b"captured battle").unwrap();
    let request_path = battle.join("StartDungeonRequest.json");
    let write_assist_request = |hero_uid, user_id| {
        fs::write(
            &request_path,
            serde_json::to_vec(&sonettobuf::StartDungeonRequest {
                fight_group: Some(sonettobuf::FightGroup {
                    assist_hero_uid: Some(hero_uid),
                    assist_user_id: Some(user_id),
                    ..Default::default()
                }),
                is_balance: Some(false),
                ..Default::default()
            })
            .unwrap(),
        )
        .unwrap();
    };
    write_assist_request(selected_uid, 7);
    let mut fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(selected_uid),
                model_id: Some(3146),
                level: Some(180),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };

    assert_eq!(preview_attributes(&fight, &battle_path).unwrap().0.len(), 1);
    fight.attacker.as_mut().unwrap().entitys[0].uid = Some(unselected_uid);
    assert!(
        preview_attributes(&fight, &battle_path)
            .unwrap_err()
            .to_string()
            .contains("missing build metadata")
    );
    fight.attacker.as_mut().unwrap().entitys[0].uid = Some(selected_uid);
    write_assist_request(selected_uid, 9);
    assert!(
        preview_attributes(&fight, &battle_path)
            .unwrap_err()
            .to_string()
            .contains("user mismatch")
    );
    write_assist_request(selected_uid, 7);
    write_assist_roster(
        "capture_000003_RefreshAssistReply.json",
        vec![assist(unselected_uid, 8, 1, 61)],
    );
    assert!(
        preview_attributes(&fight, &battle_path)
            .unwrap_err()
            .to_string()
            .contains("missing from the captured assist roster")
    );
    write_assist_request(42, 7);
    fight.attacker.as_mut().unwrap().entitys[0].uid = Some(42);
    write_assist_roster(
        "capture_000003_RefreshAssistReply.json",
        vec![assist(42, 7, 2, 64)],
    );
    assert!(
        preview_attributes(&fight, &battle_path)
            .unwrap_err()
            .to_string()
            .contains("conflicts with owned roster")
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn capture_timeline_supplies_missing_fight_equipment_records() {
    crate::init_test_config();
    let directory = test_directory("capture-equipment");
    let common = directory.join("decoded/common");
    let battle = directory.join("decoded/Dungeon/Battle1");
    fs::create_dir_all(&common).unwrap();
    fs::create_dir_all(&battle).unwrap();
    fs::write(
        common.join("capture_000001_GetEquipInfoReply.json"),
        serde_json::to_vec(&GetEquipInfoReply {
            equips: vec![
                Equip {
                    equip_id: Some(1532),
                    uid: Some(100),
                    level: Some(60),
                    break_lv: Some(2),
                    refine_lv: Some(5),
                    ..Default::default()
                },
                Equip {
                    equip_id: Some(1201),
                    uid: Some(200),
                    level: Some(1),
                    ..Default::default()
                },
            ],
        })
        .unwrap(),
    )
    .unwrap();
    fs::write(
        common.join("capture_000002_EquipUpdatePush.json"),
        serde_json::to_vec(&EquipUpdatePush {
            equips: vec![Equip {
                equip_id: Some(1532),
                uid: Some(100),
                level: Some(60),
                break_lv: Some(3),
                refine_lv: Some(5),
                ..Default::default()
            }],
        })
        .unwrap(),
    )
    .unwrap();
    fs::write(
        common.join("capture_000003_EquipDeletePush.json"),
        serde_json::to_vec(&EquipDeletePush { uids: vec![200] }).unwrap(),
    )
    .unwrap();
    fs::write(
        common.join("capture_000004_StartDungeonReply.json"),
        b"captured battle",
    )
    .unwrap();
    let battle_path = battle.join("StartDungeonReply.json");
    fs::write(&battle_path, b"captured battle").unwrap();

    let inventory = captured_equipment_metadata(&battle_path).unwrap();
    let entity = FightEntityInfo {
        model_id: Some(3095),
        equip_uid: Some(100),
        ..Default::default()
    };
    let hero = HeroInfo {
        hero_id: 3095,
        default_equip_uid: Some(100),
        ..Default::default()
    };
    let equips = validated_equipment_loadout(&entity, &hero, None, inventory.get(&100)).unwrap();

    assert_eq!(equips.len(), 1);
    assert_eq!(equips[0].equip_id, 1532);
    assert_eq!(equips[0].break_level, 3);
    assert!(!inventory.contains_key(&200));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn unique_later_packet_resolves_repeated_battle_start() {
    for command in ["StartDungeonReply", "StartTowerBattleReply"] {
        let directory = test_directory(command);
        let common = directory.join("decoded/common");
        let battle = directory.join("decoded/Dungeon/Battle2");
        fs::create_dir_all(&common).unwrap();
        fs::create_dir_all(&battle).unwrap();
        let uid = 42;
        let mut updated = hero(uid);
        updated.talent = Some(12);
        fs::write(
            common.join("capture_000001_HeroInfoListReply.json"),
            serde_json::to_vec(&HeroInfoListReply {
                heros: vec![hero(uid)],
                ..Default::default()
            })
            .unwrap(),
        )
        .unwrap();
        for sequence in [2, 4] {
            fs::write(
                common.join(format!("capture_{sequence:06}_{command}.json")),
                b"repeated battle",
            )
            .unwrap();
        }
        fs::write(
            common.join("capture_000003_HeroUpdatePush.json"),
            serde_json::to_vec(&HeroUpdatePush {
                hero_updates: vec![updated.clone()],
            })
            .unwrap(),
        )
        .unwrap();
        fs::write(
            common.join("capture_000005_EndFightPush.json"),
            b"unique end",
        )
        .unwrap();
        let battle_path = battle.join(format!("{command}.json"));
        fs::write(&battle_path, b"repeated battle").unwrap();
        fs::write(battle.join("EndFightPush.json"), b"unique end").unwrap();

        assert_eq!(
            battle_build_metadata(&battle_path).unwrap()[&uid].talent,
            updated.talent
        );
        fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn repeated_battle_start_without_bounded_anchor_fails_loudly() {
    let directory = test_directory("ambiguous-start");
    let common = directory.join("decoded/common");
    let battle = directory.join("decoded/Dungeon/Battle2");
    fs::create_dir_all(&common).unwrap();
    fs::create_dir_all(&battle).unwrap();
    for sequence in [1, 2] {
        fs::write(
            common.join(format!("capture_{sequence:06}_StartDungeonReply.json")),
            b"repeated battle",
        )
        .unwrap();
    }
    let battle_path = battle.join("StartDungeonReply.json");
    fs::write(&battle_path, b"repeated battle").unwrap();

    assert!(
        battle_build_metadata(&battle_path)
            .unwrap_err()
            .to_string()
            .contains("multiple timeline positions")
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn unmatched_common_timeline_falls_back_to_local_build_metadata() {
    let directory = test_directory("unresolved-common");
    let common = directory.join("decoded/common");
    let battle = directory.join("decoded/Dungeon/Battle1");
    fs::create_dir_all(&common).unwrap();
    fs::create_dir_all(&battle).unwrap();
    fs::write(
        common.join("capture_000001_StartDungeonReply.json"),
        b"another battle",
    )
    .unwrap();
    let battle_path = battle.join("StartDungeonReply.json");
    fs::write(&battle_path, b"captured battle").unwrap();
    fs::write(
        battle.join("HeroUpdatePush.json"),
        serde_json::to_vec(&HeroUpdatePush {
            hero_updates: vec![hero(42)],
        })
        .unwrap(),
    )
    .unwrap();

    assert!(
        battle_build_metadata(&battle_path)
            .unwrap()
            .contains_key(&42)
    );
    assert_eq!(build_metadata_source(&battle_path).unwrap(), "local");
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn repeated_session_starts_use_the_rosters_before_their_own_position() {
    let directory = test_directory("repeated-session-start");
    let common = directory.join("decoded/common");
    fs::create_dir_all(&common).unwrap();
    let roster = |name: &str, uid| {
        fs::write(
            common.join(name),
            serde_json::to_vec(&HeroInfoListReply {
                heros: vec![hero(uid)],
                ..Default::default()
            })
            .unwrap(),
        )
        .unwrap();
    };
    roster("20260927_080000_000_000001_HeroInfoListReply.json", 42);
    let first = common.join("20260927_080100_000_000002_StartDungeonReply.json");
    fs::write(&first, b"same battle").unwrap();
    roster("20260927_080200_000_000003_HeroInfoListReply.json", 43);
    let second = common.join("20260927_080300_000_000004_StartDungeonReply.json");
    fs::write(&second, b"same battle").unwrap();

    assert!(battle_build_metadata(&first).unwrap().contains_key(&42));
    assert!(battle_build_metadata(&second).unwrap().contains_key(&43));
    assert_eq!(build_metadata_source(&second).unwrap(), "timeline");
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn session_roster_backs_battles_missing_from_the_timeline() {
    let directory = test_directory("session-roster");
    let common = directory.join("decoded/common");
    let battle = directory.join("decoded/Dungeon/Battle1");
    fs::create_dir_all(&common).unwrap();
    fs::create_dir_all(&battle).unwrap();
    fs::write(
        common.join("capture_000001_HeroInfoListReply.json"),
        serde_json::to_vec(&HeroInfoListReply {
            heros: vec![hero(42)],
            ..Default::default()
        })
        .unwrap(),
    )
    .unwrap();
    let battle_path = battle.join("StartDungeonReply.json");
    fs::write(&battle_path, b"captured battle").unwrap();

    assert!(
        battle_build_metadata(&battle_path)
            .unwrap()
            .contains_key(&42)
    );
    assert_eq!(build_metadata_source(&battle_path).unwrap(), "session");
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn session_battle_uses_only_builds_captured_before_its_start() {
    let directory = test_directory("session-battle-build");
    let common = directory.join("decoded/common");
    fs::create_dir_all(&common).unwrap();
    fs::write(
        common.join("20260927_080000_000_000001_HeroInfoListReply.json"),
        serde_json::to_vec(&HeroInfoListReply {
            heros: vec![hero(42)],
            ..Default::default()
        })
        .unwrap(),
    )
    .unwrap();
    let start = common.join("20260927_080100_000_000002_StartDungeonReply.json");
    fs::write(&start, b"captured battle").unwrap();
    fs::write(
        common.join("20260927_080200_000_000003_HeroUpdatePush.json"),
        serde_json::to_vec(&HeroUpdatePush {
            hero_updates: vec![hero(43)],
        })
        .unwrap(),
    )
    .unwrap();

    let heroes = battle_build_metadata(&start).unwrap();

    assert!(heroes.contains_key(&42));
    assert!(!heroes.contains_key(&43));
    assert_eq!(build_metadata_source(&start).unwrap(), "timeline");
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn snake_case_round_name_maps_to_capture_command() {
    assert_eq!(
        capture_command(Path::new("begin_round_2.json")).as_deref(),
        Some("BeginRoundReply")
    );
    assert_eq!(
        capture_command(Path::new("BeginRoundReply_2.json")).as_deref(),
        Some("BeginRoundReply")
    );
    assert_eq!(
        capture_command(Path::new(
            "20260927_080404_757_000410_StartDungeonReply.json"
        ))
        .as_deref(),
        Some("StartDungeonReply")
    );
    assert!(is_battle_start_packet(Path::new(
        "20260927_080404_757_000410_StartAct229BattleReply.json"
    )));
}
