use super::ActivityManager;
use crate::{
    error::AppError,
    types::{
        dice_hero_get_reward_type::DiceHeroGetRewardType, dice_hero_level_type::DiceHeroLevelType,
        dice_hero_reward_type::DiceHeroRewardType,
    },
};
use database::db::game::dice_hero;
use sonettobuf::{
    DiceGameInfo, DiceHeroBaseInfo, DiceHeroEnterStoryReply, DiceHeroGameInfo,
    DiceHeroGetInfoReply, DiceHeroGetRewardReply, DiceHeroRewardItem, DiceHeroRewardPanel,
};
use sqlx::SqlitePool;

impl ActivityManager {
    pub async fn dice_hero_info(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
    ) -> Result<DiceHeroGetInfoReply, AppError> {
        dice_hero_info(db, self.player_id, tables).await
    }

    pub async fn dice_hero_enter_story(
        &self,
        db: &SqlitePool,
        chapter: i32,
        level_id: i32,
        tables: &config::GameDB,
    ) -> Result<DiceHeroEnterStoryReply, AppError> {
        dice_hero_enter_story(db, self.player_id, chapter, level_id, tables).await
    }

    pub async fn dice_hero_get_reward(
        &self,
        db: &SqlitePool,
        chapter: i32,
        indexes: Vec<i32>,
        tables: &config::GameDB,
    ) -> Result<DiceHeroGetRewardReply, AppError> {
        dice_hero_get_reward(db, self.player_id, chapter, indexes, tables).await
    }
}

async fn dice_hero_info(
    db: &SqlitePool,
    player_id: i64,
    tables: &config::GameDB,
) -> Result<DiceHeroGetInfoReply, AppError> {
    sync_state(db, player_id, tables).await?;
    let chapters = dice_hero::get_chapters(db, player_id, chapter_ids(tables)).await?;

    Ok(DiceHeroGetInfoReply {
        info: Some(DiceGameInfo {
            game_info: chapters
                .into_iter()
                .map(|chapter| {
                    let pass_level_ids = serde_json::from_str(&chapter.pass_level_ids)?;
                    let relic_ids = serde_json::from_str(&chapter.relic_ids)?;
                    let reward_items = serde_json::from_str(&chapter.reward_items_json)?;

                    Ok(dice_hero_game_info(
                        chapter.chapter,
                        pass_level_ids,
                        chapter.current_hero_id,
                        relic_ids,
                        reward_items,
                        tables,
                    ))
                })
                .collect::<Result<Vec<_>, AppError>>()?,
        }),
    })
}

async fn sync_state(
    db: &SqlitePool,
    player_id: i64,
    tables: &config::GameDB,
) -> Result<(), AppError> {
    let ids = chapter_ids(tables);
    dice_hero::sync_chapters(db, player_id, ids.clone()).await?;
    let mut chapters = dice_hero::get_chapters(db, player_id, ids).await?;
    for chapter in &mut chapters {
        ensure_dice_hero_reward_panel(db, player_id, chapter, tables).await?;
    }
    Ok(())
}

fn chapter_ids(tables: &config::GameDB) -> Vec<i32> {
    if !tables.activity205_enter.is_empty() {
        tables.activity205_enter.iter().map(|row| row.id).collect()
    } else {
        tables.activity193_enter.iter().map(|row| row.id).collect()
    }
}

