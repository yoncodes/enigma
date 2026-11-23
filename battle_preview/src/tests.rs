use serde_json::json;

#[test]
fn length_diffs_list_each_step_and_mark_the_moved_effect() {
    let generated = json!({"fightStep": [
        {"actEffect": [{"effectType": 162}, {"effectType": 7}]},
        {"actEffect": [{"effectType": 162}]},
    ]});
    let captured = json!({"fightStep": [
        {"actEffect": [{"effectType": 7}, {"effectType": 162}]},
    ]});

    assert_eq!(
        super::array_len_diff_lines(&generated, &captured, "/round/fightStep.len", "/round"),
        vec![
            "!! [0] generated=step(162 7) captured=step(7 162)",
            "!! [1] generated=step(162) captured=-",
        ]
    );
}

#[test]
fn external_outputs_keep_the_whole_capture_path() {
    let root = std::path::Path::new("gen");
    let first = super::external_output_path(root, std::path::Path::new("/a/Day1/Battle1/R.json"));
    let second = super::external_output_path(root, std::path::Path::new("/a/Day2/Battle1/R.json"));

    assert_ne!(first, second);
    assert!(first.ends_with("Day1/Battle1/R.json"));
}

#[test]
fn resolves_the_value_reported_by_a_diff_path() {
    let value = json!({"steps": [{"effects": [{"energy": 2}]}]});

    assert_eq!(
        super::value_at_diff_path(
            &value["steps"],
            "/round/fightStep[0]/effects[0]/energy",
            "/round/fightStep",
        ),
        Some(&json!(2))
    );
}

use super::{
    canonical_comparison, first_diff_path, preview_output_text,
    render_json_with_capture_conventions,
};

#[test]
fn canonical_comparison_reports_capture_only_fields() {
    let generated = json!({ "fight": { "version": 7 } });
    let captured = json!({ "fight": { "version": 7, "captureOnly": 9 } });

    let (generated, captured) = canonical_comparison(generated, captured);

    assert_eq!(
        first_diff_path(&generated, &captured, ""),
        Some("/fight/captureOnly".to_owned())
    );
}

#[test]
fn canonical_comparison_ignores_dedicated_enemy_card_cleanup_steps() {
    use sonettobuf::effect_type_enum::EffectType;
    use sonettobuf::fight_step::ActType;

    fn step(
        act_type: ActType,
        act_id: i64,
        from_id: i64,
        to_id: i64,
        effects: Vec<serde_json::Value>,
    ) -> serde_json::Value {
        json!({
            "actType": act_type as i32,
            "actId": act_id,
            "fromId": from_id,
            "toId": to_id,
            "actEffect": effects
        })
    }
    fn cleanup(effect_type: EffectType, team_type: i32) -> serde_json::Value {
        json!({ "effectType": effect_type as i32, "teamType": team_type })
    }
    fn round(steps: Vec<serde_json::Value>) -> serde_json::Value {
        json!({ "round": { "fightStep": steps } })
    }

    let deck_count = step(
        ActType::Effect,
        0,
        0,
        0,
        vec![cleanup(EffectType::Carddecknum, 1)],
    );
    let generated = json!({ "round": { "fightStep": [deck_count.clone()] } });
    let nested_cleanup = |effect_type| {
        json!({
            "effectType": EffectType::Fightstep as i32,
            "fightStep": step(
                ActType::Effect,
                0,
                0,
                0,
                vec![cleanup(effect_type, 2)]
            )
        })
    };
    let captured = round(vec![
        step(
            ActType::Effect,
            0,
            0,
            0,
            vec![
                nested_cleanup(EffectType::Cardremove2),
                nested_cleanup(EffectType::Removeentitycards),
            ],
        ),
        deck_count,
    ]);

    let (generated, captured) = canonical_comparison(generated, captured);
    assert_eq!(generated, captured);

    let enemy_cleanup = || cleanup(EffectType::Cardremove2, 2);
    for preserved in [
        step(
            ActType::Effect,
            0,
            0,
            0,
            vec![cleanup(EffectType::Cardremove2, 1)],
        ),
        step(ActType::Skill, 123, -1, -2, vec![enemy_cleanup()]),
        step(ActType::Effect, 123, 0, 0, vec![enemy_cleanup()]),
        step(ActType::Effect, 0, 123, 0, vec![enemy_cleanup()]),
        step(ActType::Effect, 0, 0, 123, vec![enemy_cleanup()]),
        step(
            ActType::Effect,
            0,
            0,
            0,
            vec![enemy_cleanup(), cleanup(EffectType::Carddecknum, 1)],
        ),
    ] {
        let (generated, preserved) =
            canonical_comparison(round(Vec::new()), round(vec![preserved]));
        assert_ne!(generated, preserved);
    }
}

