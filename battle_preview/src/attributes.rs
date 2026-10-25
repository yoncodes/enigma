use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use battle::tooling::{
    damage_tracing_enabled,
    entity::{BattleBalance, EntityBuilder, EquipmentBuildInput, HeroBuildInput, Stats},
};
use sonettobuf::{
    Equip, EquipDeletePush, EquipUpdatePush, Fight, FightEntityInfo, GetEquipInfoReply,
    HeroExAttribute, HeroInfo, HeroInfoListReply, HeroSpAttribute, HeroUpdatePush,
};

use crate::normalize_live_json;

type PreviewAttributes = (Vec<(i64, HeroExAttribute)>, Vec<(i64, HeroSpAttribute)>);

pub fn preview_attributes(fight: &Fight, battle_path: &Path) -> anyhow::Result<PreviewAttributes> {
    preview_attributes_with_request(
        fight,
        battle_path,
        folder_request_path(battle_path).as_deref(),
    )
}

/// `request_path` is the start request of this battle, when one was captured.
pub fn preview_attributes_with_request(
    fight: &Fight,
    battle_path: &Path,
    request_path: Option<&Path>,
) -> anyhow::Result<PreviewAttributes> {
    let metadata = battle_build_metadata(battle_path)?;
    let request = request_path
        .map(request_metadata)
        .transpose()?
        .unwrap_or_default();
    let battle_balance = request_battle_balance(fight, request.is_balance)?;
    let needs_captured_equipment = fight.attacker.iter().any(|team| {
        team.entitys
            .iter()
            .chain(&team.sub_entitys)
            .any(|entity| entity.equips.is_empty() && entity.equip_uid.is_some_and(|uid| uid > 0))
    });
    let captured_equipment = if needs_captured_equipment {
        captured_equipment_metadata(battle_path)?
    } else {
        HashMap::new()
    };
    let mut ex_attributes = Vec::new();
    let mut sp_attributes = Vec::new();

    for entity in fight
        .attacker
        .iter()
        .flat_map(|team| team.entitys.iter().chain(team.sub_entitys.iter()))
    {
        let uid = entity
            .uid
            .ok_or_else(|| anyhow::anyhow!("attacker is missing uid"))?;
        let model_id = entity
            .model_id
            .filter(|model_id| *model_id > 0)
            .ok_or_else(|| anyhow::anyhow!("attacker {uid} is missing model id"))?;

        if let Some((trial, stats)) = configured_trial(entity)? {
            ex_attributes.push((uid, stats.ex()));
            sp_attributes.push((uid, stats.sp()));
            if damage_tracing_enabled() {
                eprintln!(
                    "attribute preview uid={uid} hero={} source=configured-trial",
                    trial.model_id.unwrap_or_default(),
                );
            }
            continue;
        }

        let hero = metadata.get(&uid).ok_or_else(|| {
            anyhow::anyhow!("attribute preview missing build metadata uid={uid} hero={model_id}")
        })?;
        let build = preview_build_input(entity, hero)?;
        let equips = validated_equipment_loadout(
            entity,
            hero,
            request.selected_equips.get(&uid).copied(),
            entity
                .equip_uid
                .and_then(|equip_uid| captured_equipment.get(&equip_uid)),
        )
        .map_err(|()| {
            anyhow::anyhow!(
                "attribute preview has invalid equipment metadata uid={uid} hero={model_id}",
            )
        })?;
        let stats = battle_balance.map_or_else(
            || Stats::build_for_captured_loadout(&build, &equips),
            |balance| balance.stats_for(&build, &equips),
        );

        if damage_tracing_enabled() {
            eprintln!(
                "attribute preview uid={uid} hero={model_id} source=validated-build build={build:?} stats={stats:?}",
            );
        }
        ex_attributes.push((uid, stats.ex()));
        sp_attributes.push((uid, stats.sp()));
    }

    Ok((ex_attributes, sp_attributes))
}

