use std::{collections::HashMap, sync::OnceLock};

pub struct Destiny;

type DestinyCache = HashMap<(i32, i32), HashMap<i32, i32>>;

static DESTINY_CACHE: OnceLock<DestinyCache> = OnceLock::new();

impl Destiny {
    pub fn stones(game: &config::GameDB, hero_id: i32) -> Vec<i32> {
        game.character_destiny
            .iter()
            .find(|row| row.hero_id == hero_id)
            .map(|row| {
                row.facets_id
                    .split('#')
                    .filter_map(|id| id.parse().ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn stones_for_hero(hero_id: i32) -> Vec<i32> {
        Self::stones(crate::catalog::BattleCatalog::global().game_data(), hero_id)
    }

    pub fn rank_limit(game: &config::GameDB, facets_id: i32) -> i32 {
        game.character_destiny_facets
            .iter()
            .filter(|row| row.facets_id == facets_id)
            .map(|row| row.level)
            .max()
            .unwrap_or_default()
    }

    pub fn max_rank(facets_id: i32) -> i32 {
        Self::rank_limit(
            crate::catalog::BattleCatalog::global().game_data(),
            facets_id,
        )
    }

    pub fn exchanges(
        game: &config::GameDB,
        facets_id: i32,
        rank: i32,
    ) -> Option<HashMap<i32, i32>> {
        if facets_id <= 0 || rank <= 0 {
            return None;
        }
        let mut rows = game
            .character_destiny_facets
            .iter()
            .filter(|row| row.facets_id == facets_id && row.level > 0 && row.level <= rank)
            .collect::<Vec<_>>();
        rows.sort_by_key(|row| row.level);
        if rows.is_empty() {
            return None;
        }
        let mut exchanges = HashMap::new();
        for row in rows {
            Self::parse_exchange_into(&row.exchange_skills, &mut exchanges);
        }
        Some(exchanges)
    }

    pub fn get(facets_id: i32, rank: i32) -> Option<HashMap<i32, i32>> {
        Self::get_ref(facets_id, rank).cloned()
    }

    pub fn get_ref(facets_id: i32, rank: i32) -> Option<&'static HashMap<i32, i32>> {
        if facets_id <= 0 || rank <= 0 {
            return None;
        }

        let cache = DESTINY_CACHE.get_or_init(build_cache);
        cache.get(&(facets_id, rank)).or_else(|| {
            cache
                .iter()
                .filter_map(|(&(id, level), map)| {
                    (id == facets_id && level <= rank).then_some((level, map))
                })
                .max_by_key(|(level, _)| *level)
                .map(|(_, map)| map)
        })
    }

    pub fn battle_tags(facets_id: i32, rank: i32) -> Option<Vec<i32>> {
        if facets_id <= 0 || rank <= 0 {
            return None;
        }

        crate::catalog::BattleCatalog::global()
            .game_data()
            .character_destiny_facets_consume
            .iter()
            .find(|row| row.facets_id == facets_id)
            .and_then(|row| {
                let tags = row
                    .tag
                    .split('#')
                    .filter_map(|tag| tag.parse().ok())
                    .collect::<Vec<_>>();
                (!tags.is_empty()).then_some(tags)
            })
    }

    fn parse_exchange_into(s: &str, map: &mut HashMap<i32, i32>) {
        for pair in s.split('|') {
            if let Some((old, new)) = pair.split_once('#')
                && let (Ok(o), Ok(n)) = (old.parse(), new.parse())
            {
                map.insert(o, n);
            }
        }
    }
}

fn build_cache() -> DestinyCache {
    let game = crate::catalog::BattleCatalog::global().game_data();
    let mut grouped: HashMap<i32, Vec<(i32, &str)>> = HashMap::new();

    for row in game.character_destiny_facets.iter() {
        if row.facets_id > 0 && row.level > 0 {
            grouped
                .entry(row.facets_id)
                .or_default()
                .push((row.level, row.exchange_skills.as_str()));
        }
    }

    let mut cache = HashMap::new();
    for (facets_id, mut rows) in grouped {
        rows.sort_by_key(|(level, _)| *level);

        let mut cumulative = HashMap::new();
        for (level, exchange_skills) in rows {
            Destiny::parse_exchange_into(exchange_skills, &mut cumulative);
            cache.insert((facets_id, level), cumulative.clone());
        }
    }

    cache
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_stone_uses_its_max_rank() {
        crate::test_support::init_config();
        assert_eq!(Destiny::stones_for_hero(3074), vec![307401, 307402]);
        assert_eq!(Destiny::max_rank(307401), 4);
        assert_eq!(
            Destiny::stones(crate::test_support::game_data(), 3074),
            Destiny::stones_for_hero(3074)
        );
        assert_eq!(
            Destiny::rank_limit(crate::test_support::game_data(), 307401),
            Destiny::max_rank(307401)
        );
        assert_eq!(
            Destiny::exchanges(crate::test_support::game_data(), 307401, 4),
            Destiny::get(307401, 4)
        );
    }

    #[test]
    fn selected_isolde_stone_grants_lingering_glow_membership() {
        crate::test_support::init_config();
        assert_eq!(Destiny::battle_tags(308101, 0), None);
        assert_eq!(Destiny::battle_tags(308101, 1), Some(vec![102, 114, 116]));
        assert_eq!(Destiny::battle_tags(308101, 4), Some(vec![102, 114, 116]));
    }
}
