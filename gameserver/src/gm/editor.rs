use super::{
    GrantData, is_premium_hero_skin, parse_positive, resolve_name, send_push, send_snapshot_pushes,
};
use crate::{
    logic::{collection::CollectionManager, hero::HeroManager, inventory::InventoryManager},
    net::app::AppState,
};
use anyhow::Result;
use database::{
    db::game::items,
    models::game::{
        equipment::{EquipmentModel, UserEquipmentModel},
        heros::UserHeroModel,
    },
};
use muipserver::GmResponse;
use serde::Serialize;
use sonettobuf::{CmdId, EquipUpdatePush};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    io::Write,
    sync::Mutex,
};
use toml_edit::{DocumentMut, Value};

static EVENT_CONFIG_LOCK: Mutex<()> = Mutex::new(());

pub(super) async fn achievements(
    state: &'static AppState,
    player_id: i64,
    args: &[&str],
) -> Result<GmResponse> {
    require_all(args, "achievement unlock all")?;
    let push = CollectionManager::new(player_id)
        .complete_all_achievements(state.db)
        .await?;
    let count = push.infos.len();
    send_push(state, player_id, CmdId::UpdateAchievementPushCmd, push).await?;
    Ok(GmResponse::ok(format!("unlocked {count} achievements")))
}

pub(super) async fn backgrounds(
    state: &'static AppState,
    player_id: i64,
    args: &[&str],
) -> Result<GmResponse> {
    require_all(args, "background unlock all")?;
    let item_ids = state
        .tables
        .player_bg
        .iter()
        .map(|background| background.item)
        .filter(|item_id| *item_id > 0)
        .collect::<BTreeSet<_>>();
    let mut missing = Vec::new();
    for item_id in item_ids {
        if items::get_item(state.db, player_id, item_id as u32)
            .await?
            .is_none_or(|item| item.quantity <= 0)
        {
            missing.push((item_id as u32, 1));
        }
    }
    if missing.is_empty() {
        return Ok(GmResponse::ok("all backgrounds are already unlocked"));
    }

    let applied = crate::logic::reward::RewardManager::new(player_id)
        .apply(
            state.db,
            crate::logic::reward::RewardSet {
                items: missing.clone(),
                ..Default::default()
            },
        )
        .await?;
    let mut data = GrantData::empty(player_id);
    data.merge_rewards(applied);
    data.deduplicate();
    send_snapshot_pushes(state, &data).await?;
    Ok(GmResponse::ok_data(
        format!("unlocked {} backgrounds", missing.len()),
        data,
    ))
}

pub(super) async fn garments(
    state: &'static AppState,
    player_id: i64,
    args: &[&str],
) -> Result<GmResponse> {
    require_all(args, "garment unlock all")?;
    let heroes = UserHeroModel::new(player_id, (*state.db).clone());
    let owned_heroes = heroes
        .get_all_heroes()
        .await?
        .into_iter()
        .map(|hero| hero.record.hero_id)
        .collect::<HashSet<_>>();
    let owned_skins = heroes
        .get_skins()
        .await?
        .into_iter()
        .collect::<HashSet<_>>();
    let skins = state
        .tables
        .skin
        .iter()
        .filter(|skin| owned_heroes.contains(&skin.character_id))
        .filter(|skin| is_premium_hero_skin(state.tables, skin))
        .filter(|skin| !owned_skins.contains(&skin.id))
        .map(|skin| skin.id)
        .collect::<Vec<_>>();
    if skins.is_empty() {
        return Ok(GmResponse::ok(
            "all garments for owned heroes are already unlocked",
        ));
    }
    let unlocked = heroes.unlock_skins_without_equipping(&skins).await?;
    let unlocked_count = unlocked.len();

    let mut data = GrantData::empty(player_id);
    data.changed_hero_ids = unlocked
        .iter()
        .filter_map(|skin_id| state.tables.skin.get(*skin_id))
        .map(|skin| skin.character_id)
        .collect();
    data.deduplicate();
    send_snapshot_pushes(state, &data).await?;
    Ok(GmResponse::ok_data(
        format!("unlocked {unlocked_count} garments"),
        data,
    ))
}

