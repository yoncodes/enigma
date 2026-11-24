use crate::engine::skill::target::TargetPool;

const ALL_ROUNDER_TAG: i32 = 10_000;
const ALL_ROUNDER_MAPPING: i32 = 61;

pub(super) fn allies(pool: &TargetPool, source_uid: i64, tag: i32) -> Vec<i64> {
    pool.allies(source_uid)
        .iter()
        .filter(|entity| {
            entity.battle_tags.contains(&tag)
                || (entity.battle_tags.contains(&ALL_ROUNDER_TAG) && mapped(pool, tag))
        })
        .map(|entity| entity.uid)
        .collect()
}

fn mapped(pool: &TargetPool, tag: i32) -> bool {
    pool.catalog()
        .game_data()
        .fight_const
        .get(ALL_ROUNDER_MAPPING)
        .is_some_and(|row| row.value.split(',').any(|id| id.trim().parse() == Ok(tag)))
}
