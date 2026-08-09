fn normalize_custom_data_types(map: &mut serde_json::Map<String, serde_json::Value>) {
    let Some(entries) = map
        .get_mut("customData")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };
    for entry in entries {
        let Some(entry) = entry.as_object_mut() else {
            continue;
        };
        let Some(kind) = entry
            .get("type")
            .and_then(serde_json::Value::as_str)
            .and_then(crate::custom_data::CustomDataType::from_str_name)
        else {
            continue;
        };
        entry.insert("type".to_owned(), (kind as i32).into());
    }
}

pub fn normalize_live_json(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for key in [
                "uid",
                "fromId",
                "toId",
                "targetId",
                "targetUid",
                "fromUid",
                "userId",
                "equipUid",
                "defaultEquipUid",
                "heroUid",
                "lastChangeHeroUid",
                "reserveId",
                "assistHeroUid",
                "createUid",
                "createTime",
                "max",
                "value",
            ] {
                normalize_i64_field(map, key);
            }
            if map.get("fightActType").and_then(|value| value.as_str()) == Some("Normal") {
                map.insert("fightActType".to_owned(), 1.into());
            }
            normalize_custom_data_types(map);
            if let Some(card_type) = map.get("cardType").and_then(|value| value.as_str()) {
                let value = match card_type {
                    "NONE" => Some(0),
                    "ROUGE_SP" => Some(1),
                    "SUPPORT_NORMAL" => Some(2),
                    "SUPPORT_EX" => Some(3),
                    "NOT_COMPOSE" => Some(4),
                    "CARD_DECK_USE_ACT_POINT" => Some(5),
                    "SKILL3" => Some(6),
                    _ => None,
                };
                if let Some(value) = value {
                    map.insert("cardType".to_owned(), value.into());
                }
            }
            if let Some(status) = map.get("status").and_then(|value| value.as_str())
                && let Some(status) = crate::card_info::CardStatus::from_str_name(status)
            {
                map.insert("status".to_owned(), (status as i32).into());
            }
            if let Some(act_type) = map.get("actType").and_then(|value| value.as_str()) {
                let value = match act_type {
                    "SKILL" => Some(1),
                    "BUFF" => Some(2),
                    "EFFECT" => Some(3),
                    "CHANGEHERO" => Some(4),
                    "CHANGEWAVE" => Some(5),
                    _ => None,
                };
                if let Some(value) = value {
                    map.insert("actType".to_owned(), value.into());
                }
            }
            if let Some(damage_from_type) =
                map.get("damageFromType").and_then(|value| value.as_str())
            {
                let value = match damage_from_type {
                    "NONE" => Some(0),
                    "Skill" => Some(1),
                    "SkillEffect" => Some(2),
                    "Buff" => Some(3),
                    "Additional" => Some(4),
                    "AbsorbHurt" => Some(5),
                    "ShareHurt" => Some(6),
                    "FakeSkill" => Some(7),
                    _ => None,
                };
                if let Some(value) = value {
                    map.insert("damageFromType".to_owned(), value.into());
                }
            }
            for value in map.values_mut() {
                normalize_live_json(value);
            }
            prune_default_wrapper_fields(map);
        }
        serde_json::Value::Array(values) => {
            for value in values {
                normalize_live_json(value);
            }
        }
        _ => {}
    }
}

fn prune_default_wrapper_fields(map: &mut serde_json::Map<String, serde_json::Value>) {
    for key in [
        "cardIndex",
        "supportHeroId",
        "realSkillType",
        "realSkinId",
        "nowmalDmg",
    ] {
        if map.get(key).and_then(|value| value.as_i64()) == Some(0) {
            map.remove(key);
        }
    }
    if map.get("fakeTimeline").and_then(|value| value.as_bool()) == Some(false) {
        map.remove("fakeTimeline");
    }
    if map.get("reserveStr").and_then(|value| value.as_str()) == Some("") {
        map.remove("reserveStr");
    }
}

fn normalize_i64_field(map: &mut serde_json::Map<String, serde_json::Value>, key: &str) {
    let Some(value) = map.get(key).and_then(|value| value.as_str()) else {
        return;
    };
    let Ok(value) = value.parse::<i64>() else {
        return;
    };
    map.insert(key.to_owned(), value.into());
}
