use std::collections::HashSet;

use anyhow::Result;
use database::models::game::heros::UserHeroModel;
use muipserver::{GmResponse, MaterialQuery};
use serde::Serialize;

use crate::net::app::AppState;

pub(super) fn dungeon_catalog() -> GmResponse {
    let tables = config::configs::get();
    let active_episodes = tables
        .story_episodes()
        .chain(tables.resource_episodes())
        .collect::<Vec<_>>();
    let active_chapter_ids = active_episodes
        .iter()
        .map(|episode| episode.chapter_id)
        .collect::<HashSet<_>>();
    let mut chapters = tables
        .chapter
        .iter()
        .filter(|chapter| {
            active_chapter_ids.contains(&chapter.id)
                && crate::logic::dungeon::chapter_missing_reward_heroes(chapter.id).is_empty()
        })
        .map(|chapter| DungeonCatalogChapter {
            id: chapter.id,
            name: resolve_name(
                tables,
                if chapter.name_en.is_empty() {
                    &chapter.name
                } else {
                    &chapter.name_en
                },
            ),
        })
        .collect::<Vec<_>>();
    let visible_chapter_ids = chapters
        .iter()
        .map(|chapter| chapter.id)
        .collect::<HashSet<_>>();
    let mut episodes = active_episodes
        .into_iter()
        .filter(|episode| visible_chapter_ids.contains(&episode.chapter_id))
        .map(|episode| DungeonCatalogEpisode {
            id: episode.id,
            chapter_id: episode.chapter_id,
            name: resolve_name(
                tables,
                if episode.name_en.is_empty() {
                    &episode.name
                } else {
                    &episode.name_en
                },
            ),
        })
        .collect::<Vec<_>>();
    chapters.sort_unstable_by_key(|chapter| chapter.id);
    episodes.sort_unstable_by_key(|episode| (episode.chapter_id, episode.id));

    GmResponse::ok_data("dungeons", DungeonCatalog { chapters, episodes })
}

pub(super) async fn materials(state: &AppState, query: MaterialQuery) -> GmResponse {
    match material_catalog(state, query).await {
        Ok(catalog) => GmResponse::ok_data("materials", catalog),
        Err(err) => GmResponse::err(400, err.to_string()),
    }
}

pub(super) async fn hero_upgrade_catalog(state: &AppState, player_id: i64) -> GmResponse {
    match build_hero_upgrade_catalog(state, player_id).await {
        Ok(catalog) => GmResponse::ok_data("heroes", catalog),
        Err(err) => GmResponse::err(400, err.to_string()),
    }
}

#[derive(Debug, Serialize)]
pub struct CatalogType {
    r#type: i32,
    name: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    r#type: i32,
    pub(super) id: i32,
    name: String,
    raw_name: String,
    rare: i32,
}

#[derive(Debug, Serialize)]
struct CatalogResponse {
    types: Vec<CatalogType>,
    items: Vec<CatalogEntry>,
}

#[derive(Debug, Serialize)]
struct DungeonCatalog {
    chapters: Vec<DungeonCatalogChapter>,
    episodes: Vec<DungeonCatalogEpisode>,
}

