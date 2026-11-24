use super::*;

#[test]
fn rouge2_side_skills_use_the_buff_inventory_at_battle_start() {
    crate::init_test_config();
    let db = config::configs::get();
    let directory = test_directory("rouge2-side-buffs");
    let common = directory.join("common");
    let numbered = directory.join("numbered");
    fs::create_dir(&common).unwrap();
    fs::create_dir(&numbered).unwrap();
    let write = |index: usize, command: &str, json: &str| {
        let path = common.join(format!("20260813_204552_000_{index:06}_{command}.json"));
        fs::write(&path, json).unwrap();
        path
    };
    write(
        1,
        "Rouge2InfoPush",
        r#"{"rouge2Info":{"state":2,"bagInfo":{"bags":[
            {"bagType":3,"items":[
                {"uid":300000001,"itemId":220026,"count":1},
                {"uid":300000002,"itemId":220033,"count":1}
            ]}
        ]}}}"#,
    );
    write(
        2,
        "Rouge2BagItemRemovePush",
        r#"{"items":[{"uid":300000002}]}"#,
    );
    let before = write(3, "StartDungeonReply", r#"{"fight":{"battleId":1}}"#);
    write(
        4,
        "Rouge2BagItemUpdatePush",
        r#"{"items":[{"uid":300000003,"itemId":220033,"count":1}]}"#,
    );
    let start = write(5, "StartDungeonReply", r#"{"fight":{"battleId":2}}"#);
    let extracted = numbered.join("StartDungeonReply.json");
    fs::copy(&start, &extracted).unwrap();
    write(6, "Rouge2EndReply", r#"{"rouge2Info":{"state":3}}"#);
    let after = write(7, "StartDungeonReply", r#"{"fight":{"battleId":3}}"#);

    let fight = Fight {
        custom_data: vec![sonettobuf::CustomData {
            r#type: Some(sonettobuf::custom_data::CustomDataType::Rouge2 as i32),
            ..Default::default()
        }],
        ..Default::default()
    };
    assert!(rouge2_side_skills(db, &fight, &before).unwrap().is_empty());
    for path in [start, extracted] {
        let skills = rouge2_side_skills(db, &fight, &path).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].skill_id, 8290407);
    }
    assert!(rouge2_side_skills(db, &fight, &after).unwrap().is_empty());
    fs::remove_dir_all(directory).unwrap();
}
