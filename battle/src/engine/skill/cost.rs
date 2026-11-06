use crate::engine::{
    manager::BattleManagers, mechanic::card::CardMechanic, skill::target::TargetEntity,
};

pub(crate) fn moxie(managers: &BattleManagers, entity: &TargetEntity, skill_id: i32) -> i32 {
    if managers.catalog().skill_moxie_exempt(skill_id) {
        0
    } else {
        CardMechanic.required_ultimate_cost(managers, entity)
    }
}
