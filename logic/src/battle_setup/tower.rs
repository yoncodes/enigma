use battle::dungeon::{BuiltFight, FightOptions};
use database::{
    db::game::tower as tower_db,
    models::game::tower::{TowerConstId, TowerType},
};
use serde::{Deserialize, Serialize};
use sonettobuf::{FightGroup, StartDungeonRequest, StartTowerBattleRequest};
use sqlx::SqlitePool;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct BattleContext {
    pub tower_type: i32,
    pub tower_id: i32,
    pub layer_id: i32,
    pub difficulty: i32,
    pub talent_plan_id: i32,
}

pub fn validate_battle_start(
    tables: &config::GameDB,
    request: &StartTowerBattleRequest,
) -> anyhow::Result<(StartDungeonRequest, BattleContext)> {
    let mut dungeon = request
        .start_dungeon_request
        .clone()
        .ok_or_else(|| anyhow::anyhow!("tower battle has no dungeon request"))?;
    let episode_id = dungeon
        .episode_id
        .ok_or_else(|| anyhow::anyhow!("tower battle has no episode"))?;
    let mut context = BattleContext {
        tower_type: request
            .r#type
            .ok_or_else(|| anyhow::anyhow!("tower battle has no tower type"))?,
        tower_id: request.tower_id.unwrap_or_default(),
        layer_id: request.layer_id.unwrap_or_default(),
        difficulty: request.difficulty.unwrap_or_default(),
        talent_plan_id: request.talent_plan_id.unwrap_or_default(),
    };
    // Tutorial encounters use a fixed trial boss and talent plan, even if the
    // player has never unlocked that boss or has another plan selected.
    let teach = (context.tower_type == TowerType::Boss.id() && context.layer_id == 0)
        .then(|| tables.tower_boss_teach.iter().find(|row| {
            row.tower_id == context.tower_id && row.teach_id == context.difficulty
                && row.episode_id == episode_id
        })).flatten();
    if let Some(teach) = teach {
        let boss = tables.tower_boss.iter().find(|row| row.tower_id == context.tower_id)
            .ok_or_else(|| anyhow::anyhow!("tutorial has no configured boss"))?;
        anyhow::ensure!(tables.tower_talent_plan.iter().any(|plan|
            plan.boss_id == boss.boss_id && plan.plan_id == teach.plan_id),
            "tutorial has no configured talent plan");
        if let Some(group) = dungeon.fight_group.as_mut() {
            let selected = group.assist_boss_id.unwrap_or_default();
            anyhow::ensure!(selected == 0 || selected == boss.boss_id,
                "tutorial selected a different assist boss");
            group.assist_boss_id = Some(boss.boss_id);
        }
        context.talent_plan_id = teach.plan_id;
    }
    let boss_id = dungeon
        .fight_group
        .as_ref()
        .and_then(|group| group.assist_boss_id)
        .unwrap_or_default();
    let custom_plan_count =
        tower_const(tables, TowerConstId::CustomTalentPlanCount).unwrap_or_default();
    let valid_boss = boss_id == 0
        || (tables
            .tower_assist_boss
            .iter()
            .any(|row| row.boss_id == boss_id)
            && ((1..=custom_plan_count).contains(&context.talent_plan_id)
                || tables
                    .tower_talent_plan
                    .iter()
                    .any(|row| row.boss_id == boss_id && row.plan_id == context.talent_plan_id)));

    let valid_episode = match context.tower_type {
        value if value == TowerType::Normal.id() => {
            context.tower_id == 0
                && tables.tower_permanent_episode.iter().any(|row| {
                    row.layer_id == context.layer_id
                        && row
                            .episode_ids
                            .split('|')
                            .any(|id| id.parse::<i32>() == Ok(episode_id))
                })
        }
        value if value == TowerType::Boss.id() => {
            tables.tower_boss_episode.iter().any(|row| {
                row.tower_id == context.tower_id
                    && row.layer_id == context.layer_id
                    && row.episode_id == episode_id
            }) || (context.layer_id == 0
                && tables.tower_boss_teach.iter().any(|row| {
                    row.tower_id == context.tower_id
                        && row.teach_id == context.difficulty
                        && row.episode_id == episode_id
                }))
        }
        value if value == TowerType::Limited.id() => {
            tables.tower_limited_episode.iter().any(|row| {
                row.season == context.tower_id
                    && row.layer_id == context.layer_id
                    && row.difficulty == context.difficulty
                    && row.episode_id == episode_id
            })
        }
        _ => false,
    };

    anyhow::ensure!(
        valid_episode && valid_boss,
        "invalid tower battle selection"
    );
    Ok((dungeon, context))
}

pub async fn build_fight(
    db: &SqlitePool,
    player_id: i64,
    episode_id: i32,
    battle_id: i32,
    fight_group: &FightGroup,
    options: FightOptions,
    context: BattleContext,
) -> anyhow::Result<BuiltFight> {
    let mut built = super::dungeon::build_fight(
        db,
        player_id,
        episode_id,
        battle_id,
        fight_group,
        options,
        None,
    )
    .await?;
    let boss_id = fight_group.assist_boss_id.unwrap_or_default();
    if boss_id == 0 {
        return Ok(built);
    }

    let tables = config::configs::get();
    let teaching = context.tower_type == TowerType::Boss.id() && context.layer_id == 0;
    let boss_level = if teaching {
        let teach = tables.tower_boss_teach.iter().find(|row| row.tower_id == context.tower_id
            && row.teach_id == context.difficulty && row.episode_id == episode_id)
            .ok_or_else(|| anyhow::anyhow!("unknown boss tutorial"))?;
        let boss = tables.tower_boss.iter().find(|row| row.tower_id == context.tower_id)
            .ok_or_else(|| anyhow::anyhow!("tutorial has no configured boss"))?;
        anyhow::ensure!(boss.boss_id == boss_id && teach.plan_id == context.talent_plan_id,
            "invalid boss tutorial setup");
        tower_const(tables, TowerConstId::TeachBossLevel)
            .filter(|level| *level > 0).ok_or_else(|| anyhow::anyhow!("missing tutorial boss level"))?
    } else {
        let owned_level = tower_db::assist_boss_level(db, player_id, boss_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("assist boss {boss_id} is not owned"))?;
        effective_level(tables, context, owned_level)
    };
    let talent_ids = if let Some(plan) = tables
        .tower_talent_plan
        .iter()
        .find(|plan| plan.boss_id == boss_id && plan.plan_id == context.talent_plan_id)
    {
        battle::tower::system_plan_talents(tables, boss_id, boss_level, &plan.talent_ids)
    } else {
        tower_db::talent_plan_ids(db, player_id, boss_id, context.talent_plan_id).await?
    };

    battle::tower::apply_assist_boss(
        tables,
        player_id,
        boss_id,
        boss_level,
        &talent_ids,
        &mut built,
    )?;
    Ok(built)
}

fn effective_level(tables: &config::GameDB, context: BattleContext, owned_level: i32) -> i32 {
    match context.tower_type {
        value if value == TowerType::Boss.id() && context.layer_id == 0 => {
            tower_const(tables, TowerConstId::TeachBossLevel).unwrap_or(owned_level)
        }
        value if value == TowerType::Limited.id() => owned_level
            .max(tower_const(tables, TowerConstId::BalanceBossLevel).unwrap_or(owned_level)),
        _ => owned_level,
    }
}

fn tower_const(tables: &config::GameDB, id: TowerConstId) -> Option<i32> {
    tables.tower_const.get(id.id())?.value.parse().ok()
}

#[cfg(test)]
mod tests;