/// Rebuilds each captured attacker from its build inputs and lists where the
/// engine's loadout differs from the captured one.
pub fn loadout_diffs(fight: &Fight, battle_path: &Path) -> anyhow::Result<Vec<String>> {
    let metadata = battle_build_metadata(battle_path)?;
    let battle_balance =
        request_battle_balance(fight, battle_request_metadata(battle_path)?.is_balance)?;
    let mut diffs = Vec::new();
    let teams = fight.attacker.iter().flat_map(|team| {
        team.entitys
            .iter()
            .map(|entity| (entity, false))
            .chain(team.sub_entitys.iter().map(|entity| (entity, true)))
    });
    for (entity, is_sub) in teams {
        let uid = entity
            .uid
            .ok_or_else(|| anyhow::anyhow!("attacker is missing uid"))?;
        let built = if let Some((trial, _)) = configured_trial(entity)? {
            trial
        } else {
            let hero = metadata.get(&uid).ok_or_else(|| {
                anyhow::anyhow!("loadout preview missing build metadata uid={uid}")
            })?;
            let build = preview_build_input(entity, hero)?;
            let mut builder = EntityBuilder::new(
                build.clone(),
                entity.position.unwrap_or_default(),
                entity.team_type.unwrap_or_default(),
                is_sub,
            );
            // Balance rewrites rank, which picks the kit and resource type.
            if let Some(balance) = battle_balance {
                builder = builder.with_balance(balance, balance.stats_for(&build, &[]));
            }
            builder.build()?
        };
        let fields = [
            (
                "skillGroup1",
                format!("{:?}", built.skill_group1),
                format!("{:?}", entity.skill_group1),
            ),
            (
                "skillGroup2",
                format!("{:?}", built.skill_group2),
                format!("{:?}", entity.skill_group2),
            ),
            (
                "exSkill",
                format!("{}", built.ex_skill.unwrap_or_default()),
                format!("{}", entity.ex_skill.unwrap_or_default()),
            ),
            (
                "exPointType",
                format!("{}", built.ex_point_type.unwrap_or_default()),
                format!("{}", entity.ex_point_type.unwrap_or_default()),
            ),
        ];
        for (field, built, captured) in fields {
            if built != captured {
                diffs.push(format!(
                    "uid={uid} hero={} {field} built={built} captured={captured}",
                    entity.model_id.unwrap_or_default()
                ));
            }
        }
    }
    Ok(diffs)
}

#[derive(Debug, Default)]
struct BattleRequestMetadata {
    is_balance: bool,
    selected_equips: HashMap<i64, i64>,
}

fn folder_request_path(battle_path: &Path) -> Option<PathBuf> {
    let parent = battle_path.parent()?;
    ["StartDungeonRequest.json", "StartAct229BattleRequest.json"]
        .map(|name| parent.join(name))
        .into_iter()
        .find(|path| path.exists())
}

fn battle_request_metadata(battle_path: &Path) -> anyhow::Result<BattleRequestMetadata> {
    folder_request_path(battle_path)
        .map(|path| request_metadata(&path))
        .transpose()
        .map(Option::unwrap_or_default)
}

fn request_metadata(request_path: &Path) -> anyhow::Result<BattleRequestMetadata> {
    let request: serde_json::Value = serde_json::from_str(&fs::read_to_string(request_path)?)?;
    let request = request
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("invalid dungeon request"))?;
    let request = if capture_command(request_path).as_deref() == Some("StartAct229BattleRequest") {
        request
            .get("startDungeonRequest")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| anyhow::anyhow!("invalid activity startDungeonRequest"))?
    } else {
        request
    };
    let is_balance = request
        .get("isBalance")
        .or_else(|| request.get("is_balance"))
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| anyhow::anyhow!("invalid isBalance value"))
        })
        .transpose()?
        .unwrap_or_default();
    let selections = match request
        .get("fightGroup")
        .or_else(|| request.get("fight_group"))
    {
        Some(group) => match group
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("invalid fight group"))?
            .get("equips")
        {
            Some(equips) => equips
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("invalid equipment selections"))?
                .as_slice(),
            None => &[],
        },
        None => &[],
    };
    let mut selected_equips = HashMap::new();
    let mut selected_heroes = std::collections::HashSet::new();
    for selection in selections {
        let selection = selection
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("invalid equipment selection"))?;
        let hero_uid = json_i64(
            selection
                .get("heroUid")
                .or_else(|| selection.get("hero_uid"))
                .ok_or_else(|| anyhow::anyhow!("equipment selection is missing hero uid"))?,
        )?;
        if hero_uid < 0 {
            anyhow::bail!("equipment selection has invalid hero uid {hero_uid}");
        }
        let equip_uids = selection
            .get("equipUid")
            .or_else(|| selection.get("equip_uid"))
            .map(|uids| {
                uids.as_array()
                    .ok_or_else(|| anyhow::anyhow!("invalid equipment uids for hero {hero_uid}"))
            })
            .transpose()?
            .into_iter()
            .flatten()
            .map(json_i64)
            .collect::<anyhow::Result<Vec<_>>>()?;
        if equip_uids.iter().any(|uid| *uid < 0) {
            anyhow::bail!("negative equipment uid selected for hero {hero_uid}");
        }
        if hero_uid == 0 {
            continue;
        }
        if !selected_heroes.insert(hero_uid) {
            anyhow::bail!("duplicate equipment selection for hero {hero_uid}");
        }
        let mut equip_uids = equip_uids.into_iter().filter(|uid| *uid != 0);
        let Some(equip_uid) = equip_uids.next() else {
            continue;
        };
        if equip_uids.any(|uid| uid != equip_uid) {
            anyhow::bail!("multiple primary equipment selections for hero {hero_uid}");
        }
        selected_equips.insert(hero_uid, equip_uid);
    }
    Ok(BattleRequestMetadata {
        is_balance,
        selected_equips,
    })
}

