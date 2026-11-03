use anyhow::{Context, Result, ensure};
use config::exploration::Map;
use database::models::game::explore::state::{InteractState, MapState};
use sonettobuf::{ExploreInfoNo, ExploreInteractNo, ExploreMapNo};

pub(super) fn initial(map: &Map) -> Result<MapState> {
    // ExploreEnum.ItemType.Born identifies the configured entry point.
    let mut entries = map
        .areas
        .values()
        .flat_map(|area| &area.units)
        .filter(|unit| unit.kind == 0);
    let born = entries
        .next()
        .context("exploration entry point is missing")?;
    ensure!(
        entries.next().is_none(),
        "exploration has multiple entry points"
    );
    Ok(MapState {
        map_id: map.id,
        posx: born.node_pos[0],
        posy: born.node_pos[1],
        dir: born.unit_dir,
        challenge_count: 0,
        interacts: map
            .areas
            .values()
            .flat_map(|area| &area.units)
            .map(|unit| InteractState {
                id: unit.id,
                kind: unit.kind,
                status: unit.interact,
                status2: String::new(),
                posx: unit.node_pos[0],
                posy: unit.node_pos[1],
                dir: unit.unit_dir,
            })
            .collect(),
        area_ids: vec![born.area_id],
        move_nodes: String::new(),
    })
}

pub(super) fn info(map: MapState, first: bool) -> ExploreInfoNo {
    ExploreInfoNo {
        explore_map: Some(ExploreMapNo {
            map_id: Some(map.map_id),
            posx: Some(map.posx),
            posy: Some(map.posy),
            dir: Some(map.dir),
            interacts: map
                .interacts
                .into_iter()
                .map(|unit| ExploreInteractNo {
                    id: Some(unit.id),
                    r#type: Some(unit.kind),
                    status: Some(unit.status),
                    status2: Some(unit.status2),
                    posx: Some(unit.posx),
                    posy: Some(unit.posy),
                    dir: Some(unit.dir),
                })
                .collect(),
            area_ids: map.area_ids,
            move_nodes: Some(map.move_nodes),
            is_first_enter: Some(first),
            challenge_count: Some(map.challenge_count),
        }),
        explore_items: Vec::new(),
        use_item_uid: Some(0),
    }
}
