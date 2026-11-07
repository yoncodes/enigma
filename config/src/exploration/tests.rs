#[test]
fn map_data_retains_client_unit_fields() {
    crate::init_test_config();
    let tables = crate::configs::get();
    let map = tables.explore_map.get(101).unwrap();
    let born = map
        .areas
        .values()
        .flat_map(|area| &area.units)
        .find(|unit| unit.id == 169)
        .unwrap();
    assert_eq!(
        (
            born.kind,
            born.node_pos,
            born.area_id,
            born.interact,
            born.unit_dir
        ),
        (0, [1, 6], 1, 3, 180)
    );
    let second = tables.explore_map.get(102).unwrap();
    let born = second
        .areas
        .values()
        .flat_map(|area| &area.units)
        .find(|unit| unit.id == 13)
        .unwrap();
    assert_eq!(
        (
            born.kind,
            born.node_pos,
            born.area_id,
            born.interact,
            born.unit_dir
        ),
        (0, [1, 2], 1000, 3, 90)
    );
    assert!(second.areas.values().any(|area| area.can_reset));

    let teleport = map
        .areas
        .values()
        .flat_map(|area| &area.units)
        .find(|unit| unit.id == 440)
        .unwrap();
    assert_eq!(teleport.trigger_dir, 4);
}
