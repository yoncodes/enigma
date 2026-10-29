use super::*;

#[test]
fn current_cycle_maps_each_configured_layer_to_its_encounters() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let tables = config::configs::get();
    let cycle = current_cycle(tables).unwrap();
    assert!(!cycle.maps.is_empty());
    assert!(
        cycle
            .maps
            .windows(2)
            .all(|maps| maps[0].layer < maps[1].layer)
    );

    for map in &cycle.maps {
        let config = tables.weekwalk.get(map.map_id).unwrap();
        assert_eq!(config.issue_id, cycle.issue_id);
        assert_eq!(map.front_battle_id, config.fight_id_front);
        assert_eq!(map.rear_battle_id, config.fight_id_rear);
        assert_eq!(
            tables
                .weekwalk_element
                .get(map.front_element_id)
                .unwrap()
                .round_id,
            1
        );
        assert_eq!(
            tables
                .weekwalk_element
                .get(map.rear_element_id)
                .unwrap()
                .round_id,
            2
        );
    }
}
