use super::*;

#[test]
fn initial_outside_info_matches_live_capture() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let info = outside_info(config::configs::get()).info.unwrap();

    assert_eq!(info.season, Some(13406));
    assert_eq!(
        info.role_box
            .unwrap()
            .roles
            .iter()
            .map(|role| (role.role_id, role.max_progress, role.unlocked))
            .collect::<Vec<_>>(),
        vec![
            (Some(1), Some(0), Some(true)),
            (Some(2), Some(0), Some(true)),
            (Some(3), Some(3), Some(false)),
            (Some(999), Some(0), Some(true)),
            (Some(4), Some(0), Some(true)),
            (Some(5), Some(4), Some(false)),
            (Some(6), Some(5), Some(false))
        ]
    );
    assert_eq!(info.mall.unwrap().items.len(), 17);
}
