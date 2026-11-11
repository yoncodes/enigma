use crate::engine::{
    entity::stats::configured_rank, manager::BattleManagers, skill::target::TargetPool,
};

use super::ParsedConditionKind;

pub(super) fn minimum(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [value] = args else {
        return None;
    };
    let rank = value.parse().ok()?;
    (rank > 0).then_some(ParsedConditionKind::RankAtLeast(rank))
}

pub(super) fn matches(
    minimum: i32,
    targets: &[i64],
    managers: &BattleManagers,
    pool: &TargetPool,
) -> bool {
    let game = managers.catalog().game_data();
    targets
        .iter()
        .filter_map(|uid| pool.entity(*uid))
        .any(|entity| {
            if entity.level <= 0 || game.character.get(entity.model_id).is_none() {
                return false;
            }
            let rank = configured_rank(game, entity.model_id, entity.level);
            rank >= minimum
                && game
                    .character_rank_level_limit(entity.model_id, rank)
                    .is_some_and(|maximum| entity.level <= maximum)
        })
}

#[cfg(test)]
mod tests;