fn json_i64(value: &serde_json::Value) -> anyhow::Result<i64> {
    value
        .as_i64()
        .or_else(|| value.as_str().and_then(|raw| raw.parse().ok()))
        .ok_or_else(|| anyhow::anyhow!("invalid integer value"))
}

fn request_battle_balance(
    fight: &Fight,
    is_balance: bool,
) -> anyhow::Result<Option<BattleBalance>> {
    if !is_balance {
        return Ok(None);
    }
    let battle_id = fight
        .battle_id
        .filter(|battle_id| *battle_id > 0)
        .ok_or_else(|| anyhow::anyhow!("balanced request missing battle id"))?;
    let battle = config::configs::get()
        .battle
        .get(battle_id)
        .ok_or_else(|| anyhow::anyhow!("unknown battle {battle_id}"))?;
    BattleBalance::parse(&battle.balance)
        .map(Some)
        .ok_or_else(|| anyhow::anyhow!("invalid balance config for battle {battle_id}"))
}

fn configured_trial(entity: &FightEntityInfo) -> anyhow::Result<Option<(FightEntityInfo, Stats)>> {
    let Some(trial_id) = entity.trial_id.filter(|trial_id| *trial_id > 0) else {
        return Ok(None);
    };
    let uid = entity
        .uid
        .ok_or_else(|| anyhow::anyhow!("trial {trial_id} is missing attacker uid"))?;
    let (trial, stats) = EntityBuilder::trial(
        trial_id,
        uid,
        entity.position.unwrap_or_default(),
        entity.team_type.unwrap_or_default(),
    )
    .map_err(|error| anyhow::anyhow!("invalid trial {trial_id}: {error}"))?;
    if trial.model_id != entity.model_id {
        anyhow::bail!(
            "trial {trial_id} hero mismatch: configured={} fight={}",
            trial.model_id.unwrap_or_default(),
            entity.model_id.unwrap_or_default(),
        );
    }
    Ok(Some((trial, stats)))
}

