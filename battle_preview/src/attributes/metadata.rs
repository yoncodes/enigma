use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use sonettobuf::{
    AssistHeroInfo, Equip, EquipDeletePush, EquipUpdatePush, GetEquipInfoReply, HeroInfo,
    HeroInfoListReply, HeroUpdatePush, RefreshAssistReply,
};

use super::{BuildMetadata, request::SelectedAssist};
use crate::normalize_live_json;

pub(super) fn resolve_build_metadata(
    path: &Path,
    selected_assist: Option<SelectedAssist>,
) -> anyhow::Result<(BuildMetadata, &'static str)> {
    if let Some(captured) = capture_build_metadata(path, selected_assist)? {
        return Ok((captured, "timeline"));
    }
    let mut roster_files = Vec::new();
    let mut update_files = Vec::new();
    if let Some(parent) = path.parent() {
        for path in fs::read_dir(parent)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
        {
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if name.starts_with("HeroInfoListReply") && name.ends_with(".json") {
                roster_files.push(path);
            } else if name.starts_with("HeroUpdatePush") && name.ends_with(".json") {
                update_files.push(path);
            }
        }
    }
    if !roster_files.is_empty() || !update_files.is_empty() {
        roster_files.sort();
        update_files.sort();
        let mut heroes = HashMap::new();
        for file in roster_files {
            apply_hero_roster(&file, &mut heroes)?;
        }
        for file in update_files {
            apply_hero_update(&file, &mut heroes)?;
        }
        return Ok((
            BuildMetadata {
                heroes,
                ..Default::default()
            },
            "local",
        ));
    }
    if let Some(files) = session_files(path)?
        && let Some(metadata) = replay_build_metadata(files, selected_assist)?
    {
        return Ok((metadata, "session"));
    }
    Ok((BuildMetadata::default(), "none"))
}

fn capture_build_metadata(
    path: &Path,
    selected_assist: Option<SelectedAssist>,
) -> anyhow::Result<Option<BuildMetadata>> {
    let Some(files) = capture_timeline_through(path)? else {
        return Ok(None);
    };
    replay_build_metadata(files, selected_assist)?
        .map(Some)
        .ok_or_else(|| anyhow::anyhow!("capture timeline has no hero roster before battle"))
}

/// Replays owned and assist build metadata in timeline order; `None` without a roster.
fn replay_build_metadata(
    files: Vec<PathBuf>,
    selected_assist: Option<SelectedAssist>,
) -> anyhow::Result<Option<BuildMetadata>> {
    let mut heroes = HashMap::new();
    let mut assists = BuildMetadata::default();
    let mut saw_roster = false;
    for file in files {
        let Some(name) = file.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.ends_with("_HeroInfoListReply.json") {
            saw_roster = true;
            heroes.clear();
            apply_hero_roster(&file, &mut heroes)?;
        } else if name.ends_with("_HeroUpdatePush.json") {
            apply_hero_update(&file, &mut heroes)?;
        } else if name.ends_with("_RefreshAssistReply.json") {
            assists = assist_build_metadata(&file, selected_assist)?;
        }
    }
    if !saw_roster {
        return Ok(None);
    }
    if let Some(selected) = selected_assist {
        let hero = assists.heroes.remove(&selected.hero_uid).ok_or_else(|| {
            anyhow::anyhow!(
                "selected assist {} is missing from the captured assist roster",
                selected.hero_uid
            )
        })?;
        if hero.user_id != selected.user_id {
            anyhow::bail!(
                "selected assist {} user mismatch: request={} metadata={}",
                selected.hero_uid,
                selected.user_id,
                hero.user_id,
            );
        }
        if heroes.contains_key(&selected.hero_uid) {
            anyhow::bail!(
                "selected assist {} conflicts with owned roster",
                selected.hero_uid
            );
        }
        let style = assists
            .talent_styles
            .remove(&selected.hero_uid)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "selected assist {} is missing talent style",
                    selected.hero_uid
                )
            })?;
        heroes.insert(selected.hero_uid, hero);
        assists.talent_styles.insert(selected.hero_uid, style);
    }
    Ok(Some(BuildMetadata {
        heroes,
        talent_styles: assists.talent_styles,
    }))
}

