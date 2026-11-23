use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn load_or_create_writes_missing_config() {
    let name = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("enigma-config-{name}"));
    let path = dir.join("config.toml");

    let cfg = ServerConfig::load_or_create(&path).unwrap();

    assert!(path.exists());
    assert_eq!(cfg.server.http_port, 21000);
    assert_eq!(cfg.server.game_port, 23301);
    assert!(!cfg.server.skip_tutorial);
    assert_eq!(cfg.muip.port, 21100);
    assert_eq!(cfg.muip_gm.port, 21101);
    assert_eq!(cfg.summon.permanent_pool_ids, [2]);
    assert_eq!(cfg.tutorial_cards.cards.len(), 4);
    assert_eq!(cfg.tutorial_cards.cards[1].episode_id, 10002);
    assert_eq!(
        cfg.tutorial_cards.cards[1].opening_cards,
        "100102#2|100102#2|100102#2|100101#1|100101#1|100101#2|100102#2"
    );
    assert_eq!(cfg.weekwalk_v2.schedule[0].time_id, 21);
    assert_eq!(cfg.weekwalk_v2.schedule[0].layers[0].layer_id, 1069);
    assert_eq!(cfg.rouge2.initial_attributes.len(), 61);
    assert_eq!(cfg.rouge2.initial_update_order.len(), 16);
    assert!(cfg.activity.schedule.iter().any(|row| row.id == 13930));
    assert_eq!(cfg.activity.hero_story_schedule.len(), 33);

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn missing_weekwalk_v2_schedule_uses_embedded_default() {
    let mut template: toml::Value = toml::from_str(CONFIG_TEMPLATE).unwrap();
    template.as_table_mut().unwrap().remove("weekwalk_v2");

    let config: ServerConfig = template.try_into().unwrap();
    assert_eq!(config.weekwalk_v2.schedule[0].time_id, 21);
}

#[test]
fn missing_tutorial_cards_use_embedded_defaults() {
    let mut template: toml::Value = toml::from_str(CONFIG_TEMPLATE).unwrap();
    template.as_table_mut().unwrap().remove("tutorial_cards");

    let config: ServerConfig = template.try_into().unwrap();
    assert_eq!(config.tutorial_cards.cards.len(), 4);
    assert_eq!(config.tutorial_cards.cards[0].episode_id, 10001);
}

#[test]
fn missing_rouge2_initialization_uses_embedded_default() {
    let mut template: toml::Value = toml::from_str(CONFIG_TEMPLATE).unwrap();
    template.as_table_mut().unwrap().remove("rouge2");

    let config: ServerConfig = template.try_into().unwrap();
    assert_eq!(config.rouge2.initial_attributes.len(), 61);
    assert_eq!(config.rouge2.initial_update_order[0], 320);
}

#[test]
fn missing_rouge2_store_currency_uses_embedded_default() {
    let mut template: toml::Value = toml::from_str(CONFIG_TEMPLATE).unwrap();
    template["rouge2"]
        .as_table_mut()
        .unwrap()
        .remove("store_currency_id");

    let config: ServerConfig = template.try_into().unwrap();
    assert_eq!(config.rouge2.store_currency_id, 3201);
}

#[test]
fn invalid_rouge2_initialization_is_rejected() {
    let validate = |mut config: ServerConfig| {
        config.paths.excel_data = std::env::temp_dir();
        config.database.path = std::env::temp_dir().join("enigma-config-test.db");
        config.validate_paths()
    };

    let mut duplicate = ServerConfig::template();
    duplicate
        .rouge2
        .initial_attributes
        .push(duplicate.rouge2.initial_attributes[0].clone());
    assert!(validate(duplicate).is_err());

    let mut unknown_update = ServerConfig::template();
    unknown_update.rouge2.initial_update_order.push(999);
    assert!(validate(unknown_update).is_err());

    let mut negative_value = ServerConfig::template();
    negative_value.rouge2.initial_attributes[0].value = -1;
    assert!(validate(negative_value).is_err());
}

#[test]
fn invalid_weekwalk_v2_schedule_is_rejected() {
    let mut duplicate = ServerConfig::template();
    duplicate.paths.excel_data = std::env::temp_dir();
    duplicate.database.path = std::env::temp_dir().join("enigma-config-test.db");
    duplicate
        .weekwalk_v2
        .schedule
        .push(duplicate.weekwalk_v2.schedule[0].clone());
    assert!(duplicate.validate_paths().is_err());

    let mut invalid_layer = ServerConfig::template();
    invalid_layer.paths.excel_data = std::env::temp_dir();
    invalid_layer.database.path = std::env::temp_dir().join("enigma-config-test.db");
    invalid_layer.weekwalk_v2.schedule[0].layers[0].rear_element_id =
        invalid_layer.weekwalk_v2.schedule[0].layers[0].front_element_id;
    assert!(invalid_layer.validate_paths().is_err());
}

#[test]
fn missing_hero_story_schedule_uses_embedded_default() {
    let activity: ActivityConfig = toml::from_str("schedule = []").unwrap();

    assert_eq!(activity.hero_story_schedule.len(), 33);
}

#[test]
fn configured_hero_story_schedule_replaces_default() {
    let activity: ActivityConfig = toml::from_str(
        r#"
            schedule = []
            hero_story_schedule = [
              { story_id = 99, start_time = 1, end_time = 2, start_time_resident = 3, end_time_resident = 4 }
            ]
            "#,
    )
    .unwrap();

    assert_eq!(activity.hero_story_schedule.len(), 1);
    assert_eq!(activity.hero_story_schedule[0].story_id, 99);
}

#[test]
fn activity_schedule_defaults_to_enabled_and_accepts_explicit_disable() {
    let enabled: ActivityConfig =
        toml::from_str("schedule = [{ id = 1, start_time = 10, end_time = 20, is_unlock = true }]")
            .unwrap();
    let disabled: ActivityConfig = toml::from_str(
        "schedule = [{ id = 1, start_time = 10, end_time = 20, is_unlock = true, enabled = false }]",
    )
    .unwrap();

    assert!(enabled.schedule[0].enabled);
    assert!(!disabled.schedule[0].enabled);
}
