pub mod defender;
pub mod reserve;
pub mod rules;
pub mod team;
pub mod trigger;
pub(crate) mod versions;

#[cfg(test)]
mod test;

#[cfg(test)]
pub(crate) fn configured_battle(
    fight: &sonettobuf::Fight,
) -> Option<&'static config::battle::Battle> {
    configured_battle_with_game_data(
        crate::catalog::BattleCatalog::try_global()?.game_data(),
        fight,
    )
}

pub(crate) fn configured_battle_with_game_data<'a>(
    db: &'a config::GameDB,
    fight: &sonettobuf::Fight,
) -> Option<&'a config::battle::Battle> {
    match fight.battle_id {
        Some(battle_id) => db.battle.get(battle_id),
        None => fight
            .episode_id
            .and_then(|episode_id| db.episode.get(episode_id))
            .and_then(|episode| db.battle.get(episode.battle_id)),
    }
}