pub(super) async fn hero(
    state: &'static AppState,
    player_id: i64,
    args: &[&str],
) -> Result<GmResponse> {
    match args {
        [level, hero_id, target] if level.eq_ignore_ascii_case("level") => {
            let hero_id = parse_positive(hero_id, "hero id")?;
            let target = parse_positive(target, "hero level")?;
            let update = HeroManager::new(player_id)
                .set_level(state.db, hero_id, target)
                .await?;
            let mut data = GrantData::empty(player_id);
            data.changed_hero_ids.push(hero_id);
            data.changed_skin_ids.extend(update.skin_id);
            send_snapshot_pushes(state, &data).await?;
            Ok(GmResponse::ok_data(
                format!("set hero {hero_id} to level {target}"),
                update.hero,
            ))
        }
        [portrait, all]
            if portrait.eq_ignore_ascii_case("portrait") && all.eq_ignore_ascii_case("all") =>
        {
            let update = HeroManager::new(player_id)
                .set_all_portrait_levels(state.db, 5)
                .await?;
            let mut data = GrantData::empty(player_id);
            data.changed_hero_ids = update.hero_ids;
            data.changed_item_ids = update.item_ids.into_iter().map(|id| id as i32).collect();
            data.deduplicate();
            send_snapshot_pushes(state, &data).await?;
            Ok(GmResponse::ok_data(
                format!(
                    "maxed {} hero portraits and added {} missing shards",
                    data.changed_hero_ids.len(),
                    update.granted
                ),
                data,
            ))
        }
        [portrait, hero_id, target] if portrait.eq_ignore_ascii_case("portrait") => {
            let hero_id = parse_positive(hero_id, "hero id")?;
            let target = parse_positive(target, "portrait level")?;
            let update = HeroManager::new(player_id)
                .set_portrait_level(state.db, hero_id, target)
                .await?;
            let mut data = GrantData::empty(player_id);
            data.changed_hero_ids.push(hero_id);
            data.changed_item_ids.push(update.item_id as i32);
            send_snapshot_pushes(state, &data).await?;
            Ok(GmResponse::ok_data(
                format!(
                    "set hero {hero_id} portrait to {target} and added {} missing shards",
                    update.granted
                ),
                data,
            ))
        }
        [resonance, hero_id, target] if resonance.eq_ignore_ascii_case("resonance") => {
            let hero_id = parse_positive(hero_id, "hero id")?;
            let target = parse_positive(target, "resonance")?;
            let update = HeroManager::new(player_id)
                .set_resonance(state.db, hero_id, target)
                .await?;
            let mut data = GrantData::empty(player_id);
            data.changed_hero_ids.push(hero_id);
            send_snapshot_pushes(state, &data).await?;
            Ok(GmResponse::ok_data(
                format!("set hero {hero_id} resonance to {target}"),
                update,
            ))
        }
        _ => anyhow::bail!(
            "usage: hero level <hero id> <level>, hero resonance <hero id> <level>, hero portrait <hero id> <1-5>, or hero portrait all"
        ),
    }
}

pub(super) async fn psychube(
    state: &'static AppState,
    player_id: i64,
    args: &[&str],
) -> Result<GmResponse> {
    let [max, uid] = args else {
        anyhow::bail!("usage: psychube max <equipment uid>");
    };
    if !max.eq_ignore_ascii_case("max") {
        anyhow::bail!("usage: psychube max <equipment uid>");
    }
    let uid = uid.parse::<i64>()?;
    let equip = InventoryManager::new(player_id)
        .max_equip(state.db, uid)
        .await?;
    send_push(
        state,
        player_id,
        CmdId::EquipUpdatePushCmd,
        EquipUpdatePush {
            equips: vec![equip.clone()],
        },
    )
    .await?;
    Ok(GmResponse::ok_data(format!("maxed psychube {uid}"), equip))
}

pub(super) async fn equipment_catalog(
    state: &AppState,
    player_id: i64,
) -> Result<EquipmentCatalog> {
    let heroes = UserHeroModel::new(player_id, (*state.db).clone())
        .get_all_heroes()
        .await?;
    let equipped = heroes
        .iter()
        .filter(|hero| hero.record.default_equip_uid > 0)
        .map(|hero| {
            let character = state.tables.character.get(hero.record.hero_id);
            let name = character.map_or_else(
                || hero.record.hero_id.to_string(),
                |character| {
                    resolve_name(
                        state.tables,
                        if character.name_eng.is_empty() {
                            &character.name
                        } else {
                            &character.name_eng
                        },
                    )
                },
            );
            (hero.record.default_equip_uid, (hero.record.hero_id, name))
        })
        .collect::<HashMap<_, _>>();
    let model = UserEquipmentModel::new(player_id, (*state.db).clone());
    let mut equipment = Vec::new();
    for owned in model.get_all().await? {
        let Some(config) = state
            .tables
            .equip
            .get(owned.equip_id)
            .filter(|config| state.tables.is_normal_equipment(config))
        else {
            continue;
        };
        let Some(maximum) = state.tables.max_equip_progression(config.rare) else {
            continue;
        };
        let max_refine_level = state
            .tables
            .equip_refine_cap(config)
            .unwrap_or(owned.refine_lv);
        let (hero_id, hero_name) = equipped
            .get(&owned.uid)
            .cloned()
            .map_or((None, None), |(id, name)| (Some(id), Some(name)));
        equipment.push(EquipmentEntry {
            uid: owned.uid,
            id: owned.equip_id,
            name: resolve_name(
                state.tables,
                if config.name_en.is_empty() {
                    &config.name
                } else {
                    &config.name_en
                },
            ),
            level: owned.level,
            max_level: maximum.level,
            break_level: owned.break_lv,
            max_break_level: maximum.break_level,
            refine_level: owned.refine_lv,
            max_refine_level,
            hero_id,
            hero_name,
        });
    }
    equipment
        .sort_unstable_by(|left, right| left.name.cmp(&right.name).then(left.uid.cmp(&right.uid)));
    Ok(EquipmentCatalog { equipment })
}

