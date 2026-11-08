use super::*;
use crate::excel_confgen::schema_infer::analyze_field_types;
use serde_json::json;

#[test]
fn activity_scoped_ids_do_not_expose_a_lossy_global_lookup() {
    let records = vec![
        json!({"activityId": 1, "id": 1}),
        json!({"activityId": 2, "id": 1}),
    ];
    for table in ["activity128_rewards", "activity243_const"] {
        let output = emit_table_store(table, &analyze_field_types(&records), &records);
        assert!(!output.contains("by_id"));
        assert!(!output.contains("pub fn get("));
    }
}

#[test]
fn weekwalk_dialog_indexes_every_row_in_an_id_group() {
    let records = vec![
        json!({"id": 101, "stepId": 1}),
        json!({"id": 101, "stepId": 2}),
    ];
    let output = emit_table_store("weekwalk_dialog", &analyze_field_types(&records), &records);

    assert!(!output.contains("pub fn get("));
    assert!(output.contains("pub fn by_group(&self, group_id: i32)"));
}

#[test]
fn multiple_group_fields_use_a_stable_index() {
    let records = vec![json!({
        "id": 1,
        "tradeGroupId": 2,
        "upgradeGroupId": 3
    })];
    let output = emit_table_store("building", &analyze_field_types(&records), &records);

    assert!(output.contains("by_group.entry(record.trade_group_id)"));
    assert!(!output.contains("by_group.entry(record.upgrade_group_id)"));
}
