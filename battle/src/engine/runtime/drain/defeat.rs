use crate::engine::{
    event::payload::EntityDiedEvent, manager::hp::PendingDeath, mechanic::array_lifetime,
    runtime::change::BattleChange,
};

use super::*;

pub(super) fn settle_defeat(
    access: DrainAccess<'_>,
    path: &[usize],
    death: PendingDeath,
) -> Result<(), DrainError> {
    let DrainAccess {
        managers,
        pool,
        catalog,
        determinism,
        result,
        state,
    } = access;
    if !managers.hp.is_current_death(death) {
        return Ok(());
    }
    managers.conduit.record_death(death);
    let check_health = state.is_primary_death(death);
    let death = death.transition();
    for change in fight_tasks::settle_death(managers, death.target_uid, check_health)
        .into_iter()
        .flatten()
    {
        push_change(&mut result.frames, path, BattleChange::FightTask(change));
    }
    push_change(&mut result.frames, path, BattleChange::Death(death));

    let trigger = SkillOpTrigger::Event(BattleEvent::EntityDied(EntityDiedEvent {
        source_uid: death.source_uid,
        target_uid: death.target_uid,
    }));
    let queue: VecDeque<_> = array_lifetime::caster_defeated(managers, death.target_uid)
        .into_iter()
        .map(|op| QueuedOp {
            op,
            trigger: trigger.clone(),
            skill_execution: None,
            frame_path: Some(path.to_vec()),
            parent_path: None,
            frame_group: None,
            independent_parent_group: None,
            frame_owner: None,
            subscriber: None,
            caster_frame: None,
        })
        .collect();
    if !queue.is_empty() {
        drain_nested_queue(managers, pool, catalog, determinism, queue, result, state)?;
    }
    Ok(())
}