pub(super) fn captured_equipment_metadata(path: &Path) -> anyhow::Result<HashMap<i64, Equip>> {
    let Some(files) = capture_timeline_through(path)? else {
        return Ok(HashMap::new());
    };
    let mut equips = HashMap::new();
    let mut saw_inventory = false;
    for file in files {
        let Some(name) = file.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.ends_with("_GetEquipInfoReply.json") {
            let reply: GetEquipInfoReply = serde_json::from_value(read_capture_value(&file)?)?;
            equips.clear();
            saw_inventory = true;
            insert_captured_equipment(&mut equips, reply.equips)?;
        } else if saw_inventory && name.ends_with("_EquipUpdatePush.json") {
            let update: EquipUpdatePush = serde_json::from_value(read_capture_value(&file)?)?;
            insert_captured_equipment(&mut equips, update.equips)?;
        } else if saw_inventory && name.ends_with("_EquipDeletePush.json") {
            let deleted: EquipDeletePush = serde_json::from_value(read_capture_value(&file)?)?;
            for uid in deleted.uids {
                if uid <= 0 {
                    anyhow::bail!("capture equipment delete has invalid uid {uid}");
                }
                equips.remove(&uid);
            }
        }
    }
    Ok(equips)
}

fn read_capture_value(path: &Path) -> anyhow::Result<serde_json::Value> {
    let mut value: serde_json::Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    normalize_live_json(&mut value);
    Ok(value)
}

fn insert_captured_equipment(
    inventory: &mut HashMap<i64, Equip>,
    equips: Vec<Equip>,
) -> anyhow::Result<()> {
    for equip in equips {
        let uid = equip
            .uid
            .filter(|uid| *uid > 0)
            .ok_or_else(|| anyhow::anyhow!("capture equipment is missing a valid uid"))?;
        inventory.insert(uid, equip);
    }
    Ok(())
}

fn session_files(path: &Path) -> anyhow::Result<Option<Vec<PathBuf>>> {
    let Some(common) = path
        .ancestors()
        .map(|ancestor| ancestor.join("common"))
        .find(|candidate| candidate.is_dir())
    else {
        return Ok(None);
    };
    let mut files = fs::read_dir(common)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    files.sort();
    Ok(Some(files))
}

fn capture_timeline_through(path: &Path) -> anyhow::Result<Option<Vec<PathBuf>>> {
    let Some(mut files) = session_files(path)? else {
        return Ok(None);
    };

    // A packet read in place from the timeline is its own position, even when an
    // identical packet repeats elsewhere in the session.
    if let Some(index) = timeline_position(&files, path) {
        files.truncate(index + 1);
        return Ok(Some(files));
    }
    let target = fs::read(path)?;
    let command = capture_command(path)
        .ok_or_else(|| anyhow::anyhow!("capture packet has no command name"))?;
    let matches = matching_packets(&files, &command, &target)?;
    let target_index = match matches.as_slice() {
        // Older captures keep a shared timeline without this battle; their
        // build comes from the roster files saved next to the battle.
        [] => return Ok(None),
        [index] => *index,
        _ => resolve_repeated_packet(path, &files, &matches)?,
    };
    files.truncate(target_index + 1);
    Ok(Some(files))
}

fn timeline_position(timeline: &[PathBuf], path: &Path) -> Option<usize> {
    let name = path.file_name()?;
    let directory = fs::canonicalize(path.parent()?).ok()?;
    let timeline_directory = fs::canonicalize(timeline.first()?.parent()?).ok()?;
    (directory == timeline_directory)
        .then(|| {
            timeline
                .iter()
                .position(|file| file.file_name() == Some(name))
        })
        .flatten()
}

fn resolve_repeated_packet(
    path: &Path,
    timeline: &[PathBuf],
    matches: &[usize],
) -> anyhow::Result<usize> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("capture packet has no parent directory"))?;
    let mut resolved = HashSet::new();
    for anchor in fs::read_dir(parent)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|candidate| candidate.is_file() && candidate != path)
    {
        let Some(command) = capture_command(&anchor) else {
            continue;
        };
        if matches!(
            command.as_str(),
            "StartDungeonRequest" | "StartAct229BattleRequest"
        ) {
            continue;
        }
        let bytes = fs::read(&anchor)?;
        let anchor_matches = matching_packets(timeline, &command, &bytes)?;
        let [anchor_index] = anchor_matches.as_slice() else {
            continue;
        };
        for target_index in matches.iter().copied() {
            let next_start = timeline
                .iter()
                .enumerate()
                .skip(target_index + 1)
                .find(|(_, path)| is_battle_start_packet(path))
                .map(|(index, _)| index);
            if target_index <= *anchor_index
                && next_start.is_none_or(|next_start| *anchor_index < next_start)
            {
                resolved.insert(target_index);
            }
        }
    }
    match resolved.into_iter().collect::<Vec<_>>().as_slice() {
        [index] => Ok(*index),
        _ => anyhow::bail!("capture packet matches multiple timeline positions"),
    }
}

pub(super) fn is_battle_start_packet(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.ends_with("_StartDungeonReply.json")
                || name.ends_with("_StartTowerBattleReply.json")
                || name.ends_with("_StartAct229BattleReply.json")
        })
}

