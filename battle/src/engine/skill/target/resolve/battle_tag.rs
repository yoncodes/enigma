use crate::engine::skill::target::TargetPool;

pub(super) fn allies(pool: &TargetPool, source_uid: i64, tag: i32) -> Vec<i64> {
    pool.allies(source_uid)
        .iter()
        .filter(|entity| entity.battle_tags.contains(&tag))
        .map(|entity| entity.uid)
        .collect()
}