#[derive(Debug, Serialize)]
struct DungeonCatalogChapter {
    id: i32,
    name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DungeonCatalogEpisode {
    id: i32,
    chapter_id: i32,
    name: String,
}

#[derive(Debug, Serialize)]
pub(super) struct HeroUpgradeCatalog {
    pub(super) heroes: Vec<HeroUpgradeEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HeroUpgradeEntry {
    pub(super) id: i32,
    name: String,
    pub(super) rank: i32,
    pub(super) level: i32,
    pub(super) resonance: i32,
    portrait_level: i32,
    pub(super) max_resonance: i32,
    pub(super) max_editable_resonance: i32,
    pub(super) ranks: Vec<HeroUpgradeRank>,
    pub(super) destiny_rank: i32,
    pub(super) destiny_level: i32,
    pub(super) max_destiny_rank: i32,
    pub(super) destiny_stones: Vec<HeroUpgradeDestinyStone>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HeroUpgradeRank {
    pub(super) rank: i32,
    pub(super) level_offset: i32,
    pub(super) max_level: i32,
}

#[derive(Debug, Serialize)]
pub(super) struct HeroUpgradeDestinyStone {
    pub(super) id: i32,
    name: String,
    pub(super) unlocked: bool,
}

pub(super) async fn build_hero_upgrade_catalog(
    state: &AppState,
    player_id: i64,
) -> Result<HeroUpgradeCatalog> {
    let exists = sqlx::query_scalar::<_, i64>("SELECT 1 FROM users WHERE id = ?")
        .bind(player_id)
        .fetch_optional(state.db)
        .await?;
    if exists.is_none() {
        anyhow::bail!("player `{player_id}` was not found");
    }

    let tables = state.tables;
    let mut heroes = Vec::new();
    for hero in UserHeroModel::new(player_id, (*state.db).clone())
        .get_all_heroes()
        .await?
    {
        let hero_id = hero.record.hero_id;
        let character = tables
            .character
            .get(hero_id)
            .ok_or_else(|| anyhow::anyhow!("hero {hero_id} has no character config"))?;
        let mut rank_rows = tables
            .character_rank
            .iter()
            .filter(|row| row.hero_id == hero_id)
            .collect::<Vec<_>>();
        rank_rows.sort_unstable_by_key(|row| row.rank);
        let mut level_offset = 0;
        let mut ranks = Vec::with_capacity(rank_rows.len());
        for row in rank_rows {
            let level_limit = tables
                .character_rank_level_limit(hero_id, row.rank)
                .ok_or_else(|| {
                    anyhow::anyhow!("hero {hero_id} rank {} has no level limit", row.rank)
                })?;
            ranks.push(HeroUpgradeRank {
                rank: row.rank - 1,
                level_offset,
                max_level: level_limit - level_offset,
            });
            level_offset = level_limit;
        }
        let rank = hero.record.rank - 1;
        let current_offset = ranks
            .iter()
            .find(|entry| entry.rank == rank)
            .map(|entry| entry.level_offset)
            .ok_or_else(|| {
                anyhow::anyhow!("hero {hero_id} has no rank {} config", hero.record.rank)
            })?;
        let max_resonance = tables
            .character_talent
            .iter()
            .filter(|row| row.hero_id == hero_id)
            .map(|row| row.talent_id)
            .max()
            .ok_or_else(|| anyhow::anyhow!("hero {hero_id} has no character_talent config"))?;
        let max_editable_resonance = tables
            .character_talent
            .iter()
            .filter(|row| row.hero_id == hero_id && row.requirement <= hero.record.rank)
            .map(|row| row.talent_id)
            .max()
            .unwrap_or(hero.record.talent)
            .max(hero.record.talent);
        let raw_name = if character.name_eng.is_empty() {
            &character.name
        } else {
            &character.name_eng
        };
        let destiny_stones = if crate::logic::hero::destiny_available(
            hero_id,
            hero.record.rank,
            hero.record.level,
        ) {
            crate::logic::hero::destiny_stones(hero_id)
                .into_iter()
                .filter_map(|stone_id| {
                    let stone = tables.character_destiny_stone_cost(stone_id)?;
                    Some(HeroUpgradeDestinyStone {
                        id: stone_id,
                        name: resolve_name(tables, &stone.name),
                        unlocked: hero.destiny_stone_unlocks.contains(&stone_id),
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        let max_destiny_rank = tables
            .character_destiny(hero_id)
            .and_then(|destiny| {
                tables
                    .character_destiny_slots
                    .iter()
                    .filter(|slot| slot.slots_id == destiny.slots_id)
                    .map(|slot| slot.stage)
                    .max()
            })
            .unwrap_or_default();
        heroes.push(HeroUpgradeEntry {
            id: hero_id,
            name: resolve_name(tables, raw_name),
            rank,
            level: (hero.record.level - current_offset).max(1),
            resonance: hero.record.talent,
            portrait_level: hero.record.ex_skill_level,
            max_resonance,
            max_editable_resonance,
            ranks,
            destiny_rank: hero.record.destiny_rank,
            destiny_level: hero.record.destiny_level,
            max_destiny_rank,
            destiny_stones,
        });
    }
    heroes.sort_unstable_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));

    Ok(HeroUpgradeCatalog { heroes })
}

async fn material_catalog(state: &AppState, query: MaterialQuery) -> Result<CatalogResponse> {
    let Some(kind) = query.r#type.and_then(MaterialKind::from_raw) else {
        return Ok(CatalogResponse {
            types: catalog_types(),
            items: Vec::new(),
        });
    };

    let q = query.q.unwrap_or_default().to_ascii_lowercase();
    let limit = query.limit.unwrap_or(200).min(1000);
    let owned_skins = if kind == MaterialKind::Skin && query.unowned_only.unwrap_or(false) {
        match query.player_uid {
            Some(player_id) => Some(
                UserHeroModel::new(player_id, (*state.db).clone())
                    .get_skins()
                    .await?
                    .into_iter()
                    .collect::<HashSet<_>>(),
            ),
            None => None,
        }
    } else {
        None
    };
    let mut materials = entries_for_kind(state.tables, kind, owned_skins.as_ref());

    if !q.is_empty() {
        materials.retain(|entry| {
            entry.id.to_string().contains(&q)
                || entry.name.to_ascii_lowercase().contains(&q)
                || entry.raw_name.to_ascii_lowercase().contains(&q)
        });
    }

    materials.truncate(limit);

    Ok(CatalogResponse {
        types: catalog_types(),
        items: materials,
    })
}

fn catalog_types() -> Vec<CatalogType> {
    [
        MaterialKind::Item,
        MaterialKind::Currency,
        MaterialKind::PlayerExp,
        MaterialKind::Hero,
        MaterialKind::Skin,
        MaterialKind::Equipment,
        MaterialKind::PowerItem,
        MaterialKind::BlockPackage,
        MaterialKind::InsightItem,
    ]
    .into_iter()
    .map(|kind| CatalogType {
        r#type: kind.id(),
        name: kind.label(),
    })
    .collect()
}

pub(super) fn entries_for_kind(
    tables: &config::GameDB,
    kind: MaterialKind,
    owned_skins: Option<&HashSet<i32>>,
) -> Vec<CatalogEntry> {
    match kind {
        MaterialKind::Item => tables
            .item
            .iter()
            .map(|row| catalog_entry(tables, kind, row.id, &row.name, row.rare))
            .collect(),
        MaterialKind::Currency => tables
            .currency
            .iter()
            .map(|row| catalog_entry(tables, kind, row.id, &row.name, row.rare))
            .collect(),
        MaterialKind::BlockPackage => tables
            .block_package
            .iter()
            .map(|row| catalog_entry(tables, kind, row.id, &row.name, row.rare))
            .collect(),
        MaterialKind::PlayerExp => vec![catalog_entry(tables, kind, 1, "Player EXP", 0)],
        MaterialKind::Hero => tables
            .character
            .iter()
            .map(|row| {
                let raw_name = if row.name_eng.is_empty() {
                    &row.name
                } else {
                    &row.name_eng
                };
                catalog_entry(tables, kind, row.id, raw_name, row.rare)
            })
            .collect(),
        MaterialKind::Skin => tables
            .skin
            .iter()
            .filter(|row| is_premium_hero_skin(tables, row))
            .filter(|row| !owned_skins.is_some_and(|owned| owned.contains(&row.id)))
            .map(|row| {
                let raw_name = if row.name_eng.is_empty() {
                    &row.name
                } else {
                    &row.name_eng
                };
                catalog_entry(tables, kind, row.id, raw_name, row.rare)
            })
            .collect(),
        MaterialKind::Equipment => tables
            .equip
            .iter()
            .map(|row| {
                let raw_name = if row.name_en.is_empty() {
                    &row.name
                } else {
                    &row.name_en
                };
                catalog_entry(tables, kind, row.id, raw_name, row.rare)
            })
            .collect(),
        MaterialKind::PowerItem => tables
            .power_item
            .iter()
            .map(|row| catalog_entry(tables, kind, row.id, &row.name, row.rare))
            .collect(),
        MaterialKind::InsightItem => tables
            .insight_item
            .iter()
            .map(|row| catalog_entry(tables, kind, row.id, &row.name, row.rare))
            .collect(),
    }
}

pub(super) fn is_premium_hero_skin(tables: &config::GameDB, skin: &config::skin::Skin) -> bool {
    tables.character.get(skin.character_id).is_some() && skin.skin_level > 0
}

fn catalog_entry(
    tables: &config::GameDB,
    kind: MaterialKind,
    id: i32,
    raw_name: &str,
    rare: i32,
) -> CatalogEntry {
    let name = resolve_name(tables, raw_name);
    CatalogEntry {
        r#type: kind.id(),
        id,
        name,
        raw_name: raw_name.to_string(),
        rare,
    }
}

pub(super) fn resolve_name(tables: &config::GameDB, raw_name: &str) -> String {
    let resolved = tables.language_en.get(raw_name).unwrap_or(raw_name);

    clean_name(resolved)
}

fn clean_name(value: &str) -> String {
    let mut cleaned = String::with_capacity(value.len());
    let mut in_tag = false;
    for ch in value.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            '\r' | '\n' if !in_tag => cleaned.push(' '),
            _ if !in_tag => cleaned.push(ch),
            _ => {}
        }
    }
    cleaned.trim().to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub(super) enum MaterialKind {
    Item = 1,
    Currency = 2,
    PlayerExp = 3,
    Hero = 4,
    Skin = 5,
    Equipment = 9,
    PowerItem = 10,
    BlockPackage = 13,
    InsightItem = 24,
}

impl MaterialKind {
    pub(super) fn parse(value: &str) -> Option<Self> {
        if let Ok(raw) = value.parse::<i32>() {
            return Self::from_raw(raw);
        }

        match value.to_ascii_lowercase().as_str() {
            "item" | "items" => Some(Self::Item),
            "currency" | "currencies" | "coin" => Some(Self::Currency),
            "exp" | "playerexp" => Some(Self::PlayerExp),
            "hero" | "heroes" => Some(Self::Hero),
            "skin" | "skins" => Some(Self::Skin),
            "equip" | "equipment" | "psychube" | "psychubes" => Some(Self::Equipment),
            "power" | "poweritem" => Some(Self::PowerItem),
            "blockpackage" | "package" => Some(Self::BlockPackage),
            "insight" | "insightitem" => Some(Self::InsightItem),
            _ => None,
        }
    }

    pub(super) fn from_raw(raw: i32) -> Option<Self> {
        match raw {
            raw if raw == Self::Item as i32 => Some(Self::Item),
            raw if raw == Self::Currency as i32 => Some(Self::Currency),
            raw if raw == Self::PlayerExp as i32 => Some(Self::PlayerExp),
            raw if raw == Self::Hero as i32 => Some(Self::Hero),
            raw if raw == Self::Skin as i32 => Some(Self::Skin),
            raw if raw == Self::Equipment as i32 => Some(Self::Equipment),
            raw if raw == Self::PowerItem as i32 => Some(Self::PowerItem),
            raw if raw == Self::BlockPackage as i32 => Some(Self::BlockPackage),
            raw if raw == Self::InsightItem as i32 => Some(Self::InsightItem),
            _ => None,
        }
    }

    pub(super) fn id(self) -> i32 {
        self as i32
    }

    fn label(self) -> &'static str {
        match self {
            Self::Item => "item",
            Self::Currency => "currency",
            Self::PlayerExp => "playerExp",
            Self::Hero => "hero",
            Self::Skin => "heroSkin",
            Self::Equipment => "equipment",
            Self::PowerItem => "powerItem",
            Self::BlockPackage => "blockPackage",
            Self::InsightItem => "insightItem",
        }
    }
}
