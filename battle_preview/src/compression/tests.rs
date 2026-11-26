use std::io::Write;

use flate2::{Compression, write::GzEncoder};
use serde_json::json;

use super::*;

#[test]
fn compressed_steps_require_consistent_round_fields() {
    let step = FightStep {
        act_type: Some(sonettobuf::fight_step::ActType::Device as i32),
        from_id: Some(20),
        card_index: Some(0),
        ..Default::default()
    };
    let frame = step.encode_to_vec();
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(&1_u32.to_be_bytes()).unwrap();
    encoder
        .write_all(&(frame.len() as u32).to_be_bytes())
        .unwrap();
    encoder.write_all(&frame).unwrap();
    let bytes = encoder.finish().unwrap();
    let compressed = STANDARD.encode(&bytes);

    for expanded in [
        json!([]),
        json!([{ "actType": "DEVICE", "fromId": "20", "cardIndex": 0 }]),
    ] {
        let mut round = json!({
            "fightStepBytes": compressed,
            "totalStep": 1,
            "fightStep": expanded,
        });
        expand_compressed_fight_steps(&mut round).unwrap();
        assert_eq!(round, json!({ "fightStep": [step] }));
    }
    let mut round = json!({ "fightStepBytes": bytes, "totalStep": 1 });
    expand_compressed_fight_steps(&mut round).unwrap();
    assert_eq!(round, json!({ "fightStep": [step] }));

    for (total, expanded, expected_error) in [
        (1, json!([{ "fromId": 21 }]), "fightStep disagrees"),
        (2, json!([]), "totalStep 2 disagrees"),
    ] {
        let mut round = json!({
            "fightStepBytes": compressed,
            "totalStep": total,
            "fightStep": expanded,
        });
        let original = round.clone();
        let error = expand_compressed_fight_steps(&mut round).unwrap_err();
        assert!(error.to_string().contains(expected_error));
        assert_eq!(round, original);
    }
}

#[test]
fn empty_step_bytes_leave_the_expanded_round_importable() {
    for empty in [json!([]), json!("")] {
        let mut round = json!({ "fightStepBytes": empty, "fightStep": [{ "fromId": 20 }] });
        expand_compressed_fight_steps(&mut round).unwrap();
        let round: sonettobuf::FightRound = serde_json::from_value(round).unwrap();
        assert_eq!(round.fight_step[0].from_id, Some(20));
        assert!(round.fight_step_bytes.is_none());
    }
}
