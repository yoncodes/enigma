use super::*;
use prost::Message;

#[test]
fn card_data_enum_names_preserve_string_payloads() {
    let mut value = serde_json::json!({
        "key": "Unnamed",
        "round": { "teamACards1": [{
            "uid": "12",
            "cardDataes": [
                { "key": "Unnamed", "value": "001" },
                { "key": 1, "value": "{\"tier\":4}" }
            ]
        }] }
    });
    normalize::normalize_live_json(&mut value);
    assert_eq!(value["key"], "Unnamed");
    let round: FightRound = serde_json::from_value(value["round"].clone()).unwrap();
    assert_eq!(
        round.team_a_cards1[0].card_dataes[0].key,
        Some(card_data::CardDataKey::Unnamed as i32)
    );
    assert_eq!(
        round.team_a_cards1[0].card_dataes[0].value.as_deref(),
        Some("001")
    );
    assert_eq!(
        round.team_a_cards1[0].card_dataes[1].value.as_deref(),
        Some("{\"tier\":4}")
    );
    value["round"]["teamACards1"][0]["cardDataes"][0]["key"] = "Unknown".into();
    normalize::normalize_live_json(&mut value);
    assert_eq!(
        value["round"]["teamACards1"][0]["cardDataes"][0]["key"],
        "Unknown"
    );
    assert!(serde_json::from_value::<FightRound>(value["round"].clone()).is_err());
}

#[test]
fn friend_list_reply_decodes_the_friend_module_friend_info() {
    // info { userId: 7, name: "Sonetto" }; field 2 is a string in FriendModule's FriendInfo.
    let mut bytes = vec![0x0a, 0x0b, 0x08, 0x07, 0x12, 0x07];
    bytes.extend_from_slice(b"Sonetto");

    let reply = GetFriendInfoListReply::decode(&bytes[..]).expect("valid friend list reply");

    assert_eq!(reply.info[0].user_id, Some(7));
    assert_eq!(reply.info[0].name.as_deref(), Some("Sonetto"));
}
