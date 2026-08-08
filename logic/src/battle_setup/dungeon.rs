use anyhow::{Result, ensure};
use battle::{
    dungeon::{BattleFighter, BattleRoster, BattleRosterPlan, BuiltFight, FightOptions},
    engine::entity::input::{EquipmentBuildInput, HeroBuildInput},
};
use database::models::game::{
    equipment::{Equipment, EquipmentModel, UserEquipmentModel},
    heros::{HeroData, HeroModel, UserHeroModel, get_hero_by_uid},
};
use sonettobuf::FightGroup;
use sqlx::SqlitePool;

pub async fn build_fight(
    db: &SqlitePool,
    player_id: i64,
    episode_id: i32,
    battle_id: i32,
    fight_group: &FightGroup,
    options: FightOptions,
    params: Option<&str>,
) -> Result<BuiltFight> {
    let plan = battle::dungeon::plan_roster(
        episode_id,
        battle_id,
        options.is_balance,
        fight_group,
        params,
    )?;
    let roster = load_roster(db, player_id, &plan, fight_group).await?;
    battle::dungeon::build_fight(&roster, episode_id, battle_id, fight_group, options, params)
}

async fn load_roster(
    db: &SqlitePool,
    player_id: i64,
    plan: &BattleRosterPlan,
    fight_group: &FightGroup,
) -> Result<BattleRoster> {
    let heroes = UserHeroModel::new(player_id, db.clone());
    let mut fighters = Vec::new();
    for uid in plan.hero_uids().iter().copied() {
        let hero = heroes.get_uid(uid).await?;
        let equips = load_equips(db, &hero, fight_group).await?;
        fighters.push(BattleFighter {
            hero: hero_build_input(&hero),
            equips: equips.iter().map(equipment_build_input).collect(),
        });
    }

    Ok(BattleRoster {
        user_id: player_id,
        fighters,
        compose_support: load_compose_support(db, &heroes, plan).await?,
    })
}

async fn load_equips(
    db: &SqlitePool,
    hero: &HeroData,
    fight_group: &FightGroup,
) -> Result<Vec<Equipment>> {
    let requested = fight_group
        .equips
        .iter()
        .find(|equip| equip.hero_uid == Some(hero.record.uid))
        .map(|equip| equip.equip_uid.as_slice())
        .unwrap_or_default();
    let mut requested = requested.iter().copied().filter(|uid| *uid != 0);
    let selected_uid = requested.next().unwrap_or(hero.record.default_equip_uid);
    ensure!(
        requested.all(|uid| uid == selected_uid),
        "multiple primary psychubes selected for hero {}",
        hero.record.uid
    );

    let model = UserEquipmentModel::new(hero.record.user_id, db.clone());
    let mut equips = Vec::new();
    if selected_uid != 0 {
        equips.push(model.get_equip(selected_uid).await?);
    }
    let companions = equips
        .iter()
        .filter_map(|equip| {
            config::configs::get().linked_psychube_id(hero.record.hero_id, equip.equip_id)
        })
        .collect::<Vec<_>>();
    if !companions.is_empty() {
        let owned = EquipmentModel::get_all(&model).await?;
        for equip_id in companions {
            if !equips.iter().any(|equip| equip.equip_id == equip_id)
                && let Some(equip) = owned.iter().find(|equip| equip.equip_id == equip_id)
            {
                equips.push(equip.clone());
            }
        }
    }
    Ok(equips)
}

async fn load_compose_support(
    db: &SqlitePool,
    heroes: &UserHeroModel,
    plan: &BattleRosterPlan,
) -> Result<Option<HeroBuildInput>> {
    let Some(selection) = plan.compose_support() else {
        return Ok(None);
    };
    let hero = if let Some(uid) = selection.hero_uid {
        get_hero_by_uid(db, uid).await?
    } else {
        heroes.get_hero(selection.hero_id).await?
    };
    Ok(Some(hero_build_input(&hero)))
}

fn hero_build_input(hero: &HeroData) -> HeroBuildInput {
    let record = &hero.record;
    let template = hero
        .talent_templates
        .iter()
        .find(|(template, _)| template.template_id == record.use_talent_template_id)
        .or_else(|| hero.talent_templates.first());
    let cubes = template
        .filter(|(_, cubes)| !cubes.is_empty())
        .map(|(_, cubes)| cubes.as_slice())
        .unwrap_or(&hero.talent_cubes);
    HeroBuildInput {
        uid: record.uid,
        user_id: record.user_id,
        hero_id: record.hero_id,
        skin: record.skin,
        level: record.level,
        rank: record.rank,
        ex_skill_level: record.ex_skill_level,
        talent: record.talent,
        talent_style: template
            .map(|(template, _)| template.style)
            .unwrap_or_default(),
        talent_placements: cubes.iter().map(|cube| cube.cube_id).collect(),
        destiny_rank: record.destiny_rank,
        destiny_stone: record.destiny_stone,
    }
}

fn equipment_build_input(equip: &Equipment) -> EquipmentBuildInput {
    EquipmentBuildInput {
        uid: equip.uid,
        equip_id: equip.equip_id,
        level: equip.level,
        break_level: equip.break_lv,
        refine_level: equip.refine_lv,
    }
}

#[cfg(test)]
mod test;