async fn dice_hero_enter_story(
    db: &SqlitePool,
    player_id: i64,
    chapter: i32,
    level_id: i32,
    tables: &config::GameDB,
) -> Result<DiceHeroEnterStoryReply, AppError> {
    if let Some(level) = tables.dice_level.get(level_id).filter(|row| {
        row.chapter == chapter
            && DiceHeroLevelType::from_id(row.r#type) == Some(DiceHeroLevelType::Story)
    }) {
        if DiceHeroGetRewardType::from_id(level.reward_select_type)
            == Some(DiceHeroGetRewardType::None)
        {
            dice_hero::complete_level(db, player_id, chapter, level_id).await?;
            dice_hero::save_reward_items(db, player_id, chapter, "[]".to_string()).await?;
        } else {
            let items = dice_hero_reward_items(level, tables);
            if !items.is_empty() {
                let json = serde_json::to_string(&items)?;
                dice_hero::save_reward_items(db, player_id, chapter, json).await?;
            }
        }
    }

    sync_state(db, player_id, tables).await?;
    Ok(DiceHeroEnterStoryReply {
        info: dice_hero_info(db, player_id, tables).await?.info,
    })
}

async fn dice_hero_get_reward(
    db: &SqlitePool,
    player_id: i64,
    chapter: i32,
    indexes: Vec<i32>,
    tables: &config::GameDB,
) -> Result<DiceHeroGetRewardReply, AppError> {
    let chapter_state = dice_hero::get_chapter(db, player_id, chapter).await?;
    let pass_level_ids = serde_json::from_str::<Vec<i32>>(&chapter_state.pass_level_ids)?;
    let reward_items =
        serde_json::from_str::<Vec<DiceHeroRewardItem>>(&chapter_state.reward_items_json)?;
    let mut relic_ids = serde_json::from_str::<Vec<i32>>(&chapter_state.relic_ids)?;
    let mut skill_card_ids = serde_json::from_str::<Vec<i32>>(&chapter_state.skill_card_ids)?;
    let mut current_hero_id = chapter_state.current_hero_id;

    if reward_items.is_empty() {
        return Ok(DiceHeroGetRewardReply {
            info: dice_hero_info(db, player_id, tables).await?.info,
            chapter: Some(chapter),
        });
    }

    for index in indexes {
        let Some(item) = reward_items.get(index as usize) else {
            continue;
        };
        apply_dice_hero_reward_item(
            item,
            &mut current_hero_id,
            &mut relic_ids,
            &mut skill_card_ids,
            tables,
        );
    }

    if let Some(level) = next_dice_hero_level(chapter, &pass_level_ids, tables) {
        dice_hero::complete_level(db, player_id, chapter, level.id).await?;
    }
    dice_hero::save_reward_state(
        db,
        player_id,
        chapter,
        current_hero_id,
        &relic_ids,
        &skill_card_ids,
    )
    .await?;

    sync_state(db, player_id, tables).await?;
    Ok(DiceHeroGetRewardReply {
        info: dice_hero_info(db, player_id, tables).await?.info,
        chapter: Some(chapter),
    })
}

fn dice_hero_game_info(
    chapter: i32,
    pass_level_ids: Vec<i32>,
    current_hero_id: i32,
    relic_ids: Vec<i32>,
    reward_items: Vec<DiceHeroRewardItem>,
    tables: &config::GameDB,
) -> DiceHeroGameInfo {
    DiceHeroGameInfo {
        chapter: Some(chapter),
        hero_base_info: Some(dice_hero_base_info(current_hero_id, relic_ids, tables)),
        panel: Some(DiceHeroRewardPanel { reward_items }),
        pass_level_ids,
    }
}

fn dice_hero_base_info(
    current_hero_id: i32,
    relic_ids: Vec<i32>,
    tables: &config::GameDB,
) -> DiceHeroBaseInfo {
    let hero = tables
        .dice_character
        .get(current_hero_id)
        .or_else(|| tables.dice_character.iter().next());
    let Some(hero) = hero else {
        return DiceHeroBaseInfo::default();
    };

    DiceHeroBaseInfo {
        id: Some(hero.id),
        hp: Some(hero.hp as i64),
        shield: Some(0),
        power: Some(hero.power as i64),
        max_hp: Some(hero.hp as i64),
        max_shield: Some(0),
        max_power: Some(hero.power as i64),
        relic_ids: if relic_ids.is_empty() {
            split_i32_list(&hero.relic_ids)
        } else {
            relic_ids
        },
    }
}

async fn ensure_dice_hero_reward_panel(
    db: &SqlitePool,
    player_id: i64,
    chapter: &mut database::models::game::dice_hero::DiceHeroChapter,
    tables: &config::GameDB,
) -> Result<(), AppError> {
    if chapter.reward_items_json != "[]" {
        return Ok(());
    }

    let pass_level_ids = serde_json::from_str::<Vec<i32>>(&chapter.pass_level_ids)?;
    let Some(level) = next_dice_hero_level(chapter.chapter, &pass_level_ids, tables) else {
        return Ok(());
    };
    if DiceHeroLevelType::from_id(level.r#type) != Some(DiceHeroLevelType::Story) {
        return Ok(());
    }
    let Some(reward_type) = DiceHeroGetRewardType::from_id(level.reward_select_type) else {
        return Ok(());
    };
    if reward_type == DiceHeroGetRewardType::None {
        return Ok(());
    }

    let items = dice_hero_reward_items(level, tables);
    if items.is_empty() {
        return Ok(());
    }

    chapter.reward_items_json = serde_json::to_string(&items)?;
    dice_hero::save_reward_items(
        db,
        player_id,
        chapter.chapter,
        chapter.reward_items_json.clone(),
    )
    .await?;

    Ok(())
}

fn dice_hero_reward_items(
    level: &config::dice_level::DiceLevel,
    tables: &config::GameDB,
) -> Vec<DiceHeroRewardItem> {
    let Some(reward_type) = DiceHeroGetRewardType::from_id(level.reward_select_type) else {
        return Vec::new();
    };
    if reward_type == DiceHeroGetRewardType::None {
        return Vec::new();
    }

    const INFINITE_MODE: i32 = 2;

    if level.mode == INFINITE_MODE {
        return tables
            .dice_character
            .iter()
            .map(|hero| DiceHeroRewardItem {
                r#type: Some(DiceHeroRewardType::Hero.id()),
                id: Some(hero.id),
            })
            .collect();
    }

    if DiceHeroLevelType::from_id(level.r#type) == Some(DiceHeroLevelType::Story) {
        let hero_id = level.chapter.clamp(1, tables.dice_character.len() as i32);
        return vec![DiceHeroRewardItem {
            r#type: Some(DiceHeroRewardType::Hero.id()),
            id: Some(hero_id),
        }];
    }

    let relic = nth_reward_row(level.id, tables.dice_relic.len()).and_then(|idx| {
        tables
            .dice_relic
            .iter()
            .nth(idx)
            .map(|row| DiceHeroRewardItem {
                r#type: Some(DiceHeroRewardType::Relic.id()),
                id: Some(row.id),
            })
    });
    let card = nth_reward_row(level.id, tables.dice_card.len()).and_then(|idx| {
        tables
            .dice_card
            .iter()
            .nth(idx)
            .map(|row| DiceHeroRewardItem {
                r#type: Some(DiceHeroRewardType::SkillCard.id()),
                id: Some(row.id),
            })
    });

    relic.into_iter().chain(card).collect()
}

fn nth_reward_row(seed: i32, len: usize) -> Option<usize> {
    (len != 0).then_some(seed as usize % len)
}

fn apply_dice_hero_reward_item(
    item: &DiceHeroRewardItem,
    current_hero_id: &mut i32,
    relic_ids: &mut Vec<i32>,
    skill_card_ids: &mut Vec<i32>,
    tables: &config::GameDB,
) {
    let Some(id) = item.id else {
        return;
    };

    match item.r#type {
        Some(value) if value == DiceHeroRewardType::Hero.id() => {
            *current_hero_id = id;
            if let Some(hero) = tables.dice_character.get(id) {
                add_unique(relic_ids, split_i32_list(&hero.relic_ids));
                add_unique(skill_card_ids, split_i32_list(&hero.skilllist));
            }
        }
        Some(value) if value == DiceHeroRewardType::Relic.id() => add_unique(relic_ids, [id]),
        Some(value) if value == DiceHeroRewardType::SkillCard.id() => {
            add_unique(skill_card_ids, [id])
        }
        _ => {}
    }
}

