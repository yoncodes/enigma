use super::*;

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
