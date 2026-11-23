use crate::engine::{fight::reserve::PromotionBatch, manager::BattleManagers};

pub(crate) fn promote(
    managers: &mut BattleManagers,
    promotes: impl Fn(i32) -> bool + Copy,
) -> PromotionBatch {
    let mut promotions = Vec::new();
    loop {
        let batch = managers
            .entity
            .promote_immediate_reserves(&managers.hp, promotes);
        if batch.is_empty() {
            break;
        }
        for mut promotion in batch {
            let plan = managers
                .buff
                .active_act_definitions(promotion.defeated_uid)
                .find_map(|definition| definition.state.reserve_promotion);
            if let Some(plan) = plan {
                promotion.queued_reserve = Some(managers.queue_reserve_cycle(plan(&promotion)));
            }
            if let Some(entering) = managers.entity_snapshot(promotion.entering_uid) {
                promotion.entering = entering;
            }
            promotions.push(promotion);
        }
    }
    PromotionBatch::new(promotions)
}
