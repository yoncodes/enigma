use super::*;

#[test]
fn cloth_input_discovery_returns_same_round_requests_in_capture_order() {
    let directory = std::env::temp_dir().join(format!(
        "enigma-cloth-inputs-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    for name in [
        "UseClothSkillRequest_3_20260804_192249_251.json",
        "UseClothSkillRequest_3.json",
        "UseClothSkillRequest_30.json",
        "UseClothSkillReply_3.json",
    ] {
        fs::write(directory.join(name), "{}").unwrap();
    }

    let paths = cloth_input_paths(&directory, 3).unwrap();

    assert_eq!(
        paths
            .iter()
            .filter_map(|path| path.file_name()?.to_str())
            .collect::<Vec<_>>(),
        vec![
            "UseClothSkillRequest_3.json",
            "UseClothSkillRequest_3_20260804_192249_251.json"
        ]
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn captured_twins_selection_has_a_committed_runtime_source() {
    init_config().unwrap();
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/battles/battle116385108/BeginRoundReply_1.json");
    let value = captured_start_reply(&path).unwrap();
    let fight: Fight = serde_json::from_value(value["fight"].clone()).unwrap();
    let (ex_attributes, sp_attributes) = preview_attributes(&fight, &path).unwrap();
    let mut runtime = BattleRuntime::new_with_attributes(fight, ex_attributes, sp_attributes);
    runtime.start_round().unwrap();
    let captured = captured_round(&path).unwrap();
    seed_captured_randomness(&mut runtime, &captured);
    let request = begin_round_request(&path.with_file_name("BeginRoundRequest_1.json")).unwrap();
    let round = runtime.advance_round(request).unwrap();
    let conduit = round
        .fight_step
        .iter()
        .find(|step| {
            step.act_effect.iter().any(|effect| {
                effect
                    .fight_step
                    .as_ref()
                    .is_some_and(|nested| nested.act_id == Some(31490121))
            })
        })
        .unwrap();
    assert_eq!(conduit.to_id, Some(-2));
    assert_eq!(
        conduit
            .act_effect
            .iter()
            .filter_map(|effect| effect.effect_type)
            .collect::<Vec<_>>(),
        vec![
            sonettobuf::effect_type_enum::EffectType::Devicerunning as i32,
            sonettobuf::effect_type_enum::EffectType::Devicepowerchange as i32,
            sonettobuf::effect_type_enum::EffectType::Buffupdate as i32,
            sonettobuf::effect_type_enum::EffectType::Counterchange as i32,
            sonettobuf::effect_type_enum::EffectType::Fightstep as i32,
        ]
    );
    let skill = conduit
        .act_effect
        .iter()
        .find_map(|effect| effect.fight_step.as_ref())
        .filter(|step| step.act_id == Some(31490121))
        .unwrap();
    let finish = skill
        .act_effect
        .iter()
        .position(|effect| {
            effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Counterchange as i32)
                && effect.effect_num == Some(63)
        })
        .unwrap();
    let harmonization = skill
        .act_effect
        .iter()
        .enumerate()
        .filter(|(_, effect)| {
            effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Expointchange as i32)
                && effect.effect_num == Some(1)
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    assert_eq!(harmonization.len(), 3);
    assert!(harmonization.into_iter().all(|index| index < finish));
    assert!(!round.fight_step.iter().any(|step| {
        step.act_effect.iter().any(|effect| {
            effect
                .fight_step
                .as_ref()
                .is_some_and(|nested| nested.act_id == Some(31490191))
        })
    }));
}

#[test]
fn captured_version7_conduit_sentinel_keeps_activation_sequence() {
    fn contains_act(step: &FightStep, act_id: i32) -> bool {
        step.act_id == Some(act_id)
            || step
                .act_effect
                .iter()
                .filter_map(|effect| effect.fight_step.as_ref())
                .any(|nested| contains_act(nested, act_id))
    }
    fn parent_of(step: &FightStep, act_id: i32) -> Option<&FightStep> {
        if step.act_effect.iter().any(|effect| {
            effect
                .fight_step
                .as_ref()
                .is_some_and(|nested| nested.act_id == Some(act_id))
        }) {
            return Some(step);
        }
        step.act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
            .find_map(|nested| parent_of(nested, act_id))
    }

    init_config().unwrap();
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/battles/battle72/BeginRoundReply_1.json");
    let generated = generate_reply(&path).unwrap().0.round.unwrap();
    let captured = captured_round(&path).unwrap();

    assert!(
        captured
            .fight_step
            .iter()
            .any(|step| contains_act(step, 31490121))
    );
    let signature = |round: &FightRound| {
        let parent = round
            .fight_step
            .iter()
            .find_map(|step| parent_of(step, 31490121))
            .unwrap();
        let nested = parent
            .act_effect
            .iter()
            .find_map(|effect| effect.fight_step.as_ref())
            .filter(|step| step.act_id == Some(31490121))
            .unwrap();
        (
            parent
                .act_effect
                .iter()
                .map(|effect| {
                    (
                        effect.effect_type,
                        effect.effect_num,
                        effect.reserve_str.clone(),
                    )
                })
                .collect::<Vec<_>>(),
            nested
                .act_effect
                .iter()
                .map(|effect| (effect.effect_type, effect.effect_num))
                .filter(|effect| {
                    *effect
                        == (
                            Some(sonettobuf::effect_type_enum::EffectType::Expointchange as i32),
                            Some(1),
                        )
                        || *effect
                            == (
                                Some(
                                    sonettobuf::effect_type_enum::EffectType::Counterchange as i32,
                                ),
                                Some(63),
                            )
                })
                .collect::<Vec<_>>(),
        )
    };

    let captured = signature(&captured);
    assert_eq!(signature(&generated), captured);
    assert_eq!(
        captured.1,
        vec![
            (
                Some(sonettobuf::effect_type_enum::EffectType::Expointchange as i32),
                Some(1),
            ),
            (
                Some(sonettobuf::effect_type_enum::EffectType::Expointchange as i32),
                Some(1),
            ),
            (
                Some(sonettobuf::effect_type_enum::EffectType::Expointchange as i32),
                Some(1),
            ),
            (
                Some(sonettobuf::effect_type_enum::EffectType::Counterchange as i32),
                Some(63),
            ),
        ]
    );
}

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
