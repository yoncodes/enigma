use crate::engine::manager::{
    BattleManagers,
    entity::RosterLane,
    fight_task::{FightTaskChange, MonsterHealth},
};

pub(super) fn settle_death(
    managers: &mut BattleManagers,
    target_uid: i64,
    check_health: bool,
) -> [Option<FightTaskChange>; 2] {
    let entities = &managers.entity;
    let hp = &managers.hp;
    let below_hp = if check_health && entities.team_type(target_uid) == Some(2) {
        managers.fight_task.record_monster_health(
            || {
                entities
                    .ordered_uids()
                    .filter(move |uid| {
                        entities.team_type(*uid) == Some(2)
                            && entities.roster_lane(*uid) == Some(RosterLane::Main)
                    })
                    .filter_map(move |uid| {
                        Some(MonsterHealth {
                            model_id: entities.model_id(uid)?,
                            current_hp: hp.current(uid),
                            max_hp: hp.max(uid),
                        })
                    })
            },
            1,
        )
    } else {
        None
    };
    let killed = entities
        .model_id(target_uid)
        .and_then(|model_id| managers.fight_task.record_kill(model_id, 1));
    [below_hp, killed]
}
