use super::*;

#[test]
fn catalog_contains_every_configured_activity() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);

    let infos = catalog_infos();
    assert_eq!(infos.len(), config::configs::get().activity.len());
    assert!(
        config::configs::get()
            .activity
            .iter()
            .all(|activity| infos.iter().any(|info| info.id == Some(activity.id as u32)))
    );
}

#[test]
fn current_schedule_replaces_the_expired_catalog() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);

    assert!(!is_activity_online(ActivityId::V3a6Dungeon.id()));
    assert!(!is_activity_online(138502));
    assert!(!is_activity_online(138512));
    assert!(!is_activity_online(138508));
    assert!(is_activity_online(13901));
    assert!(is_activity_online(13930));
    assert_eq!(
        catalog_infos()
            .iter()
            .filter(|info| info.online == Some(true))
            .count(),
        134
    );
}

#[test]
fn activity101_login_progress_uses_each_active_schedule_window() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);

    let activity_id = 13905;
    let schedule = crate::activity::schedule::get(activity_id).unwrap();
    assert!(!active_act101_activity_ids_at(schedule.start_time - 1).contains(&activity_id));
    assert!(active_act101_activity_ids_at(schedule.start_time).contains(&activity_id));
    assert!(active_act101_activity_ids_at(schedule.end_time).contains(&activity_id));
    assert!(!active_act101_activity_ids_at(schedule.end_time + 1).contains(&activity_id));
    assert!(!active_act101_activity_ids_at(schedule.start_time).contains(&13714));
}

#[test]
fn permanent_end_matches_lua_millisecond_time_shape() {
    assert_eq!(PERMANENT_END_MS, 2_145_934_800_000);
}

#[test]
fn activity_time_uses_copost_version_window() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);

    assert_eq!(
        version_activity_time_range(),
        Some((1_786_597_200_000, 1_790_225_999_000))
    );
}

#[test]
fn parses_copost_time_with_extra_space() {
    assert_eq!(
        parse_config_time_millis("2026-07-23  4:59:59"),
        Some(1_784_782_799_000)
    );
}

#[test]
fn schedule_preserves_per_activity_windows_and_unlocks() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);

    assert_eq!(
        activity_time_range(13801),
        (1_790_244_000_000, 1_793_872_799_000)
    );
    assert_eq!(
        activity_time_range(12801),
        (1_786_615_200_000, 2_145_934_800_000)
    );
    assert!(!is_unlocked_by_default(12301));
}

#[test]
fn empty_param_request_keeps_the_catalog_and_current_act125() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);

    let mut infos = catalog_infos();
    apply_act125_activity(&mut infos);

    assert!(infos.iter().any(|info| info.id.is_some()));
    assert_eq!(default_act125_activity_id(), Some(13924));
    assert!(infos.iter().any(|info| info.id == Some(13924)));
    assert!(
        infos
            .iter()
            .any(|info| info.id == default_act125_activity_id().map(|id| id as u32))
    );
    assert_eq!(
        infos
            .iter()
            .find(|info| info.id == Some(13116))
            .and_then(|info| info.online),
        Some(false)
    );
    assert_eq!(
        infos
            .iter()
            .find(|info| info.id == Some(13612))
            .and_then(|info| info.online),
        Some(false)
    );
}

#[test]
fn current_act125_claim_has_material_reward() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let activity_id = default_act125_activity_id().unwrap();
    let row = config::configs::get()
        .activity125
        .iter()
        .find(|row| row.activity_id == activity_id)
        .unwrap();

    assert!(
        !crate::reward::parse(&row.bonus)
            .material_changes()
            .is_empty()
    );
}