fn validated_equipment_loadout(
    entity: &FightEntityInfo,
    hero: &HeroInfo,
    requested_equip_uid: Option<i64>,
    captured_equip: Option<&Equip>,
) -> Result<Vec<EquipmentBuildInput>, ()> {
    if requested_equip_uid.is_some_and(|uid| uid < 0)
        || hero.default_equip_uid.is_some_and(|uid| uid < 0)
        || entity.equip_uid.is_some_and(|uid| uid < 0)
    {
        return Err(());
    }
    let expected_equip_uid =
        requested_equip_uid.or_else(|| hero.default_equip_uid.filter(|uid| *uid > 0));
    if entity.equips.is_empty() {
        return match (
            expected_equip_uid,
            entity.equip_uid.filter(|uid| *uid > 0),
            captured_equip,
        ) {
            (None, None, None) => Ok(Vec::new()),
            (Some(expected), Some(selected), Some(equip))
                if expected == selected && equip.uid == Some(selected) =>
            {
                let input = validated_captured_equipment(equip)?;
                if config::configs::get()
                    .linked_psychube_id(hero.hero_id, input.equip_id)
                    .is_some()
                {
                    return Err(());
                }
                Ok(vec![input])
            }
            _ => Err(()),
        };
    }
    if entity.model_id.filter(|model_id| *model_id > 0) != Some(hero.hero_id) {
        return Err(());
    }
    let selected_equip_uid = entity.equip_uid.filter(|uid| *uid > 0).ok_or(())?;
    if Some(selected_equip_uid) != expected_equip_uid {
        return Err(());
    }
    let primary = entity.equips.first().ok_or(())?;
    if primary.equip_uid != Some(selected_equip_uid) || entity.equips.len() > 2 {
        return Err(());
    }

    let game = config::configs::get();
    let mut equip_uids = HashSet::with_capacity(entity.equips.len());
    for equip in &entity.equips {
        let uid = equip.equip_uid.filter(|uid| *uid > 0).ok_or(())?;
        if equip.break_lv.is_some_and(|level| level < 0) || !equip_uids.insert(uid) {
            return Err(());
        }
    }
    let primary_equip_id = primary.equip_id.filter(|id| *id > 0).ok_or(())?;
    let primary_equipment = game.equip.get(primary_equip_id).ok_or(())?;
    let primary_level = primary.equip_lv.filter(|level| *level > 0).ok_or(())?;
    let primary_break_level = primary.break_lv.unwrap_or_default();
    game.equip_strengthen_cost(primary_equipment.rare, primary_level)
        .ok_or(())?;
    game.equip_break_cost(primary_equipment.rare, primary_break_level)
        .ok_or(())?;
    let primary = EquipmentBuildInput {
        uid: selected_equip_uid,
        equip_id: primary_equip_id,
        level: primary_level,
        break_level: primary_break_level,
        refine_level: primary.refine_lv.unwrap_or_default(),
    };

    let linked_equip_id = game.linked_psychube_id(hero.hero_id, primary_equip_id);
    let supplemental = entity
        .equips
        .get(1)
        .map(|equip| {
            let uid = equip.equip_uid.filter(|uid| *uid > 0).ok_or(())?;
            let equip_id = equip.equip_id.filter(|id| *id > 0).ok_or(())?;
            if Some(equip_id) != linked_equip_id {
                return Err(());
            }
            let equipment = game.equip.get(equip_id).ok_or(())?;
            let level = equip.equip_lv.filter(|level| *level > 0).ok_or(())?;
            let break_level = equip.break_lv.unwrap_or_default();
            game.equip_strengthen_cost(equipment.rare, level)
                .ok_or(())?;
            game.equip_break_cost(equipment.rare, break_level)
                .ok_or(())?;
            Ok(EquipmentBuildInput {
                uid,
                equip_id,
                level,
                break_level,
                refine_level: equip.refine_lv.unwrap_or_default(),
            })
        })
        .transpose()?;
    Ok(std::iter::once(primary).chain(supplemental).collect())
}

fn validated_captured_equipment(equip: &Equip) -> Result<EquipmentBuildInput, ()> {
    let uid = equip.uid.filter(|uid| *uid > 0).ok_or(())?;
    let equip_id = equip.equip_id.filter(|id| *id > 0).ok_or(())?;
    let level = equip.level.filter(|level| *level > 0).ok_or(())?;
    let break_level = equip.break_lv.unwrap_or_default();
    let refine_level = equip.refine_lv.unwrap_or_default();
    if break_level < 0 || refine_level < 0 {
        return Err(());
    }
    let configured = config::configs::get().equip.get(equip_id).ok_or(())?;
    config::configs::get()
        .equip_strengthen_cost(configured.rare, level)
        .ok_or(())?;
    config::configs::get()
        .equip_break_cost(configured.rare, break_level)
        .ok_or(())?;
    Ok(EquipmentBuildInput {
        uid,
        equip_id,
        level,
        break_level,
        refine_level,
    })
}

/// Where attacker builds come from: the capture timeline; the roster files saved
/// next to the battle; or, lowest confidence, the whole session's rosters and
/// updates (which may include changes made after the battle).
pub fn build_metadata_source(battle_path: &Path) -> anyhow::Result<&'static str> {
    Ok(resolve_build_metadata(battle_path)?.1)
}

