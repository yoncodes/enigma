use crate::engine::{
    manager::BattleManagers,
    runtime::determinism::RoundDeterminism,
    skill::{
        buff_act::registry::BuffActKind,
        target::{TargetEntity, TargetPool},
    },
};

pub(super) fn candidates(
    pool: &TargetPool,
    source_uid: i64,
    managers: Option<&BattleManagers>,
) -> Vec<TargetEntity> {
    let Some(managers) = managers else {
        return Vec::new();
    };
    let enemies = pool
        .enemies(source_uid, false)
        .iter()
        .filter(|enemy| !pool.is_reserve(enemy.uid))
        .collect::<Vec<_>>();
    let amount =
        |enemy: &&TargetEntity| managers.buff.buff_act_amount(enemy.uid, BuffActKind::Burn);
    let Some(maximum) = enemies.iter().map(amount).max() else {
        return Vec::new();
    };
    enemies
        .into_iter()
        .filter(|enemy| amount(enemy) == maximum)
        .cloned()
        .collect()
}

pub(super) fn select(candidates: &[TargetEntity], determinism: &mut RoundDeterminism) -> Vec<i64> {
    if let [candidate] = candidates {
        return vec![candidate.uid];
    }
    determinism
        .lua_random_index(candidates.len())
        .map(|index| candidates[index].uid)
        .into_iter()
        .collect()
}
