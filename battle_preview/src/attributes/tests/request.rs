use super::*;

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
