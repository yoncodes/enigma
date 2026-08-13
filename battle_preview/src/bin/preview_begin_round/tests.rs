use super::*;

#[test]
fn captured_round_continuity_rejects_skips_and_reversals() {
    let previous = FightRound {
        cur_round: Some(1),
        ..Default::default()
    };
    let skipped = FightRound {
        cur_round: Some(3),
        ..Default::default()
    };
    let reversed = FightRound {
        cur_round: Some(0),
        ..Default::default()
    };

    let error = validate_captured_round_continuity(&previous, &skipped).unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("previous round 1"));
    assert!(error.to_string().contains("next round 3"));
    assert!(validate_captured_round_continuity(&previous, &reversed).is_err());
}

#[test]
fn captured_round_continuity_accepts_next_terminal_and_missing_evidence() {
    let previous = FightRound {
        cur_round: Some(3),
        ..Default::default()
    };
    let next = FightRound {
        cur_round: Some(4),
        ..Default::default()
    };
    let terminal = FightRound {
        is_finish: Some(true),
        ..next.clone()
    };
    assert!(validate_captured_round_continuity(&previous, &next).is_ok());
    assert!(validate_captured_round_continuity(&next, &terminal).is_ok());
    assert!(validate_captured_round_continuity(&FightRound::default(), &next).is_ok());
    assert!(validate_captured_round_continuity(&next, &FightRound::default()).is_ok());
}

#[test]
fn captured_round_continuity_rejects_same_round_without_terminal_evidence() {
    let round = FightRound {
        cur_round: Some(4),
        ..Default::default()
    };

    assert!(validate_captured_round_continuity(&round, &round).is_err());

    let maximum = FightRound {
        cur_round: Some(i32::MAX),
        ..Default::default()
    };
    assert!(validate_captured_round_continuity(&maximum, &maximum).is_err());
}

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

#[cfg(feature = "private-fixtures")]
#[test]
fn captured_twins_selection_has_a_committed_runtime_source() {
    let db = init_config().unwrap();
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/battles/battle116385108/BeginRoundReply_1.json");
    let value = captured_start_reply(&path).unwrap();
    let fight: Fight = serde_json::from_value(value["fight"].clone()).unwrap();
    let (ex_attributes, sp_attributes) = preview_attributes(&fight, &path).unwrap();
    let mut runtime = BattleRuntime::new_with_attributes(
        battle::catalog::BattleCatalog::new(db),
        fight,
        ex_attributes,
        sp_attributes,
    );
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

#[cfg(feature = "private-fixtures")]
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

    let db = init_config().unwrap();
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/battles/battle72/BeginRoundReply_1.json");
    let generated = generate_reply(db, &path).unwrap().0.round.unwrap();
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

    let captured_signature = signature(&captured);
    assert_eq!(signature(&generated), captured_signature);
    assert_eq!(
        captured_signature.1,
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

    fn child_of<'a>(step: &'a FightStep, parent_id: i32, child_id: i32) -> Option<&'a FightStep> {
        if step.act_id == Some(parent_id) {
            return step.act_effect.iter().find_map(|effect| {
                effect
                    .fight_step
                    .as_ref()
                    .filter(|child| child.act_id == Some(child_id))
            });
        }
        step.act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
            .find_map(|nested| child_of(nested, parent_id, child_id))
    }

    fn reaction_frame(round: &FightRound) -> &FightStep {
        round
            .fight_step
            .iter()
            .find_map(|step| child_of(step, 31490111, 31430151))
            .expect("Atomic active-ally reaction frame")
    }
    assert_eq!(reaction_frame(&captured).to_id, Some(263620439));
    let generated_reaction = reaction_frame(&generated);
    assert_eq!(generated_reaction.to_id, Some(263620439));
    assert!(
        generated_reaction
            .act_effect
            .iter()
            .all(|effect| effect.target_id == Some(263620439))
    );
}