fn add_unique(ids: &mut Vec<i32>, new_ids: impl IntoIterator<Item = i32>) {
    for id in new_ids {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
}

fn next_dice_hero_level<'a>(
    chapter: i32,
    pass_level_ids: &[i32],
    tables: &'a config::GameDB,
) -> Option<&'a config::dice_level::DiceLevel> {
    let mut levels = tables
        .dice_level
        .iter()
        .filter(|row| row.chapter == chapter)
        .collect::<Vec<_>>();
    levels.sort_by_key(|row| (row.room, row.id));
    levels
        .into_iter()
        .find(|row| !pass_level_ids.contains(&row.id))
}

fn split_i32_list(value: &str) -> Vec<i32> {
    value
        .split('#')
        .filter_map(|part| part.parse::<i32>().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn malformed_chapter_state_is_not_replaced() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let tables = config::configs::get();
        let chapter = chapter_ids(tables)[0];
        let db = SqlitePool::connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (43, 'dice-hero-corrupt', 0, 0)",
        )
        .execute(&db)
        .await
        .unwrap();
        dice_hero::sync_chapters(&db, 43, vec![chapter])
            .await
            .unwrap();

        sqlx::query(
            "UPDATE user_dice_hero_chapters SET pass_level_ids = '{'
             WHERE user_id = 43 AND chapter = ?",
        )
        .bind(chapter)
        .execute(&db)
        .await
        .unwrap();
        assert!(
            dice_hero::complete_level(&db, 43, chapter, 1)
                .await
                .is_err()
        );

        for field in [
            "pass_level_ids",
            "reward_items_json",
            "relic_ids",
            "skill_card_ids",
        ] {
            sqlx::query(
                "UPDATE user_dice_hero_chapters
                 SET pass_level_ids = '[]', reward_items_json = '[]',
                     relic_ids = '[]', skill_card_ids = '[]'
                 WHERE user_id = 43 AND chapter = ?",
            )
            .bind(chapter)
            .execute(&db)
            .await
            .unwrap();
            sqlx::query(&format!(
                "UPDATE user_dice_hero_chapters SET {field} = '{{'
                 WHERE user_id = 43 AND chapter = ?"
            ))
            .bind(chapter)
            .execute(&db)
            .await
            .unwrap();

            assert!(matches!(
                dice_hero_get_reward(&db, 43, chapter, Vec::new(), tables).await,
                Err(AppError::Serde(_))
            ));
            let stored: String = sqlx::query_scalar(&format!(
                "SELECT {field} FROM user_dice_hero_chapters
                 WHERE user_id = 43 AND chapter = ?"
            ))
            .bind(chapter)
            .fetch_one(&db)
            .await
            .unwrap();
            assert_eq!(stored, "{", "{field}");
        }
    }
}
