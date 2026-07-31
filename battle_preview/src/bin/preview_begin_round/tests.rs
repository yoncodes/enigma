use super::*;

#[test]
fn generated_round_uses_captured_rng_but_not_damage_amounts() {
    init_config().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/battles/battle71");
    let temporary = std::env::temp_dir().join(format!(
        "enigma-preview-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&temporary).unwrap();
    for name in [
        "StartDungeonReply.json",
        "BeginRoundRequest_1.json",
        "BeginRoundReply_1.json",
    ] {
        fs::copy(source.join(name), temporary.join(name)).unwrap();
    }

    let expected = replay_to_round(&source.join("BeginRoundReply_1.json")).unwrap();
    let reply_path = temporary.join("BeginRoundReply_1.json");
    let mut captured: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&reply_path).unwrap()).unwrap();
    expand_compressed_fight_steps(&mut captured).unwrap();
    let round = captured.get_mut("round").unwrap();
    round["nextRoundBeginStep"] = serde_json::json!([]);
    round["fightStep"][2]["actEffect"][0]["effectNum"] = serde_json::json!(999_999);
    round["fightStep"][2]["actEffect"][0]["hurtInfo"]["damage"] = serde_json::json!(999_999);
    fs::write(&reply_path, serde_json::to_vec(&captured).unwrap()).unwrap();

    let actual = replay_to_round(&reply_path).unwrap();
    captured.get_mut("round").unwrap()["teamACards2"] = serde_json::json!([]);
    fs::write(&reply_path, serde_json::to_vec(&captured).unwrap()).unwrap();
    let without_card_choices = replay_to_round(&reply_path).unwrap();
    fs::remove_dir_all(temporary).unwrap();

    assert_eq!(actual, expected);
    assert_ne!(without_card_choices, expected);
}

#[test]
fn reads_dungeon_and_tower_start_reply_envelopes() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/battles");

    let dungeon = captured_start_reply(&fixtures.join("battle69/BeginRoundReply_1.json"));
    let tower = captured_start_reply(&fixtures.join("battle74/BeginRoundReply_1.json"));

    assert!(dungeon.unwrap().get("fight").is_some());
    assert!(tower.unwrap().get("fight").is_some());
}
