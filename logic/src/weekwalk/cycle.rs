use config::GameDB;
use database::db::game::weekwalk::{WeekwalkEncounterSeed, WeekwalkMapSeed};

use crate::error::AppError;

pub(super) struct WeekwalkCycle {
    pub issue_id: i32,
    pub maps: Vec<WeekwalkMapSeed>,
}

pub(super) fn current(tables: &GameDB) -> Result<WeekwalkCycle, AppError> {
    let issue_id = tables
        .weekwalk
        .iter()
        .filter(|map| map.issue_id > 0 && map.fight_id_front > 0 && map.fight_id_rear > 0)
        .map(|map| map.issue_id)
        .max()
        .ok_or_else(|| AppError::Custom("no usable Weekwalk issue configured".into()))?;
    let mut deep_maps = tables
        .weekwalk
        .iter()
        .filter(|map| map.issue_id == issue_id)
        .collect::<Vec<_>>();
    deep_maps.sort_unstable_by_key(|map| map.layer);
    let first_layer = deep_maps.first().ok_or(AppError::InvalidRequest)?.layer;
    let scene_family = deep_maps.first().ok_or(AppError::InvalidRequest)?.scene_id / 10;
    let mut front = deep_elements(tables, scene_family, 1);
    let mut rear = deep_elements(tables, scene_family, 2);
    front.sort_unstable();
    rear.sort_unstable();

    let deep_maps = deep_maps
        .into_iter()
        .map(|map| {
            let index =
                usize::try_from(map.layer - first_layer).map_err(|_| AppError::InvalidRequest)?;
            Ok(WeekwalkMapSeed {
                map_id: map.id,
                scene_id: map.scene_id,
                layer: map.layer,
                is_deep: true,
                buff_id: 0,
                encounters: vec![
                    WeekwalkEncounterSeed {
                        battle_id: map.fight_id_front,
                        element_id: *front.get(index).ok_or(AppError::InvalidRequest)?,
                        visible: true,
                    },
                    WeekwalkEncounterSeed {
                        battle_id: map.fight_id_rear,
                        element_id: *rear.get(index).ok_or(AppError::InvalidRequest)?,
                        visible: false,
                    },
                ],
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let mut shallow_maps = tables
        .weekwalk
        .iter()
        .filter(|map| map.issue_id == 0 && map.layer > 0 && map.layer < first_layer)
        .collect::<Vec<_>>();
    shallow_maps.sort_unstable_by_key(|map| map.layer);
    if shallow_maps.windows(2).any(|maps| {
        maps[1].pre_id != maps[0].id || maps[1].layer != maps[0].layer.saturating_add(1)
    }) {
        return Err(AppError::Custom(
            "Weekwalk shallow map chain is not contiguous".into(),
        ));
    }
    let mut groups = tables
        .weekwalk_element
        .iter()
        .filter(is_shallow_carrier)
        .filter_map(|element| battle_step(element).map(|(_, battle_id)| battle_id / 100))
        .collect::<Vec<_>>();
    groups.sort_unstable();
    groups.dedup();
    if groups.len() != shallow_maps.len()
        || groups
            .windows(2)
            .any(|groups| groups[1] != groups[0].saturating_add(1))
    {
        return Err(AppError::Custom(format!(
            "Weekwalk shallow encounter groups do not match the map chain: {groups:?}"
        )));
    }
    let shallow_maps = shallow_maps
        .into_iter()
        .zip(groups)
        .map(|(map, group)| {
            let scene = tables
                .weekwalk_scene
                .get(map.scene_id)
                .ok_or(AppError::InvalidRequest)?;
            let mut encounters = tables
                .weekwalk_element
                .iter()
                .filter(is_shallow_carrier)
                .filter_map(|element| {
                    battle_step(element).and_then(|(_, battle_id)| {
                        (battle_id / 100 == group).then_some(WeekwalkEncounterSeed {
                            battle_id,
                            element_id: element.id,
                            visible: true,
                        })
                    })
                })
                .collect::<Vec<_>>();
            encounters.sort_unstable_by_key(|encounter| encounter.battle_id);
            if encounters.is_empty() {
                return Err(AppError::Custom(format!(
                    "no Weekwalk encounters configured for map {}",
                    map.id
                )));
            }
            Ok(WeekwalkMapSeed {
                map_id: map.id,
                scene_id: map.scene_id,
                layer: map.layer,
                is_deep: false,
                buff_id: scene.buff_id,
                encounters,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let maps = shallow_maps.into_iter().chain(deep_maps).collect();
    Ok(WeekwalkCycle { issue_id, maps })
}

pub(super) fn battle_step(
    element: &config::weekwalk_element::WeekwalkElement,
) -> Option<(i32, i32)> {
    element
        .r#type
        .split('#')
        .zip(element.param.split('|'))
        .enumerate()
        .find_map(|(index, (kind, param))| {
            let battle_id = param.parse::<i32>().ok()?;
            (kind == "1" && battle_id > 0).then(|| Some((i32::try_from(index).ok()?, battle_id)))?
        })
}

pub(super) fn element_step(
    tables: &GameDB,
    element_id: i32,
    index: i32,
) -> Result<(i32, i32, i32), AppError> {
    let element = tables
        .weekwalk_element
        .get(element_id)
        .ok_or(AppError::InvalidRequest)?;
    let kinds = element.r#type.split('#').collect::<Vec<_>>();
    let params = element.param.split('|').collect::<Vec<_>>();
    let position = usize::try_from(index).map_err(|_| AppError::InvalidRequest)?;
    let kind = kinds
        .get(position)
        .ok_or(AppError::InvalidRequest)?
        .parse()
        .map_err(|_| AppError::InvalidRequest)?;
    let param = params
        .get(position)
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    Ok((
        kind,
        param,
        i32::try_from(kinds.len()).map_err(|_| AppError::InvalidRequest)?,
    ))
}

pub(super) fn dialog_option_valid(tables: &GameDB, dialog_id: i32, option: i32) -> bool {
    let options = tables
        .weekwalk_dialog
        .by_group(dialog_id)
        .filter_map(|row| row.option_param.parse::<i32>().ok())
        .collect::<Vec<_>>();
    if options.is_empty() {
        option == 0
    } else {
        options.contains(&option)
    }
}

fn deep_elements(tables: &GameDB, scene_family: i32, round_id: i32) -> Vec<i32> {
    tables
        .weekwalk_element
        .iter()
        .filter(|element| {
            element.id / 100_000 == scene_family
                && element.round_id == round_id
                && element.skip_finish == 0
                && element.r#type.split('#').any(|kind| kind == "1")
        })
        .map(|element| element.id)
        .collect()
}

fn is_shallow_carrier(element: &&config::weekwalk_element::WeekwalkElement) -> bool {
    element.skip_finish == 0
        && element.round_id == 0
        && element.r#type.split('#').any(|kind| kind == "4")
}