fn battle_build_metadata(path: &Path) -> anyhow::Result<HashMap<i64, HeroInfo>> {
    Ok(resolve_build_metadata(path)?.0)
}

fn resolve_build_metadata(path: &Path) -> anyhow::Result<(HashMap<i64, HeroInfo>, &'static str)> {
    if let Some(captured) = capture_build_metadata(path)? {
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
        return Ok((heroes, "local"));
    }
    if let Some(files) = session_files(path)?
        && let Some(heroes) = replay_rosters(files)?
    {
        return Ok((heroes, "session"));
    }
    Ok((HashMap::new(), "none"))
}

fn capture_build_metadata(path: &Path) -> anyhow::Result<Option<HashMap<i64, HeroInfo>>> {
    let Some(files) = capture_timeline_through(path)? else {
        return Ok(None);
    };
    replay_rosters(files)?
        .map(Some)
        .ok_or_else(|| anyhow::anyhow!("capture timeline has no hero roster before battle"))
}

/// Replays rosters and hero updates in timeline order; `None` without a roster.
fn replay_rosters(files: Vec<PathBuf>) -> anyhow::Result<Option<HashMap<i64, HeroInfo>>> {
    let mut heroes = HashMap::new();
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
        }
    }
    Ok(saw_roster.then_some(heroes))
}

fn captured_equipment_metadata(path: &Path) -> anyhow::Result<HashMap<i64, Equip>> {
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

fn is_battle_start_packet(path: &Path) -> bool {
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

fn capture_command(path: &Path) -> Option<String> {
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

fn preview_build_input(
    entity: &FightEntityInfo,
    hero: &HeroInfo,
) -> anyhow::Result<HeroBuildInput> {
    let model_id = entity
        .model_id
        .filter(|model_id| *model_id > 0)
        .ok_or_else(|| anyhow::anyhow!("attacker is missing model id"))?;
    if hero.hero_id != model_id {
        anyhow::bail!(
            "attribute preview build mismatch uid={} fight hero={} metadata hero={}",
            entity.uid.unwrap_or_default(),
            model_id,
            hero.hero_id,
        );
    }
    let level = entity
        .level
        .or(hero.level)
        .filter(|level| *level > 0)
        .ok_or_else(|| anyhow::anyhow!("attribute preview missing hero level for {model_id}"))?;
    let talent = hero
        .talent
        .filter(|talent| *talent >= 0)
        .ok_or_else(|| anyhow::anyhow!("attribute preview missing talent for {model_id}"))?;
    let selected_template = hero.use_talent_template_id.unwrap_or_default();
    let template = if selected_template == 0 {
        None
    } else {
        Some(
            hero.talent_templates
                .iter()
                .find(|template| template.id == Some(selected_template))
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "attribute preview missing selected talent template {selected_template} for {model_id}"
                    )
                })?,
        )
    };
    let cubes = template
        .filter(|template| !template.talent_cube_infos.is_empty())
        .map(|template| template.talent_cube_infos.as_slice())
        .unwrap_or(&hero.talent_cube_infos);
    let talent_placements = cubes
        .iter()
        .map(|cube| {
            cube.cube_id
                .filter(|cube_id| *cube_id > 0)
                .ok_or_else(|| anyhow::anyhow!("invalid talent placement for {model_id}"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let rank = hero
        .rank
        .filter(|rank| *rank >= 0)
        .ok_or_else(|| anyhow::anyhow!("attribute preview missing rank for {model_id}"))?;
    Ok(HeroBuildInput {
        uid: entity.uid.unwrap_or_default(),
        user_id: hero.user_id,
        hero_id: model_id,
        skin: entity.skin.unwrap_or_default(),
        level,
        rank,
        ex_skill_level: entity.ex_skill_level.unwrap_or_default(),
        talent,
        talent_style: template
            .and_then(|template| template.style)
            .unwrap_or_default(),
        talent_placements,
        destiny_rank: entity
            .destiny_rank
            .or(hero.destiny_rank)
            .unwrap_or_default(),
        destiny_stone: entity.destiny_stone.unwrap_or_default(),
        extra_str: hero.extra_str.clone().unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests;
