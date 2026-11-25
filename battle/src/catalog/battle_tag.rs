use super::BattleCatalog;

const ALL_ROUNDER_TAG: i32 = 10_000;
const ALL_ROUNDER_MAPPING: i32 = 61;

impl BattleCatalog {
    pub(crate) fn entity_battle_tags(
        self,
        model_id: i32,
        destiny_stone: i32,
        destiny_rank: i32,
    ) -> Vec<i32> {
        let stone_tags = if destiny_stone <= 0 || destiny_rank <= 0 {
            None
        } else {
            self.game_data
                .character_destiny_facets_consume
                .iter()
                .find(|row| row.facets_id == destiny_stone)
                .and_then(|row| {
                    let tags = row
                        .tag
                        .split('#')
                        .filter_map(|tag| tag.parse().ok())
                        .collect::<Vec<_>>();
                    (!tags.is_empty()).then_some(tags)
                })
        };
        let mut tags = stone_tags.unwrap_or_else(|| {
            self.game_data
                .character
                .get(model_id)
                .map(|character| {
                    character
                        .battle_tag
                        .split('#')
                        .filter_map(|tag| tag.parse().ok())
                        .collect()
                })
                .unwrap_or_default()
        });
        if tags.contains(&ALL_ROUNDER_TAG)
            && let Some(mapping) = self.game_data.fight_const.get(ALL_ROUNDER_MAPPING)
        {
            tags.extend(
                mapping
                    .value
                    .split(',')
                    .filter_map(|tag| tag.trim().parse::<i32>().ok()),
            );
        }
        tags.sort_unstable();
        tags.dedup();
        tags
    }
}
