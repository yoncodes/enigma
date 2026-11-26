use crate::engine::{
    fight::reserve::PromotionBatch, manager::BattleManagers, skill::buff_act::registry::BuffActKind,
};

#[derive(Clone, Copy)]
pub(crate) enum Window<'a> {
    ActionDrain,
    AfterAction,
    AfterReaction(&'a [i64]),
    EndOfPhase,
}

pub(crate) fn promote(managers: &mut BattleManagers, window: Window<'_>) -> PromotionBatch {
    let mut promotions = Vec::new();
    loop {
        let battle_rule = &managers.battle_rule;
        let buffs = &managers.buff;
        let batch =
            managers
                .entity
                .promote_reserves_matching(&managers.hp, |uid, team| match window {
                    Window::ActionDrain => battle_rule.promotes_reserves_immediately(team),
                    Window::AfterAction | Window::AfterReaction(_) => {
                        !battle_rule.promotes_reserves_immediately(team)
                            && buffs.has_buff_act_kind(uid, BuffActKind::CycleToSub)
                            && match window {
                                Window::AfterReaction(defeated) => defeated.contains(&uid),
                                _ => true,
                            }
                    }
                    Window::EndOfPhase => true,
                });
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
            managers.buff.sync_promotion(&promotion);
            if let Some(entering) = managers.entity_snapshot(promotion.entering_uid) {
                promotion.entering = entering;
            }
            promotions.push(promotion);
        }
    }
    PromotionBatch::new(promotions)
}
