use super::*;
use sonettobuf::{EquipRecord, FightTeam, TalentCubeInfo};

fn test_directory(label: &str) -> std::path::PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "enigma-preview-attributes-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    directory
}

fn fight(uid: i64) -> Fight {
    Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(uid),
                model_id: Some(3149),
                level: Some(60),
                equip_uid: Some(100),
                equips: vec![
                    EquipRecord {
                        equip_uid: Some(100),
                        equip_id: Some(1571),
                        equip_lv: Some(60),
                        ..Default::default()
                    },
                    EquipRecord {
                        equip_uid: Some(200),
                        equip_id: Some(1572),
                        equip_lv: Some(60),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn hero(uid: i64) -> HeroInfo {
    HeroInfo {
        uid,
        hero_id: 3149,
        level: Some(60),
        rank: Some(5),
        talent: Some(10),
        talent_cube_infos: vec![TalentCubeInfo {
            cube_id: Some(61),
            ..Default::default()
        }],
        default_equip_uid: Some(100),
        ex_attr: Some(HeroExAttribute {
            cri: Some(i32::MAX),
            ..Default::default()
        }),
        sp_attr: Some(HeroSpAttribute {
            heal: Some(i32::MAX),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn write_roster(directory: &Path, hero: HeroInfo) {
    fs::write(
        directory.join("HeroInfoListReply_1.json"),
        serde_json::to_vec(&HeroInfoListReply {
            heros: vec![hero],
            ..Default::default()
        })
        .unwrap(),
    )
    .unwrap();
}

fn write_request(directory: &Path, equips: Vec<sonettobuf::FightEquip>, is_balance: bool) {
    fs::write(
        directory.join("StartDungeonRequest.json"),
        serde_json::to_vec(&sonettobuf::StartDungeonRequest {
            fight_group: Some(sonettobuf::FightGroup {
                equips,
                ..Default::default()
            }),
            is_balance: Some(is_balance),
            ..Default::default()
        })
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn captured_derived_attributes_are_not_runtime_inputs() {
    crate::init_test_config();
    let uid = 42;
    let directory = test_directory("derived-poison");
    let hero = hero(uid);
    let fight = fight(uid);
    write_roster(&directory, hero.clone());
    let entity = &fight.attacker.as_ref().unwrap().entitys[0];
    let build = preview_build_input(entity, &hero).unwrap();
    let equips = validated_equipment_loadout(entity, &hero, None, None).unwrap();
    let expected = Stats::build_for_loadout(&build, &equips);

    let (ex, sp) = preview_attributes(&fight, &directory.join("BeginRoundReply_1.json")).unwrap();

    assert_eq!(ex, vec![(uid, expected.ex())]);
    assert_eq!(sp, vec![(uid, expected.sp())]);
    assert_ne!(expected.cri, i32::MAX);
    assert_ne!(expected.heal, i32::MAX);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn explicit_request_equipment_overrides_the_roster_default() {
    crate::init_test_config();
    let uid = 42;
    let directory = test_directory("requested-equipment");
    let mut hero = hero(uid);
    hero.default_equip_uid = Some(999);
    write_roster(&directory, hero);
    write_request(
        &directory,
        vec![sonettobuf::FightEquip {
            hero_uid: Some(uid),
            equip_uid: vec![100],
            ..Default::default()
        }],
        false,
    );

    assert!(preview_attributes(&fight(uid), &directory.join("BeginRoundReply_1.json")).is_ok());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn request_and_entity_equipment_mismatch_fails_loudly() {
    crate::init_test_config();
    let uid = 42;
    let directory = test_directory("request-mismatch");
    write_roster(&directory, hero(uid));
    write_request(
        &directory,
        vec![sonettobuf::FightEquip {
            hero_uid: Some(uid),
            equip_uid: vec![999],
            ..Default::default()
        }],
        false,
    );

    assert!(
        preview_attributes(&fight(uid), &directory.join("BeginRoundReply_1.json"))
            .unwrap_err()
            .to_string()
            .contains("invalid equipment metadata")
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn request_without_equipment_uses_the_roster_default() {
    crate::init_test_config();
    let uid = 42;
    let directory = test_directory("request-default");
    write_roster(&directory, hero(uid));
    write_request(&directory, Vec::new(), false);

    assert!(preview_attributes(&fight(uid), &directory.join("BeginRoundReply_1.json")).is_ok());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn negative_equipment_metadata_fails_loudly() {
    crate::init_test_config();
    let uid = 42;
    let mut fight = fight(uid);
    let entity = &mut fight.attacker.as_mut().unwrap().entitys[0];
    entity.equips.clear();
    entity.equip_uid = Some(-1);
    let mut hero = hero(uid);
    hero.default_equip_uid = Some(-1);

    assert!(validated_equipment_loadout(entity, &hero, None, None).is_err());
}

#[test]
fn captured_equipment_breakthrough_is_preserved() {
    crate::init_test_config();
    let uid = 42;
    let mut fight = fight(uid);
    let entity = &mut fight.attacker.as_mut().unwrap().entitys[0];
    entity.equips[0].break_lv = Some(3);
    entity.equips[1].break_lv = Some(2);

    let equips = validated_equipment_loadout(entity, &hero(uid), None, None).unwrap();

    assert_eq!(equips[0].break_level, 3);
    assert_eq!(equips[1].break_level, 2);
}

#[test]
fn invalid_captured_equipment_breakthrough_fails_loudly() {
    crate::init_test_config();
    let uid = 42;
    let mut fight = fight(uid);
    fight.attacker.as_mut().unwrap().entitys[0].equips[0].break_lv = Some(4);
    let entity = &fight.attacker.as_ref().unwrap().entitys[0];

    assert!(validated_equipment_loadout(entity, &hero(uid), None, None).is_err());
}

#[test]
fn preview_preserves_explicitly_empty_talent_layout() {
    crate::init_test_config();
    let uid = 42;
    let directory = test_directory("empty-talent-layout");
    let mut hero = hero(uid);
    hero.talent_cube_infos.clear();
    let fight = fight(uid);
    write_roster(&directory, hero.clone());
    let entity = &fight.attacker.as_ref().unwrap().entitys[0];
    let build = preview_build_input(entity, &hero).unwrap();
    let equips = validated_equipment_loadout(entity, &hero, None, None).unwrap();
    let captured = Stats::build_for_captured_loadout(&build, &equips);
    let inferred = Stats::build_for_loadout(&build, &equips);

    assert_eq!(
        preview_attributes(&fight, &directory.join("BeginRoundReply_1.json")).unwrap(),
        (vec![(uid, captured.ex())], vec![(uid, captured.sp())]),
    );
    assert_ne!(captured, inferred);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn string_encoded_request_equipment_is_accepted() {
    crate::init_test_config();
    let uid = 42;
    let directory = test_directory("string-request-equipment");
    let mut hero = hero(uid);
    hero.default_equip_uid = Some(999);
    write_roster(&directory, hero);
    fs::write(
        directory.join("StartDungeonRequest.json"),
        serde_json::to_vec(&serde_json::json!({
            "fightGroup": {
                "equips": [{
                    "heroUid": uid.to_string(),
                    "equipUid": ["100"]
                }]
            }
        }))
        .unwrap(),
    )
    .unwrap();

    assert!(preview_attributes(&fight(uid), &directory.join("BeginRoundReply_1.json")).is_ok());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn snake_case_balance_metadata_is_preserved() {
    let directory = test_directory("snake-balance");
    fs::write(
        directory.join("StartDungeonRequest.json"),
        serde_json::to_vec(&serde_json::json!({ "is_balance": true })).unwrap(),
    )
    .unwrap();

    assert!(
        battle_request_metadata(&directory.join("BeginRoundReply_1.json"))
            .unwrap()
            .is_balance
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn activity_request_metadata_unwraps_the_dungeon_request() {
    let directory = test_directory("activity-request");
    fs::write(
        directory.join("StartAct229BattleRequest.json"),
        serde_json::to_vec(&serde_json::json!({
            "startDungeonRequest": {
                "isBalance": true,
                "fightGroup": {
                    "equips": [{ "heroUid": "42", "equipUid": ["100"] }]
                }
            },
            "activityId": 1,
            "stageId": 1
        }))
        .unwrap(),
    )
    .unwrap();

    let metadata = battle_request_metadata(&directory.join("BeginRoundReply_1.json")).unwrap();

    assert!(metadata.is_balance);
    assert_eq!(metadata.selected_equips, HashMap::from([(42, 100)]));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn malformed_activity_request_wrapper_fails_loudly() {
    for (label, request) in [
        ("missing", serde_json::json!({ "activityId": 1 })),
        ("null", serde_json::json!({ "startDungeonRequest": null })),
        (
            "wrong-type",
            serde_json::json!({ "startDungeonRequest": "invalid" }),
        ),
    ] {
        let directory = test_directory(label);
        let path = directory.join("StartAct229BattleRequest.json");
        fs::write(&path, serde_json::to_vec(&request).unwrap()).unwrap();

        assert!(
            request_metadata(&path)
                .unwrap_err()
                .to_string()
                .contains("invalid activity startDungeonRequest")
        );
        fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn negative_request_equipment_fails_loudly() {
    let directory = test_directory("negative-request-equipment");
    fs::write(
        directory.join("StartDungeonRequest.json"),
        serde_json::to_vec(&serde_json::json!({
            "fightGroup": {
                "equips": [{ "heroUid": 42, "equipUid": [-1] }]
            }
        }))
        .unwrap(),
    )
    .unwrap();

    assert!(
        battle_request_metadata(&directory.join("BeginRoundReply_1.json"))
            .unwrap_err()
            .to_string()
            .contains("negative equipment uid")
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn zero_owner_equipment_rows_are_validated_then_ignored() {
    let directory = test_directory("zero-owner-request");
    fs::write(
        directory.join("StartDungeonRequest.json"),
        serde_json::to_vec(&serde_json::json!({
            "fightGroup": {
                "equips": [
                    { "heroUid": 0, "equipUid": [100] },
                    { "heroUid": 0, "equipUid": [0] },
                    { "heroUid": 42, "equipUid": [200] }
                ]
            }
        }))
        .unwrap(),
    )
    .unwrap();

    assert_eq!(
        battle_request_metadata(&directory.join("BeginRoundReply_1.json"))
            .unwrap()
            .selected_equips,
        HashMap::from([(42, 200)])
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn invalid_zero_owner_equipment_stays_fail_loud() {
    for (label, equip_uid) in [
        ("negative", serde_json::json!([-1])),
        ("malformed", serde_json::json!(["invalid"])),
    ] {
        let directory = test_directory(label);
        fs::write(
            directory.join("StartDungeonRequest.json"),
            serde_json::to_vec(&serde_json::json!({
                "fightGroup": {
                    "equips": [{ "heroUid": 0, "equipUid": equip_uid }]
                }
            }))
            .unwrap(),
        )
        .unwrap();

        assert!(battle_request_metadata(&directory.join("BeginRoundReply_1.json")).is_err());
        fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn duplicate_empty_and_selected_request_rows_fail_loudly() {
    let directory = test_directory("mixed-duplicate-request");
    fs::write(
        directory.join("StartDungeonRequest.json"),
        serde_json::to_vec(&serde_json::json!({
            "fightGroup": {
                "equips": [
                    { "heroUid": 42, "equipUid": [] },
                    { "heroUid": 42, "equipUid": [100] }
                ]
            }
        }))
        .unwrap(),
    )
    .unwrap();

    assert!(
        battle_request_metadata(&directory.join("BeginRoundReply_1.json"))
            .unwrap_err()
            .to_string()
            .contains("duplicate equipment selection")
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn duplicate_request_equipment_rows_fail_loudly() {
    crate::init_test_config();
    let uid = 42;
    let directory = test_directory("duplicate-request");
    write_roster(&directory, hero(uid));
    let selection = sonettobuf::FightEquip {
        hero_uid: Some(uid),
        equip_uid: vec![100],
        ..Default::default()
    };
    write_request(&directory, vec![selection.clone(), selection], false);

    assert!(
        preview_attributes(&fight(uid), &directory.join("BeginRoundReply_1.json"))
            .unwrap_err()
            .to_string()
            .contains("duplicate equipment selection")
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn missing_build_metadata_fails_loudly() {
    crate::init_test_config();
    let directory = test_directory("missing-build");
    let error = preview_attributes(&fight(42), &directory.join("BeginRoundReply_1.json"))
        .unwrap_err()
        .to_string();

    assert!(error.contains("missing build metadata"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn missing_talent_does_not_guess_a_default() {
    crate::init_test_config();
    let mut hero = hero(42);
    hero.talent = None;

    assert!(
        preview_build_input(&fight(42).attacker.unwrap().entitys[0], &hero)
            .unwrap_err()
            .to_string()
            .contains("missing talent")
    );
}

#[test]
fn missing_rank_does_not_infer_from_level() {
    crate::init_test_config();
    let mut hero = hero(42);
    hero.rank = None;

    assert!(
        preview_build_input(&fight(42).attacker.unwrap().entitys[0], &hero)
            .unwrap_err()
            .to_string()
            .contains("missing rank")
    );
}

#[test]
fn invalid_positive_trial_id_fails_loudly() {
    crate::init_test_config();
    let directory = test_directory("invalid-trial");
    let mut fight = fight(42);
    fight.attacker.as_mut().unwrap().entitys[0].trial_id = Some(i32::MAX);

    assert!(
        preview_attributes(&fight, &directory.join("BeginRoundReply_1.json"))
            .unwrap_err()
            .to_string()
            .contains("invalid trial")
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn battle_balance_applies_to_the_full_equipment_loadout() {
    crate::init_test_config();
    let uid = 42;
    let directory = test_directory("balanced-loadout");
    let hero = hero(uid);
    write_roster(&directory, hero.clone());
    write_request(&directory, Vec::new(), true);
    let mut fight = fight(uid);
    fight.battle_id = Some(116385108);
    for equip in &mut fight.attacker.as_mut().unwrap().entitys[0].equips {
        equip.equip_lv = Some(50);
    }
    let entity = &fight.attacker.as_ref().unwrap().entitys[0];
    let build = preview_build_input(entity, &hero).unwrap();
    let equips = validated_equipment_loadout(entity, &hero, None, None).unwrap();
    let balance = request_battle_balance(&fight, true).unwrap().unwrap();
    let expected = balance.stats_for(&build, &equips);
    let unbalanced = Stats::build_for_loadout(&build, &equips);

    assert_eq!(
        preview_attributes(&fight, &directory.join("BeginRoundReply_1.json")).unwrap(),
        (vec![(uid, expected.ex())], vec![(uid, expected.sp())]),
    );
    assert_ne!(expected, unbalanced);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn loadout_diffs_report_fields_that_differ_from_the_rebuilt_entity() {
    crate::init_test_config();
    let uid = 42;
    let directory = test_directory("loadout");
    let hero = hero(uid);
    write_roster(&directory, hero.clone());
    let mut fight = fight(uid);
    let entity = &mut fight.attacker.as_mut().unwrap().entitys[0];
    let built = EntityBuilder::new(preview_build_input(entity, &hero).unwrap(), 0, 0, false)
        .build()
        .unwrap();
    entity.skill_group1 = built.skill_group1.clone();
    entity.skill_group2 = built.skill_group2.clone();
    entity.ex_skill = built.ex_skill;
    entity.ex_point_type = built.ex_point_type;
    let path = directory.join("StartDungeonReply.json");

    assert!(loadout_diffs(&fight, &path).unwrap().is_empty());

    fight.attacker.as_mut().unwrap().entitys[0].ex_skill = Some(1);
    let diffs = loadout_diffs(&fight, &path).unwrap();
    assert_eq!(diffs.len(), 1);
    assert!(diffs[0].contains("exSkill"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn configured_trial_uses_configured_attributes_without_roster_metadata() {
    crate::init_test_config();
    let uid = 42;
    let trial_id = 116385001;
    let mut fight = fight(uid);
    let entity = &mut fight.attacker.as_mut().unwrap().entitys[0];
    entity.trial_id = Some(trial_id);
    entity.equips.clear();
    entity.equip_uid = None;
    let directory = test_directory("trial");
    let (_, expected) = EntityBuilder::trial(trial_id, uid, 0, 0).unwrap();

    assert_eq!(
        preview_attributes(&fight, &directory.join("BeginRoundReply_1.json")).unwrap(),
        (vec![(uid, expected.ex())], vec![(uid, expected.sp())]),
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn sorted_updates_override_roster_build_metadata() {
    crate::init_test_config();
    let uid = 42;
    let directory = test_directory("precedence");
    write_roster(&directory, hero(uid));
    let mut earlier = hero(uid);
    earlier.talent = Some(8);
    let mut later = hero(uid);
    later.talent = Some(12);
    fs::write(
        directory.join("HeroUpdatePush_2.json"),
        serde_json::to_vec(&HeroUpdatePush {
            hero_updates: vec![later.clone()],
        })
        .unwrap(),
    )
    .unwrap();
    fs::write(
        directory.join("HeroUpdatePush_1.json"),
        serde_json::to_vec(&HeroUpdatePush {
            hero_updates: vec![earlier],
        })
        .unwrap(),
    )
    .unwrap();

    let heroes = battle_build_metadata(&directory.join("BeginRoundReply_1.json")).unwrap();

    assert_eq!(heroes[&uid].talent, later.talent);
    fs::remove_dir_all(directory).unwrap();
}

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