#[cfg(feature = "private-fixtures")]
#[test]
fn captured_116385711_keeps_opening_owner_and_source_threshold_semantics() {
    fn contains_act(step: &FightStep, act_id: i32) -> bool {
        step.act_id == Some(act_id)
            || step
                .act_effect
                .iter()
                .filter_map(|effect| effect.fight_step.as_ref())
                .any(|nested| contains_act(nested, act_id))
    }
    fn real_damage_kill_values(round: &FightRound) -> (Vec<String>, Vec<String>) {
        fn collect(step: &FightStep, markers: &mut Vec<String>, buffs: &mut Vec<String>) {
            for effect in &step.act_effect {
                if let Some(info) = effect
                    .buff_act_info
                    .as_ref()
                    .filter(|info| info.act_id == Some(1028))
                {
                    markers.push(info.str_param.clone().unwrap_or_default());
                }
                if let Some(buff) = effect.buff.as_ref() {
                    buffs.extend(
                        buff.act_info
                            .iter()
                            .filter(|info| info.act_id == Some(1028))
                            .map(|info| info.str_param.clone().unwrap_or_default()),
                    );
                }
                if let Some(nested) = effect.fight_step.as_ref() {
                    collect(nested, markers, buffs);
                }
            }
        }

        let mut markers = Vec::new();
        let mut buffs = Vec::new();
        for step in &round.fight_step {
            collect(step, &mut markers, &mut buffs);
        }
        (markers, buffs)
    }

    let db = init_config().unwrap();
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/battles/battle72/BeginRoundReply_1.json");
    let value = captured_start_reply(&path).unwrap();
    let fight: Fight = serde_json::from_value(value["fight"].clone()).unwrap();
    let captured: FightRound = serde_json::from_value(value["round"].clone()).unwrap();
    let (ex_attributes, sp_attributes) = preview_attributes(&fight, &path).unwrap();
    let opening_determinism = captured_opening_determinism(db, &fight, &captured);
    let mut runtime = BattleRuntime::new_with_attributes(
        battle::catalog::BattleCatalog::new(db),
        fight,
        ex_attributes,
        sp_attributes,
    );
    runtime
        .start_round_with_determinism(opening_determinism)
        .unwrap();
    let generated = battle::dungeon::start_reply(&runtime).round.unwrap();

    assert_eq!(generated.fight_step.len(), captured.fight_step.len());
    assert!(
        !captured
            .fight_step
            .iter()
            .any(|step| contains_act(step, 1163855066))
    );
    assert!(
        !generated
            .fight_step
            .iter()
            .any(|step| contains_act(step, 1163855066))
    );

    let expected = vec!["75680".to_owned(); 3];
    assert_eq!(
        real_damage_kill_values(&captured),
        (expected.clone(), expected.clone())
    );
    assert_eq!(
        real_damage_kill_values(&generated),
        (expected.clone(), expected)
    );
}

#[cfg(feature = "private-fixtures")]
#[test]
fn generated_round_uses_captured_rng_but_not_damage_amounts() {
    let db = init_config().unwrap();
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

    let expected = replay_to_round(db, &source.join("BeginRoundReply_1.json")).unwrap();
    let reply_path = temporary.join("BeginRoundReply_1.json");
    let mut captured: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&reply_path).unwrap()).unwrap();
    expand_compressed_fight_steps(&mut captured).unwrap();
    let round = captured.get_mut("round").unwrap();
    round["nextRoundBeginStep"] = serde_json::json!([]);
    round["fightStep"][2]["actEffect"][0]["effectNum"] = serde_json::json!(999_999);
    round["fightStep"][2]["actEffect"][0]["hurtInfo"]["damage"] = serde_json::json!(999_999);
    fs::write(&reply_path, serde_json::to_vec(&captured).unwrap()).unwrap();

    let actual = replay_to_round(db, &reply_path).unwrap();
    captured.get_mut("round").unwrap()["teamACards2"] = serde_json::json!([]);
    fs::write(&reply_path, serde_json::to_vec(&captured).unwrap()).unwrap();
    let without_card_choices = replay_to_round(db, &reply_path).unwrap();
    fs::remove_dir_all(temporary).unwrap();

    assert_eq!(actual, expected);
    assert_ne!(without_card_choices, expected);
}

#[cfg(feature = "private-fixtures")]
#[test]
fn reads_dungeon_and_tower_start_reply_envelopes() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/battles");

    let dungeon = captured_start_reply(&fixtures.join("battle69/BeginRoundReply_1.json"));
    let tower = captured_start_reply(&fixtures.join("battle74/BeginRoundReply_1.json"));

    assert!(dungeon.unwrap().get("fight").is_some());
    assert!(tower.unwrap().get("fight").is_some());
}