pub(super) fn event_catalog(state: &AppState) -> Result<EventCatalog> {
    let config = common::config::ServerConfig::load_or_create(&common::config_path())?;
    Ok(build_event_catalog(state.tables, &config.activity.schedule))
}

pub(super) fn update_event(
    state: &AppState,
    id: i32,
    enabled: bool,
    start_time: u64,
    end_time: u64,
) -> Result<EventEntry> {
    update_event_file(
        state.tables,
        &common::config_path(),
        id,
        enabled,
        start_time,
        end_time,
    )
}

fn update_event_file(
    tables: &config::GameDB,
    path: &std::path::Path,
    id: i32,
    enabled: bool,
    start_time: u64,
    end_time: u64,
) -> Result<EventEntry> {
    if start_time >= end_time {
        anyhow::bail!("event start time must be before end time");
    }
    let start_time_value = i64::try_from(start_time)?;
    let end_time_value = i64::try_from(end_time)?;
    let _guard = EVENT_CONFIG_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("event config lock is poisoned"))?;
    let content = std::fs::read_to_string(path)?;
    let mut document = content.parse::<DocumentMut>()?;
    let schedule = document["activity"]["schedule"]
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("activity.schedule must be an array"))?;
    let row = schedule
        .iter_mut()
        .filter_map(Value::as_inline_table_mut)
        .find(|row| row.get("id").and_then(Value::as_integer) == Some(id as i64))
        .ok_or_else(|| anyhow::anyhow!("event {id} is not present in Config.toml"))?;
    row.insert("enabled", enabled.into());
    row.insert("start_time", start_time_value.into());
    row.insert("end_time", end_time_value.into());

    let updated_content = document.to_string();
    let config: common::config::ServerConfig = toml::from_str(&updated_content)?;
    let updated = config
        .activity
        .schedule
        .iter()
        .find(|schedule| schedule.id == id)
        .map(|schedule| event_entry(tables, schedule))
        .ok_or_else(|| anyhow::anyhow!("event {id} is not present in Config.toml"))?;
    replace_file(path, updated_content.as_bytes())?;
    Ok(updated)
}

fn replace_file(path: &std::path::Path, contents: &[u8]) -> Result<()> {
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("config path has no file name"))?
        .to_string_lossy();
    let replacement = path.with_file_name(format!("{file_name}.tmp"));
    let mut file = std::fs::File::create(&replacement)?;
    file.write_all(contents)?;
    file.sync_all()?;
    drop(file);
    #[cfg(unix)]
    std::fs::set_permissions(&replacement, std::fs::metadata(path)?.permissions())?;

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;

        let destination = path
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let source = replacement
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let replaced = unsafe {
            ReplaceFileW(
                destination.as_ptr(),
                source.as_ptr(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null(),
            )
        };
        if replaced == 0 {
            let error = std::io::Error::last_os_error();
            let _ = std::fs::remove_file(&replacement);
            return Err(error.into());
        }
    }
    #[cfg(not(windows))]
    std::fs::rename(&replacement, path)?;

    Ok(())
}

fn build_event_catalog(
    tables: &config::GameDB,
    schedule: &[common::config::ActivitySchedule],
) -> EventCatalog {
    EventCatalog {
        events: schedule
            .iter()
            .map(|schedule| event_entry(tables, schedule))
            .collect(),
        restart_required: true,
    }
}

fn event_entry(tables: &config::GameDB, schedule: &common::config::ActivitySchedule) -> EventEntry {
    let name = tables.activity.get(schedule.id).map_or_else(
        || format!("Activity {}", schedule.id),
        |activity| {
            resolve_name(
                tables,
                if activity.name_en.is_empty() {
                    &activity.name
                } else {
                    &activity.name_en
                },
            )
        },
    );
    EventEntry {
        id: schedule.id,
        name,
        enabled: schedule.enabled,
        start_time: schedule.start_time,
        end_time: schedule.end_time,
    }
}

fn require_all(args: &[&str], usage: &str) -> Result<()> {
    if !matches!(args, [unlock, all] if unlock.eq_ignore_ascii_case("unlock") && all.eq_ignore_ascii_case("all"))
    {
        anyhow::bail!("usage: {usage}");
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub(super) struct EquipmentCatalog {
    equipment: Vec<EquipmentEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EventCatalog {
    events: Vec<EventEntry>,
    restart_required: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EventEntry {
    id: i32,
    name: String,
    enabled: bool,
    start_time: u64,
    end_time: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EquipmentEntry {
    uid: i64,
    id: i32,
    name: String,
    level: i32,
    max_level: i32,
    break_level: i32,
    max_break_level: i32,
    refine_level: i32,
    max_refine_level: i32,
    hero_id: Option<i32>,
    hero_name: Option<String>,
}

#[cfg(test)]
mod tests;