#[test]
fn capture_conventions_change_representation_without_shaping_fields() {
    let generated = json!({
        "uid": 42,
        "summonedList": [],
        "cardType": 0,
        "actType": 3,
        "damageFromType": 2,
        "extraDefault": null
    });
    let template = json!({
        "uid": "42",
        "SummonedList": [],
        "cardType": "NONE",
        "actType": "EFFECT",
        "damageFromType": "SkillEffect",
        "captureOnly": 9
    });

    assert_eq!(
        render_json_with_capture_conventions(&generated, &template),
        json!({
            "uid": "42",
            "SummonedList": [],
            "cardType": "NONE",
            "actType": "EFFECT",
            "damageFromType": "SkillEffect",
            "extraDefault": null
        })
    );
}

#[test]
fn does_not_hide_misaligned_array_elements() {
    let generated = json!([{
        "actType": 1,
        "actId": 200,
        "actEffect": [{ "fightStep": { "actId": 201 } }]
    }]);
    let template = json!([{
        "actType": "EFFECT",
        "actId": 0,
        "actEffect": [{ "effectType": 1 }]
    }]);

    assert_eq!(
        render_json_with_capture_conventions(&generated, &template),
        generated
    );
}

#[test]
fn preserves_capture_text_when_generated_object_matches() {
    let value = json!({ "fight": {}, "round": {} });
    let text = "{\r\n  \"fight\": {},\r\n  \"round\": {}\r\n}".to_owned();

    assert_eq!(
        preview_output_text(&value, &value, text.clone()).unwrap(),
        text
    );
}

#[test]
fn recognizes_both_capture_round_reply_names() {
    assert!(super::is_begin_round_reply("begin_round_2.json"));
    assert!(super::is_begin_round_reply("BeginRoundReply_2.json"));
    assert!(!super::is_begin_round_reply("BeginRoundRequest_2.json"));
    assert!(!super::is_begin_round_reply("begin_round_2_request.json"));
}

#[test]
fn normalizes_protobuf_custom_data_type_names() {
    let mut value = json!({
        "customData": [
            { "type": "TowerCompose", "data": "{}" },
            { "type": "Atomic", "data": "{}" }
        ]
    });

    super::normalize_live_json(&mut value);

    assert_eq!(value["customData"][0]["type"], 8);
    assert_eq!(value["customData"][1]["type"], 9);
}

#[test]
fn normalizes_all_protobuf_card_status_names() {
    let mut value = json!({ "status": "STATUS_PLAYSETGRAY" });

    super::normalize_live_json(&mut value);

    assert_eq!(value["status"], 1);
}

#[test]
fn normalizes_numeric_heat_scale_maximums() {
    let mut value = json!({ "heatScale": { "max": "750", "value": "121" } });

    super::normalize_live_json(&mut value);

    assert_eq!(value["heatScale"]["max"], 750);
    assert_eq!(value["heatScale"]["value"], 121);
}

#[test]
fn tower_plan_comes_from_the_client_request() {
    let directory = std::env::temp_dir().join(format!("enigma-tower-plan-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("StartTowerBattleRequest.json"),
        r#"{"talentPlanId":401}"#,
    )
    .unwrap();
    let reply = directory.join("StartTowerBattleReply.json");

    assert_eq!(super::tower_plan_id(&reply), Some(401));
    std::fs::remove_dir_all(directory).unwrap();
}
