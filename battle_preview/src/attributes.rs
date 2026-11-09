use std::{collections::HashMap, path::Path};

use battle::tooling::{
    damage_tracing_enabled,
    entity::{EntityBuilder, HeroBuildInput, Stats},
};
use sonettobuf::{Fight, FightEntityInfo, HeroExAttribute, HeroInfo, HeroSpAttribute};

#[cfg(test)]
use sonettobuf::{
    Equip, EquipDeletePush, EquipUpdatePush, GetEquipInfoReply, HeroInfoListReply, HeroUpdatePush,
};
#[cfg(test)]
use std::fs;

mod metadata;
mod request;

#[cfg(test)]
use metadata::{capture_command, is_battle_start_packet};
use metadata::{captured_equipment_metadata, resolve_build_metadata};
use request::{
    battle_request_metadata, configured_trial, folder_request_path, request_battle_balance,
    request_metadata, validated_equipment_loadout,
};

type PreviewAttributes = (Vec<(i64, HeroExAttribute)>, Vec<(i64, HeroSpAttribute)>);

#[derive(Default)]
struct BuildMetadata {
    heroes: HashMap<i64, HeroInfo>,
    talent_styles: HashMap<i64, i32>,
}

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
    let request = request_path
        .map(request_metadata)
        .transpose()?
        .unwrap_or_default();
    let metadata = resolve_build_metadata(battle_path, request.selected_assist)?.0;
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

        let hero = metadata.heroes.get(&uid).ok_or_else(|| {
            anyhow::anyhow!("attribute preview missing build metadata uid={uid} hero={model_id}")
        })?;
        let build = preview_build_input_with_style(
            entity,
            hero,
            metadata.talent_styles.get(&uid).copied(),
        )?;
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
    let request = battle_request_metadata(battle_path)?;
    let metadata = resolve_build_metadata(battle_path, request.selected_assist)?.0;
    let battle_balance = request_battle_balance(fight, request.is_balance)?;
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
            let hero = metadata.heroes.get(&uid).ok_or_else(|| {
                anyhow::anyhow!("loadout preview missing build metadata uid={uid}")
            })?;
            let build = preview_build_input_with_style(
                entity,
                hero,
                metadata.talent_styles.get(&uid).copied(),
            )?;
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

/// Where attacker builds come from: the capture timeline; the roster files saved
/// next to the battle; or, lowest confidence, the whole session's rosters and
/// updates (which may include changes made after the battle).
pub fn build_metadata_source(battle_path: &Path) -> anyhow::Result<&'static str> {
    Ok(resolve_build_metadata(battle_path, None)?.1)
}

#[cfg(test)]
fn battle_build_metadata(path: &Path) -> anyhow::Result<HashMap<i64, HeroInfo>> {
    Ok(resolve_build_metadata(path, None)?.0.heroes)
}

#[cfg(test)]
fn preview_build_input(
    entity: &FightEntityInfo,
    hero: &HeroInfo,
) -> anyhow::Result<HeroBuildInput> {
    preview_build_input_with_style(entity, hero, None)
}

fn preview_build_input_with_style(
    entity: &FightEntityInfo,
    hero: &HeroInfo,
    talent_style: Option<i32>,
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
        talent_style: talent_style
            .or_else(|| template.and_then(|template| template.style))
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
