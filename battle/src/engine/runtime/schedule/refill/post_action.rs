use super::*;

#[derive(Default)]
pub(crate) struct RefillSlots {
    committed: usize,
    played: Vec<i32>,
}

impl RefillSlots {
    pub(crate) fn capture(managers: &BattleManagers, pool: &TargetPool) -> Self {
        let ignores_draw_limit =
            crate::engine::mechanic::card::CardMechanic.round_draw_waiver(managers);
        let exempt_ultimate = |card: &sonettobuf::CardInfo| {
            pool.entity(card.uid.unwrap_or_default())
                .is_some_and(|entity| {
                    managers.buff.has_buff_act_kind(
                        entity.uid,
                        buff_act::registry::BuffActKind::EntityExSkillNotCalSize,
                    ) && crate::engine::mechanic::card::CardMechanic
                        .is_ultimate(managers, card, entity)
                })
        };
        Self {
            committed: managers
                .card
                .committed_hand()
                .iter()
                .filter(|card| !card.temp_card.unwrap_or_default() && !ignores_draw_limit(card))
                .count(),
            played: managers
                .card
                .played()
                .iter()
                .filter(|played| {
                    !played.card.temp_card.unwrap_or_default()
                        && played.hand_index.is_some()
                        && !ignores_draw_limit(&played.card)
                        && !exempt_ultimate(&played.card)
                })
                .map(|played| played.card_index)
                .collect(),
        }
    }

    pub(in crate::engine::runtime::schedule) fn replacements(
        &self,
        managers: &BattleManagers,
    ) -> usize {
        managers
            .card
            .played()
            .iter()
            .filter(|played| played.uses_action_point && self.played.contains(&played.card_index))
            .count()
    }

    pub(crate) fn target(
        &self,
        managers: &BattleManagers,
        pool: &TargetPool,
        base_hand_size: usize,
    ) -> usize {
        if managers.card.played().is_empty() {
            return base_hand_size;
        }
        let opening_deficit = base_hand_size.saturating_sub(self.committed);
        crate::engine::mechanic::card::CardMechanic
            .refill_hand_len(managers, pool)
            .saturating_add(self.replacements(managers))
            .saturating_add(opening_deficit)
            .max(base_hand_size)
    }
}

#[cfg(test)]
mod tests;

#[allow(clippy::too_many_arguments)]
pub fn run_post_action_refill(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    target_hand_len: usize,
    team_type: i32,
) -> Result<DrainResult, DrainError> {
    super::run_card_refill(
        managers,
        pool,
        catalog,
        determinism,
        context,
        target_hand_len,
        team_type,
        CardRefillStage::AfterActions,
        Vec::new(),
    )
}
