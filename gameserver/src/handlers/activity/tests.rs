#[test]
fn activity_info_defaults_from_activity_table() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);

    assert!(super::info::default_activity_id_for_type(120).is_some());
}