fn matching_packets(
    timeline: &[PathBuf],
    command: &str,
    expected: &[u8],
) -> anyhow::Result<Vec<usize>> {
    let suffix = format!("_{command}.json");
    timeline
        .iter()
        .enumerate()
        .filter(|(_, path)| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(&suffix))
        })
        .filter_map(|(index, path)| match fs::read(path) {
            Ok(bytes) if bytes == expected => Some(Ok(index)),
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

pub(super) fn capture_command(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    if stem.starts_with("begin_round_") {
        return Some("BeginRoundReply".to_owned());
    }
    // Session timeline packets are `<date>_<time>_<millis>_<sequence>_<Command>`.
    let parts = stem.splitn(5, '_').collect::<Vec<_>>();
    if let [date, time, millis, sequence, command] = parts.as_slice()
        && [date, time, millis, sequence]
            .iter()
            .all(|part| part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Some((*command).to_owned());
    }
    Some(stem.split('_').next()?.to_owned())
}

fn apply_hero_roster(path: &Path, heroes: &mut HashMap<i64, HeroInfo>) -> anyhow::Result<()> {
    let mut value: serde_json::Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    normalize_live_json(&mut value);
    let roster: HeroInfoListReply = serde_json::from_value(value)?;
    for hero in roster.heros {
        heroes.insert(hero.uid, hero);
    }
    Ok(())
}

fn apply_hero_update(path: &Path, heroes: &mut HashMap<i64, HeroInfo>) -> anyhow::Result<()> {
    let mut value: serde_json::Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    normalize_live_json(&mut value);
    let update: HeroUpdatePush = serde_json::from_value(value)?;
    for hero in update.hero_updates {
        heroes.insert(hero.uid, hero);
    }
    Ok(())
}

fn assist_build_metadata(
    path: &Path,
    selected_assist: Option<SelectedAssist>,
) -> anyhow::Result<BuildMetadata> {
    let Some(selected) = selected_assist else {
        return Ok(BuildMetadata::default());
    };
    let reply: RefreshAssistReply = serde_json::from_value(read_capture_value(path)?)?;
    let mut metadata = BuildMetadata::default();
    let mut uid_match = false;
    let mut selected_hero = None;
    for assist in reply
        .assist_hero_careers
        .into_iter()
        .flat_map(|career| career.assist_hero_infos)
    {
        if assist.hero_uid != Some(selected.hero_uid) {
            continue;
        }
        uid_match = true;
        if assist.user_id == Some(selected.user_id) && selected_hero.replace(assist).is_some() {
            anyhow::bail!(
                "selected assist {} has duplicate metadata for user {}",
                selected.hero_uid,
                selected.user_id,
            );
        }
    }
    let Some(assist) = selected_hero else {
        if uid_match {
            anyhow::bail!(
                "selected assist {} user mismatch: request={}",
                selected.hero_uid,
                selected.user_id,
            );
        }
        return Ok(metadata);
    };
    let (hero, style) = assist_hero_build(assist)?;
    metadata.talent_styles.insert(hero.uid, style);
    metadata.heroes.insert(hero.uid, hero);
    Ok(metadata)
}

fn assist_hero_build(assist: AssistHeroInfo) -> anyhow::Result<(HeroInfo, i32)> {
    let uid = assist
        .hero_uid
        .filter(|uid| *uid > 0)
        .ok_or_else(|| anyhow::anyhow!("assist hero is missing a valid uid"))?;
    let user_id = assist
        .user_id
        .filter(|user_id| *user_id > 0)
        .ok_or_else(|| anyhow::anyhow!("assist hero {uid} is missing a valid user id"))?;
    let hero_id = assist
        .hero_id
        .filter(|hero_id| *hero_id > 0)
        .ok_or_else(|| anyhow::anyhow!("assist hero {uid} is missing a valid hero id"))?;
    let style = assist
        .style
        .filter(|style| *style >= 0)
        .ok_or_else(|| anyhow::anyhow!("assist hero {uid} is missing a valid talent style"))?;
    Ok((
        HeroInfo {
            uid,
            user_id,
            hero_id,
            level: assist.level,
            rank: assist.rank,
            skin: assist.skin,
            passive_skill_level: assist.passive_skill_level,
            ex_skill_level: assist.ex_skill_level,
            talent: assist.talent,
            talent_cube_infos: assist.talent_cube_infos,
            destiny_rank: assist.destiny_rank,
            destiny_level: assist.destiny_level,
            destiny_stone: assist.destiny_stone,
            extra_str: assist.extra_str,
            ..Default::default()
        },
        style,
    ))
}
