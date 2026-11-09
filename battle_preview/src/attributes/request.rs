use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use battle::tooling::entity::{BattleBalance, EntityBuilder, EquipmentBuildInput, Stats};
use sonettobuf::{Equip, Fight, FightEntityInfo, HeroInfo};

use super::metadata::capture_command;

#[derive(Debug, Default)]
pub(super) struct BattleRequestMetadata {
    pub(super) is_balance: bool,
    pub(super) selected_equips: HashMap<i64, i64>,
    pub(super) selected_assist: Option<SelectedAssist>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SelectedAssist {
    pub(super) hero_uid: i64,
    pub(super) user_id: i64,
}

pub(super) fn folder_request_path(battle_path: &Path) -> Option<PathBuf> {
    let parent = battle_path.parent()?;
    ["StartDungeonRequest.json", "StartAct229BattleRequest.json"]
        .map(|name| parent.join(name))
        .into_iter()
        .find(|path| path.exists())
}

pub(super) fn battle_request_metadata(battle_path: &Path) -> anyhow::Result<BattleRequestMetadata> {
    folder_request_path(battle_path)
        .map(|path| request_metadata(&path))
        .transpose()
        .map(Option::unwrap_or_default)
}

pub(super) fn request_metadata(request_path: &Path) -> anyhow::Result<BattleRequestMetadata> {
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
    let fight_group = request
        .get("fightGroup")
        .or_else(|| request.get("fight_group"))
        .map(|group| {
            group
                .as_object()
                .ok_or_else(|| anyhow::anyhow!("invalid fight group"))
        })
        .transpose()?;
    let selections = match fight_group {
        Some(group) => match group.get("equips") {
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
    let assist_hero_uid = fight_group
        .and_then(|group| {
            group
                .get("assistHeroUid")
                .or_else(|| group.get("assist_hero_uid"))
        })
        .filter(|value| !value.is_null())
        .map(json_i64)
        .transpose()?
        .unwrap_or_default();
    let assist_user_id = fight_group
        .and_then(|group| {
            group
                .get("assistUserId")
                .or_else(|| group.get("assist_user_id"))
        })
        .filter(|value| !value.is_null())
        .map(json_i64)
        .transpose()?
        .unwrap_or_default();
    let selected_assist = match (assist_hero_uid, assist_user_id) {
        (0, 0) => None,
        (hero_uid, user_id) if hero_uid > 0 && user_id > 0 => {
            Some(SelectedAssist { hero_uid, user_id })
        }
        _ => anyhow::bail!(
            "invalid assist selection hero_uid={assist_hero_uid} user_id={assist_user_id}"
        ),
    };
    Ok(BattleRequestMetadata {
        is_balance,
        selected_equips,
        selected_assist,
    })
}

fn json_i64(value: &serde_json::Value) -> anyhow::Result<i64> {
    value
        .as_i64()
        .or_else(|| value.as_str().and_then(|raw| raw.parse().ok()))
        .ok_or_else(|| anyhow::anyhow!("invalid integer value"))
}

pub(super) fn request_battle_balance(
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

pub(super) fn configured_trial(
    entity: &FightEntityInfo,
) -> anyhow::Result<Option<(FightEntityInfo, Stats)>> {
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

pub(super) fn validated_equipment_loadout(
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
