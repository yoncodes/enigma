use super::*;

#[test]
fn session_timeline_splits_battles_and_assigns_cloth_to_the_next_round() {
    let files = [
        "20260927_080404_694_000408_StartDungeonRequest.json",
        "20260927_080404_757_000410_StartDungeonReply.json",
        "20260927_080409_543_000415_UseClothSkillRequest.json",
        "20260927_080411_302_000420_BeginRoundRequest.json",
        "20260927_080411_371_000421_BeginRoundReply.json",
        "20260927_080415_000_000422_UseClothSkillRequest.json",
        "20260927_080420_569_000424_BeginRoundRequest.json",
        "20260927_080420_623_000425_BeginRoundReply.json",
        "20260927_080425_182_000460_EndFightPush.json",
        "20260927_080430_000_000470_GetServerTimeReply.json",
        "20260927_080433_212_000474_StartDungeonReply.json",
        "20260927_080439_544_000485_BeginRoundRequest.json",
        "20260927_080439_611_000486_BeginRoundReply.json",
    ]
    .map(PathBuf::from);

    let battles = split_session(&files);

    assert_eq!(battles.len(), 2);
    let first = &battles[0];
    assert_eq!(first.start, files[1]);
    assert_eq!(first.start_request.as_ref(), Some(&files[0]));
    assert_eq!(
        first.rounds,
        vec![
            CapturedRound {
                index: 1,
                request: files[3].clone(),
                reply: files[4].clone(),
                autos: Vec::new(),
                cloth: vec![CapturedCloth {
                    request: files[2].clone(),
                    reply: None,
                }],
            },
            CapturedRound {
                index: 2,
                request: files[6].clone(),
                reply: files[7].clone(),
                autos: Vec::new(),
                cloth: vec![CapturedCloth {
                    request: files[5].clone(),
                    reply: None,
                }],
            },
        ]
    );
    assert_eq!(battles[1].start, files[10]);
    assert_eq!(battles[1].start_request, None);
    assert_eq!(battles[1].rounds.len(), 1);
}

#[test]
fn session_timeline_pairs_a_cloth_reply_with_its_request() {
    let files = [
        "20260927_080404_757_000410_StartDungeonReply.json",
        "20260927_080409_543_000415_UseClothSkillRequest.json",
        "20260927_080409_600_000416_UseClothSkillReply.json",
        "20260927_080411_302_000420_BeginRoundRequest.json",
        "20260927_080411_371_000421_BeginRoundReply.json",
    ]
    .map(PathBuf::from);

    let battles = split_session(&files);

    assert_eq!(battles[0].rounds[0].cloth.len(), 1);
    assert_eq!(battles[0].rounds[0].cloth[0].request, files[1]);
    assert_eq!(
        battles[0].rounds[0].cloth[0].reply.as_ref(),
        Some(&files[2])
    );
}

#[test]
fn session_timeline_pairs_auto_planning_with_the_following_round() {
    let files = [
        "20261011_110256_429_002589_StartDungeonReply.json",
        "20261011_110305_582_002594_AutoRoundRequest.json",
        "20261011_110305_631_002595_AutoRoundReply.json",
        "20261011_110308_819_002596_BeginRoundRequest.json",
        "20261011_110308_964_002597_BeginRoundReply.json",
    ]
    .map(PathBuf::from);

    let battles = split_session(&files);

    assert_eq!(
        battles[0].rounds[0].autos,
        vec![CapturedAuto {
            request: files[1].clone(),
            reply: files[2].clone(),
            after_cloth: 0,
        }]
    );
}

#[test]
fn session_timeline_keeps_auto_plans_on_each_side_of_cloth_use() {
    let files = [
        "20261011_110000_000_000001_StartDungeonReply.json",
        "20261011_110001_000_000002_AutoRoundRequest.json",
        "20261011_110002_000_000003_AutoRoundReply.json",
        "20261011_110003_000_000004_UseClothSkillRequest.json",
        "20261011_110004_000_000005_UseClothSkillReply.json",
        "20261011_110005_000_000006_AutoRoundRequest.json",
        "20261011_110006_000_000007_AutoRoundReply.json",
        "20261011_110007_000_000008_BeginRoundRequest.json",
        "20261011_110008_000_000009_BeginRoundReply.json",
    ]
    .map(PathBuf::from);

    let battles = split_session(&files);
    let round = &battles[0].rounds[0];

    assert_eq!(round.autos.len(), 2);
    assert_eq!(round.autos[0].request, files[1]);
    assert_eq!(round.autos[0].reply, files[2]);
    assert_eq!(round.autos[0].after_cloth, 0);
    assert_eq!(round.autos[1].request, files[5]);
    assert_eq!(round.autos[1].reply, files[6]);
    assert_eq!(round.autos[1].after_cloth, 1);
}

#[test]
fn session_timeline_positions_auto_plan_when_its_request_arrives() {
    let files = [
        "20261011_110000_000_000001_StartDungeonReply.json",
        "20261011_110001_000_000002_AutoRoundRequest.json",
        "20261011_110002_000_000003_UseClothSkillRequest.json",
        "20261011_110003_000_000004_UseClothSkillReply.json",
        "20261011_110004_000_000005_AutoRoundReply.json",
        "20261011_110005_000_000006_BeginRoundRequest.json",
        "20261011_110006_000_000007_BeginRoundReply.json",
    ]
    .map(PathBuf::from);

    let battles = split_session(&files);
    let auto = &battles[0].rounds[0].autos[0];

    assert_eq!(auto.request, files[1]);
    assert_eq!(auto.reply, files[4]);
    assert_eq!(auto.after_cloth, 0);
}

