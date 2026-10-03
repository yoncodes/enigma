use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use battle::engine::entity::{
    input::{EquipmentBuildInput, HeroBuildInput},
    stats::{BattleBalance, Stats},
};
use sonettobuf::{
    Fight, FightEntityInfo, HeroExAttribute, HeroInfo, HeroInfoListReply, HeroSpAttribute,
    HeroUpdatePush,
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
            if battle::engine::diagnostics::enabled(battle::engine::diagnostics::TraceArea::Damage)
            {
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
        let equips =
            validated_equipment_loadout(entity, hero, request.selected_equips.get(&uid).copied())
                .map_err(|()| {
                anyhow::anyhow!(
                    "attribute preview has invalid equipment metadata uid={uid} hero={model_id}",
                )
            })?;
        let stats = battle_balance.map_or_else(
            || Stats::build_for_loadout(&build, &equips),
            |balance| balance.stats_for(&build, &equips),
        );

        if battle::engine::diagnostics::enabled(battle::engine::diagnostics::TraceArea::Damage) {
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
            let mut builder = battle::engine::entity::builder::EntityBuilder::new(
                build.clone(),
                entity.position.unwrap_or_default(),
                entity.team_type.unwrap_or_default(),
                is_sub,
            );
            // Balance rewrites rank, which picks the kit and resource type.
            if let Some(balance) = battle_balance {
                builder = builder.with_balance(balance, balance.stats_for(&build, &[]));
            }
            builder.build()
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
    Some(battle_path.parent()?.join("StartDungeonRequest.json")).filter(|path| path.exists())
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

fn configured_trial(
    entity: &FightEntityInfo,
) -> anyhow::Result<Option<(FightEntityInfo, battle::engine::entity::stats::Stats)>> {
    let Some(trial_id) = entity.trial_id.filter(|trial_id| *trial_id > 0) else {
        return Ok(None);
    };
    let uid = entity
        .uid
        .ok_or_else(|| anyhow::anyhow!("trial {trial_id} is missing attacker uid"))?;
    let (trial, stats) = battle::engine::entity::builder::EntityBuilder::trial(
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
        return match (expected_equip_uid, entity.equip_uid.filter(|uid| *uid > 0)) {
            (None, None) => Ok(Vec::new()),
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
        if !equip_uids.insert(uid) {
            return Err(());
        }
    }
    let primary_equip_id = primary.equip_id.filter(|id| *id > 0).ok_or(())?;
    let primary_equipment = game.equip.get(primary_equip_id).ok_or(())?;
    let primary_level = primary.equip_lv.filter(|level| *level > 0).ok_or(())?;
    game.equip_strengthen_cost(primary_equipment.rare, primary_level)
        .ok_or(())?;
    let primary = EquipmentBuildInput {
        uid: selected_equip_uid,
        equip_id: primary_equip_id,
        level: primary_level,
        break_level: 0,
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
            game.equip_strengthen_cost(equipment.rare, level)
                .ok_or(())?;
            Ok(EquipmentBuildInput {
                uid,
                equip_id,
                level,
                break_level: 0,
                refine_level: equip.refine_lv.unwrap_or_default(),
            })
        })
        .transpose()?;
    Ok(std::iter::once(primary).chain(supplemental).collect())
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
        if command == "StartDungeonRequest" {
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
mod tests {
    use super::*;
    use sonettobuf::{EquipRecord, FightTeam, TalentCubeInfo};

    fn test_directory(label: &str) -> std::path::PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "enigma-preview-attributes-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        directory
    }

    fn fight(uid: i64) -> Fight {
        Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(uid),
                    model_id: Some(3149),
                    level: Some(60),
                    equip_uid: Some(100),
                    equips: vec![
                        EquipRecord {
                            equip_uid: Some(100),
                            equip_id: Some(1571),
                            equip_lv: Some(60),
                            ..Default::default()
                        },
                        EquipRecord {
                            equip_uid: Some(200),
                            equip_id: Some(1572),
                            equip_lv: Some(60),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn hero(uid: i64) -> HeroInfo {
        HeroInfo {
            uid,
            hero_id: 3149,
            level: Some(60),
            rank: Some(5),
            talent: Some(10),
            talent_cube_infos: vec![TalentCubeInfo {
                cube_id: Some(61),
                ..Default::default()
            }],
            default_equip_uid: Some(100),
            ex_attr: Some(HeroExAttribute {
                cri: Some(i32::MAX),
                ..Default::default()
            }),
            sp_attr: Some(HeroSpAttribute {
                heal: Some(i32::MAX),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn write_roster(directory: &Path, hero: HeroInfo) {
        fs::write(
            directory.join("HeroInfoListReply_1.json"),
            serde_json::to_vec(&HeroInfoListReply {
                heros: vec![hero],
                ..Default::default()
            })
            .unwrap(),
        )
        .unwrap();
    }

    fn write_request(directory: &Path, equips: Vec<sonettobuf::FightEquip>, is_balance: bool) {
        fs::write(
            directory.join("StartDungeonRequest.json"),
            serde_json::to_vec(&sonettobuf::StartDungeonRequest {
                fight_group: Some(sonettobuf::FightGroup {
                    equips,
                    ..Default::default()
                }),
                is_balance: Some(is_balance),
                ..Default::default()
            })
            .unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn captured_derived_attributes_are_not_runtime_inputs() {
        crate::init_test_config();
        let uid = 42;
        let directory = test_directory("derived-poison");
        let hero = hero(uid);
        let fight = fight(uid);
        write_roster(&directory, hero.clone());
        let entity = &fight.attacker.as_ref().unwrap().entitys[0];
        let build = preview_build_input(entity, &hero).unwrap();
        let equips = validated_equipment_loadout(entity, &hero, None).unwrap();
        let expected = Stats::build_for_loadout(&build, &equips);

        let (ex, sp) =
            preview_attributes(&fight, &directory.join("BeginRoundReply_1.json")).unwrap();

        assert_eq!(ex, vec![(uid, expected.ex())]);
        assert_eq!(sp, vec![(uid, expected.sp())]);
        assert_ne!(expected.cri, i32::MAX);
        assert_ne!(expected.heal, i32::MAX);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn explicit_request_equipment_overrides_the_roster_default() {
        crate::init_test_config();
        let uid = 42;
        let directory = test_directory("requested-equipment");
        let mut hero = hero(uid);
        hero.default_equip_uid = Some(999);
        write_roster(&directory, hero);
        write_request(
            &directory,
            vec![sonettobuf::FightEquip {
                hero_uid: Some(uid),
                equip_uid: vec![100],
                ..Default::default()
            }],
            false,
        );

        assert!(preview_attributes(&fight(uid), &directory.join("BeginRoundReply_1.json")).is_ok());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn request_and_entity_equipment_mismatch_fails_loudly() {
        crate::init_test_config();
        let uid = 42;
        let directory = test_directory("request-mismatch");
        write_roster(&directory, hero(uid));
        write_request(
            &directory,
            vec![sonettobuf::FightEquip {
                hero_uid: Some(uid),
                equip_uid: vec![999],
                ..Default::default()
            }],
            false,
        );

        assert!(
            preview_attributes(&fight(uid), &directory.join("BeginRoundReply_1.json"))
                .unwrap_err()
                .to_string()
                .contains("invalid equipment metadata")
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn request_without_equipment_uses_the_roster_default() {
        crate::init_test_config();
        let uid = 42;
        let directory = test_directory("request-default");
        write_roster(&directory, hero(uid));
        write_request(&directory, Vec::new(), false);

        assert!(preview_attributes(&fight(uid), &directory.join("BeginRoundReply_1.json")).is_ok());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn negative_equipment_metadata_fails_loudly() {
        crate::init_test_config();
        let uid = 42;
        let mut fight = fight(uid);
        let entity = &mut fight.attacker.as_mut().unwrap().entitys[0];
        entity.equips.clear();
        entity.equip_uid = Some(-1);
        let mut hero = hero(uid);
        hero.default_equip_uid = Some(-1);

        assert!(validated_equipment_loadout(entity, &hero, None).is_err());
    }

    #[test]
    fn string_encoded_request_equipment_is_accepted() {
        crate::init_test_config();
        let uid = 42;
        let directory = test_directory("string-request-equipment");
        let mut hero = hero(uid);
        hero.default_equip_uid = Some(999);
        write_roster(&directory, hero);
        fs::write(
            directory.join("StartDungeonRequest.json"),
            serde_json::to_vec(&serde_json::json!({
                "fightGroup": {
                    "equips": [{
                        "heroUid": uid.to_string(),
                        "equipUid": ["100"]
                    }]
                }
            }))
            .unwrap(),
        )
        .unwrap();

        assert!(preview_attributes(&fight(uid), &directory.join("BeginRoundReply_1.json")).is_ok());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn snake_case_balance_metadata_is_preserved() {
        let directory = test_directory("snake-balance");
        fs::write(
            directory.join("StartDungeonRequest.json"),
            serde_json::to_vec(&serde_json::json!({ "is_balance": true })).unwrap(),
        )
        .unwrap();

        assert!(
            battle_request_metadata(&directory.join("BeginRoundReply_1.json"))
                .unwrap()
                .is_balance
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn negative_request_equipment_fails_loudly() {
        let directory = test_directory("negative-request-equipment");
        fs::write(
            directory.join("StartDungeonRequest.json"),
            serde_json::to_vec(&serde_json::json!({
                "fightGroup": {
                    "equips": [{ "heroUid": 42, "equipUid": [-1] }]
                }
            }))
            .unwrap(),
        )
        .unwrap();

        assert!(
            battle_request_metadata(&directory.join("BeginRoundReply_1.json"))
                .unwrap_err()
                .to_string()
                .contains("negative equipment uid")
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn zero_owner_equipment_rows_are_validated_then_ignored() {
        let directory = test_directory("zero-owner-request");
        fs::write(
            directory.join("StartDungeonRequest.json"),
            serde_json::to_vec(&serde_json::json!({
                "fightGroup": {
                    "equips": [
                        { "heroUid": 0, "equipUid": [100] },
                        { "heroUid": 0, "equipUid": [0] },
                        { "heroUid": 42, "equipUid": [200] }
                    ]
                }
            }))
            .unwrap(),
        )
        .unwrap();

        assert_eq!(
            battle_request_metadata(&directory.join("BeginRoundReply_1.json"))
                .unwrap()
                .selected_equips,
            HashMap::from([(42, 200)])
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn invalid_zero_owner_equipment_stays_fail_loud() {
        for (label, equip_uid) in [
            ("negative", serde_json::json!([-1])),
            ("malformed", serde_json::json!(["invalid"])),
        ] {
            let directory = test_directory(label);
            fs::write(
                directory.join("StartDungeonRequest.json"),
                serde_json::to_vec(&serde_json::json!({
                    "fightGroup": {
                        "equips": [{ "heroUid": 0, "equipUid": equip_uid }]
                    }
                }))
                .unwrap(),
            )
            .unwrap();

            assert!(battle_request_metadata(&directory.join("BeginRoundReply_1.json")).is_err());
            fs::remove_dir_all(directory).unwrap();
        }
    }

    #[test]
    fn duplicate_empty_and_selected_request_rows_fail_loudly() {
        let directory = test_directory("mixed-duplicate-request");
        fs::write(
            directory.join("StartDungeonRequest.json"),
            serde_json::to_vec(&serde_json::json!({
                "fightGroup": {
                    "equips": [
                        { "heroUid": 42, "equipUid": [] },
                        { "heroUid": 42, "equipUid": [100] }
                    ]
                }
            }))
            .unwrap(),
        )
        .unwrap();

        assert!(
            battle_request_metadata(&directory.join("BeginRoundReply_1.json"))
                .unwrap_err()
                .to_string()
                .contains("duplicate equipment selection")
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn duplicate_request_equipment_rows_fail_loudly() {
        crate::init_test_config();
        let uid = 42;
        let directory = test_directory("duplicate-request");
        write_roster(&directory, hero(uid));
        let selection = sonettobuf::FightEquip {
            hero_uid: Some(uid),
            equip_uid: vec![100],
            ..Default::default()
        };
        write_request(&directory, vec![selection.clone(), selection], false);

        assert!(
            preview_attributes(&fight(uid), &directory.join("BeginRoundReply_1.json"))
                .unwrap_err()
                .to_string()
                .contains("duplicate equipment selection")
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn missing_build_metadata_fails_loudly() {
        crate::init_test_config();
        let directory = test_directory("missing-build");
        let error = preview_attributes(&fight(42), &directory.join("BeginRoundReply_1.json"))
            .unwrap_err()
            .to_string();

        assert!(error.contains("missing build metadata"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn missing_talent_does_not_guess_a_default() {
        crate::init_test_config();
        let mut hero = hero(42);
        hero.talent = None;

        assert!(
            preview_build_input(&fight(42).attacker.unwrap().entitys[0], &hero)
                .unwrap_err()
                .to_string()
                .contains("missing talent")
        );
    }

    #[test]
    fn missing_rank_does_not_infer_from_level() {
        crate::init_test_config();
        let mut hero = hero(42);
        hero.rank = None;

        assert!(
            preview_build_input(&fight(42).attacker.unwrap().entitys[0], &hero)
                .unwrap_err()
                .to_string()
                .contains("missing rank")
        );
    }

    #[test]
    fn invalid_positive_trial_id_fails_loudly() {
        crate::init_test_config();
        let directory = test_directory("invalid-trial");
        let mut fight = fight(42);
        fight.attacker.as_mut().unwrap().entitys[0].trial_id = Some(i32::MAX);

        assert!(
            preview_attributes(&fight, &directory.join("BeginRoundReply_1.json"))
                .unwrap_err()
                .to_string()
                .contains("invalid trial")
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn battle_balance_applies_to_the_full_equipment_loadout() {
        crate::init_test_config();
        let uid = 42;
        let directory = test_directory("balanced-loadout");
        let hero = hero(uid);
        write_roster(&directory, hero.clone());
        write_request(&directory, Vec::new(), true);
        let mut fight = fight(uid);
        fight.battle_id = Some(116385108);
        for equip in &mut fight.attacker.as_mut().unwrap().entitys[0].equips {
            equip.equip_lv = Some(50);
        }
        let entity = &fight.attacker.as_ref().unwrap().entitys[0];
        let build = preview_build_input(entity, &hero).unwrap();
        let equips = validated_equipment_loadout(entity, &hero, None).unwrap();
        let balance = request_battle_balance(&fight, true).unwrap().unwrap();
        let expected = balance.stats_for(&build, &equips);
        let unbalanced = Stats::build_for_loadout(&build, &equips);

        assert_eq!(
            preview_attributes(&fight, &directory.join("BeginRoundReply_1.json")).unwrap(),
            (vec![(uid, expected.ex())], vec![(uid, expected.sp())]),
        );
        assert_ne!(expected, unbalanced);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn loadout_diffs_report_fields_that_differ_from_the_rebuilt_entity() {
        crate::init_test_config();
        let uid = 42;
        let directory = test_directory("loadout");
        let hero = hero(uid);
        write_roster(&directory, hero.clone());
        let mut fight = fight(uid);
        let entity = &mut fight.attacker.as_mut().unwrap().entitys[0];
        let built = battle::engine::entity::builder::EntityBuilder::new(
            preview_build_input(entity, &hero).unwrap(),
            0,
            0,
            false,
        )
        .build();
        entity.skill_group1 = built.skill_group1.clone();
        entity.skill_group2 = built.skill_group2.clone();
        entity.ex_skill = built.ex_skill;
        entity.ex_point_type = built.ex_point_type;
        let path = directory.join("StartDungeonReply.json");

        assert!(loadout_diffs(&fight, &path).unwrap().is_empty());

        fight.attacker.as_mut().unwrap().entitys[0].ex_skill = Some(1);
        let diffs = loadout_diffs(&fight, &path).unwrap();
        assert_eq!(diffs.len(), 1);
        assert!(diffs[0].contains("exSkill"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn configured_trial_uses_configured_attributes_without_roster_metadata() {
        crate::init_test_config();
        let uid = 42;
        let trial_id = 116385001;
        let mut fight = fight(uid);
        let entity = &mut fight.attacker.as_mut().unwrap().entitys[0];
        entity.trial_id = Some(trial_id);
        entity.equips.clear();
        entity.equip_uid = None;
        let directory = test_directory("trial");
        let (_, expected) =
            battle::engine::entity::builder::EntityBuilder::trial(trial_id, uid, 0, 0).unwrap();

        assert_eq!(
            preview_attributes(&fight, &directory.join("BeginRoundReply_1.json")).unwrap(),
            (vec![(uid, expected.ex())], vec![(uid, expected.sp())]),
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn sorted_updates_override_roster_build_metadata() {
        crate::init_test_config();
        let uid = 42;
        let directory = test_directory("precedence");
        write_roster(&directory, hero(uid));
        let mut earlier = hero(uid);
        earlier.talent = Some(8);
        let mut later = hero(uid);
        later.talent = Some(12);
        fs::write(
            directory.join("HeroUpdatePush_2.json"),
            serde_json::to_vec(&HeroUpdatePush {
                hero_updates: vec![later.clone()],
            })
            .unwrap(),
        )
        .unwrap();
        fs::write(
            directory.join("HeroUpdatePush_1.json"),
            serde_json::to_vec(&HeroUpdatePush {
                hero_updates: vec![earlier],
            })
            .unwrap(),
        )
        .unwrap();

        let heroes = battle_build_metadata(&directory.join("BeginRoundReply_1.json")).unwrap();

        assert_eq!(heroes[&uid].talent, later.talent);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn capture_timeline_supplies_missing_battle_roster() {
        let directory = test_directory("capture-timeline");
        let common = directory.join("decoded/common");
        let battle = directory.join("decoded/Dungeon/Battle1");
        fs::create_dir_all(&common).unwrap();
        fs::create_dir_all(&battle).unwrap();
        let uid = 42;
        fs::write(
            common.join("capture_000001_HeroInfoListReply.json"),
            serde_json::to_vec(&HeroInfoListReply {
                heros: vec![hero(uid)],
                ..Default::default()
            })
            .unwrap(),
        )
        .unwrap();
        fs::write(
            common.join("capture_000002_StartDungeonReply.json"),
            b"captured battle",
        )
        .unwrap();
        let battle_path = battle.join("StartDungeonReply.json");
        fs::write(&battle_path, b"captured battle").unwrap();
        fs::write(
            battle.join("HeroUpdatePush.json"),
            serde_json::to_vec(&HeroUpdatePush {
                hero_updates: vec![hero(99)],
            })
            .unwrap(),
        )
        .unwrap();

        assert_eq!(
            battle_build_metadata(&battle_path).unwrap()[&uid].hero_id,
            3149
        );
        assert_eq!(build_metadata_source(&battle_path).unwrap(), "timeline");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn unique_later_packet_resolves_repeated_battle_start() {
        for command in ["StartDungeonReply", "StartTowerBattleReply"] {
            let directory = test_directory(command);
            let common = directory.join("decoded/common");
            let battle = directory.join("decoded/Dungeon/Battle2");
            fs::create_dir_all(&common).unwrap();
            fs::create_dir_all(&battle).unwrap();
            let uid = 42;
            let mut updated = hero(uid);
            updated.talent = Some(12);
            fs::write(
                common.join("capture_000001_HeroInfoListReply.json"),
                serde_json::to_vec(&HeroInfoListReply {
                    heros: vec![hero(uid)],
                    ..Default::default()
                })
                .unwrap(),
            )
            .unwrap();
            for sequence in [2, 4] {
                fs::write(
                    common.join(format!("capture_{sequence:06}_{command}.json")),
                    b"repeated battle",
                )
                .unwrap();
            }
            fs::write(
                common.join("capture_000003_HeroUpdatePush.json"),
                serde_json::to_vec(&HeroUpdatePush {
                    hero_updates: vec![updated.clone()],
                })
                .unwrap(),
            )
            .unwrap();
            fs::write(
                common.join("capture_000005_EndFightPush.json"),
                b"unique end",
            )
            .unwrap();
            let battle_path = battle.join(format!("{command}.json"));
            fs::write(&battle_path, b"repeated battle").unwrap();
            fs::write(battle.join("EndFightPush.json"), b"unique end").unwrap();

            assert_eq!(
                battle_build_metadata(&battle_path).unwrap()[&uid].talent,
                updated.talent
            );
            fs::remove_dir_all(directory).unwrap();
        }
    }

    #[test]
    fn repeated_battle_start_without_bounded_anchor_fails_loudly() {
        let directory = test_directory("ambiguous-start");
        let common = directory.join("decoded/common");
        let battle = directory.join("decoded/Dungeon/Battle2");
        fs::create_dir_all(&common).unwrap();
        fs::create_dir_all(&battle).unwrap();
        for sequence in [1, 2] {
            fs::write(
                common.join(format!("capture_{sequence:06}_StartDungeonReply.json")),
                b"repeated battle",
            )
            .unwrap();
        }
        let battle_path = battle.join("StartDungeonReply.json");
        fs::write(&battle_path, b"repeated battle").unwrap();

        assert!(
            battle_build_metadata(&battle_path)
                .unwrap_err()
                .to_string()
                .contains("multiple timeline positions")
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn unmatched_common_timeline_falls_back_to_local_build_metadata() {
        let directory = test_directory("unresolved-common");
        let common = directory.join("decoded/common");
        let battle = directory.join("decoded/Dungeon/Battle1");
        fs::create_dir_all(&common).unwrap();
        fs::create_dir_all(&battle).unwrap();
        fs::write(
            common.join("capture_000001_StartDungeonReply.json"),
            b"another battle",
        )
        .unwrap();
        let battle_path = battle.join("StartDungeonReply.json");
        fs::write(&battle_path, b"captured battle").unwrap();
        fs::write(
            battle.join("HeroUpdatePush.json"),
            serde_json::to_vec(&HeroUpdatePush {
                hero_updates: vec![hero(42)],
            })
            .unwrap(),
        )
        .unwrap();

        assert!(
            battle_build_metadata(&battle_path)
                .unwrap()
                .contains_key(&42)
        );
        assert_eq!(build_metadata_source(&battle_path).unwrap(), "local");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn repeated_session_starts_use_the_rosters_before_their_own_position() {
        let directory = test_directory("repeated-session-start");
        let common = directory.join("decoded/common");
        fs::create_dir_all(&common).unwrap();
        let roster = |name: &str, uid| {
            fs::write(
                common.join(name),
                serde_json::to_vec(&HeroInfoListReply {
                    heros: vec![hero(uid)],
                    ..Default::default()
                })
                .unwrap(),
            )
            .unwrap();
        };
        roster("20260927_080000_000_000001_HeroInfoListReply.json", 42);
        let first = common.join("20260927_080100_000_000002_StartDungeonReply.json");
        fs::write(&first, b"same battle").unwrap();
        roster("20260927_080200_000_000003_HeroInfoListReply.json", 43);
        let second = common.join("20260927_080300_000_000004_StartDungeonReply.json");
        fs::write(&second, b"same battle").unwrap();

        assert!(battle_build_metadata(&first).unwrap().contains_key(&42));
        assert!(battle_build_metadata(&second).unwrap().contains_key(&43));
        assert_eq!(build_metadata_source(&second).unwrap(), "timeline");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn session_roster_backs_battles_missing_from_the_timeline() {
        let directory = test_directory("session-roster");
        let common = directory.join("decoded/common");
        let battle = directory.join("decoded/Dungeon/Battle1");
        fs::create_dir_all(&common).unwrap();
        fs::create_dir_all(&battle).unwrap();
        fs::write(
            common.join("capture_000001_HeroInfoListReply.json"),
            serde_json::to_vec(&HeroInfoListReply {
                heros: vec![hero(42)],
                ..Default::default()
            })
            .unwrap(),
        )
        .unwrap();
        let battle_path = battle.join("StartDungeonReply.json");
        fs::write(&battle_path, b"captured battle").unwrap();

        assert!(
            battle_build_metadata(&battle_path)
                .unwrap()
                .contains_key(&42)
        );
        assert_eq!(build_metadata_source(&battle_path).unwrap(), "session");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn session_battle_uses_only_builds_captured_before_its_start() {
        let directory = test_directory("session-battle-build");
        let common = directory.join("decoded/common");
        fs::create_dir_all(&common).unwrap();
        fs::write(
            common.join("20260927_080000_000_000001_HeroInfoListReply.json"),
            serde_json::to_vec(&HeroInfoListReply {
                heros: vec![hero(42)],
                ..Default::default()
            })
            .unwrap(),
        )
        .unwrap();
        let start = common.join("20260927_080100_000_000002_StartDungeonReply.json");
        fs::write(&start, b"captured battle").unwrap();
        fs::write(
            common.join("20260927_080200_000_000003_HeroUpdatePush.json"),
            serde_json::to_vec(&HeroUpdatePush {
                hero_updates: vec![hero(43)],
            })
            .unwrap(),
        )
        .unwrap();

        let heroes = battle_build_metadata(&start).unwrap();

        assert!(heroes.contains_key(&42));
        assert!(!heroes.contains_key(&43));
        assert_eq!(build_metadata_source(&start).unwrap(), "timeline");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn snake_case_round_name_maps_to_capture_command() {
        assert_eq!(
            capture_command(Path::new("begin_round_2.json")).as_deref(),
            Some("BeginRoundReply")
        );
        assert_eq!(
            capture_command(Path::new("BeginRoundReply_2.json")).as_deref(),
            Some("BeginRoundReply")
        );
        assert_eq!(
            capture_command(Path::new(
                "20260927_080404_757_000410_StartDungeonReply.json"
            ))
            .as_deref(),
            Some("StartDungeonReply")
        );
    }
}