#[test]
fn session_timeline_does_not_carry_incomplete_auto_input_across_rounds() {
    let files = [
        "20261011_110000_000_000001_StartDungeonReply.json",
        "20261011_110001_000_000002_AutoRoundRequest.json",
        "20261011_110002_000_000003_BeginRoundRequest.json",
        "20261011_110003_000_000004_BeginRoundReply.json",
        "20261011_110004_000_000005_AutoRoundReply.json",
        "20261011_110005_000_000006_BeginRoundRequest.json",
        "20261011_110006_000_000007_BeginRoundReply.json",
    ]
    .map(PathBuf::from);

    let battles = split_session(&files);

    assert_eq!(battles[0].rounds.len(), 2);
    assert!(battles[0].rounds[0].autos.is_empty());
    assert!(battles[0].rounds[1].autos.is_empty());
}

#[test]
fn fixed_folder_pairs_timestamped_cloth_packets_by_order() {
    let directory =
        std::env::temp_dir().join(format!("enigma-cloth-pairing-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    for name in [
        "UseClothSkillRequest_8_20261011_123156_212.json",
        "UseClothSkillReply_8_20261011_123156_277.json",
        "UseClothSkillRequest_8_20261011_123157_839.json",
        "UseClothSkillReply_8_20261011_123157_907.json",
    ] {
        fs::write(directory.join(name), "{}").unwrap();
    }

    let inputs = cloth_inputs(&directory, 8).unwrap();

    assert_eq!(inputs.len(), 2);
    assert_eq!(
        inputs[0].reply.as_deref().and_then(Path::file_name),
        Some(std::ffi::OsStr::new(
            "UseClothSkillReply_8_20261011_123156_277.json"
        ))
    );
    assert_eq!(
        inputs[1].reply.as_deref().and_then(Path::file_name),
        Some(std::ffi::OsStr::new(
            "UseClothSkillReply_8_20261011_123157_907.json"
        ))
    );

    fs::remove_file(directory.join("UseClothSkillReply_8_20261011_123157_907.json")).unwrap();
    assert_eq!(
        cloth_inputs(&directory, 8).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
    fs::write(
        directory.join("UseClothSkillReply_8_20261011_123157_907.json"),
        "{}",
    )
    .unwrap();
    fs::write(
        directory.join("UseClothSkillReply_8_20261011_123159_548.json"),
        "{}",
    )
    .unwrap();
    assert_eq!(
        cloth_inputs(&directory, 8).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn fixed_folder_recovers_cloth_request_from_common_capture() {
    let root = std::env::temp_dir().join(format!(
        "enigma-common-cloth-pairing-{}",
        std::process::id()
    ));
    let battle = root.join("decoded/FocusedFlashback/Battle1");
    let common = root.join("decoded/common");
    fs::create_dir_all(&battle).unwrap();
    fs::create_dir_all(&common).unwrap();
    let request = common.join("20261104_140111_560_000235_UseClothSkillRequest.json");
    fs::write(&request, r#"{"type":9}"#).unwrap();
    fs::write(
        common.join("20261104_140111_694_000236_UseClothSkillReply.json"),
        r#"{"round":{"curRound":3}}"#,
    )
    .unwrap();
    let reply = battle.join("UseClothSkillReply_3.json");
    fs::write(&reply, r#"{"round":{"curRound":3}}"#).unwrap();

    assert_eq!(
        cloth_inputs(&battle, 3).unwrap(),
        vec![CapturedCloth {
            request,
            reply: Some(reply),
        }]
    );

    fs::write(
        common.join("20261104_140112_560_000237_UseClothSkillRequest.json"),
        r#"{"type":8}"#,
    )
    .unwrap();
    fs::write(
        common.join("20261104_140112_694_000238_UseClothSkillReply.json"),
        r#"{"round":{"curRound":3}}"#,
    )
    .unwrap();
    assert_eq!(
        cloth_inputs(&battle, 3).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn activity_timeline_pairs_the_wrapped_start_request_and_reply() {
    let files = [
        "20260927_080404_694_000408_StartAct229BattleRequest.json",
        "20260927_080404_757_000410_StartAct229BattleReply.json",
        "20260927_080411_302_000420_BeginRoundRequest.json",
        "20260927_080411_371_000421_BeginRoundReply.json",
    ]
    .map(PathBuf::from);

    let battles = split_session(&files);

    assert_eq!(battles.len(), 1);
    assert_eq!(battles[0].start, files[1]);
    assert_eq!(battles[0].start_request.as_ref(), Some(&files[0]));
    assert_eq!(battles[0].rounds.len(), 1);
}

#[test]
fn selects_one_timestamped_reply_from_a_session() {
    let directory = std::env::temp_dir().join(format!(
        "enigma-session-reply-selection-{}",
        std::process::id()
    ));
    fs::create_dir_all(&directory).unwrap();
    for name in [
        "20260927_080404_757_000410_StartDungeonReply.json",
        "20260927_080411_302_000420_BeginRoundRequest.json",
        "20260927_080411_371_000421_BeginRoundReply.json",
        "20260927_080420_569_000424_BeginRoundRequest.json",
        "20260927_080420_623_000425_BeginRoundReply.json",
    ] {
        fs::write(directory.join(name), "{}").unwrap();
    }
    let selected = directory.join("20260927_080411_371_000421_BeginRoundReply.json");

    let (battle, through) = CapturedBattle::from_session_reply(&selected).unwrap();

    assert_eq!(through, 1);
    assert_eq!(battle.rounds.len(), 2);
    fs::remove_dir_all(directory).unwrap();
}
