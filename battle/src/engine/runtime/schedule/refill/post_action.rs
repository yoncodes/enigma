use super::*;

pub fn replacement_count(managers: &BattleManagers) -> usize {
    managers
        .card
        .played()
        .iter()
        .filter(|played| {
            !played.card.temp_card.unwrap_or_default()
                && played.hand_index.is_some()
                && played.uses_action_point
        })
        .count()
}

pub fn target_hand_len(
    managers: &BattleManagers,
    pool: &TargetPool,
    base_hand_size: usize,
) -> usize {
    if managers.card.played().is_empty() {
        return base_hand_size;
    }
    let committed = managers
        .card
        .committed_hand()
        .iter()
        .filter(|card| !card.temp_card.unwrap_or_default())
        .count();
    let opening_deficit = base_hand_size.saturating_sub(committed);
    crate::engine::mechanic::card::CardMechanic
        .refill_hand_len(managers, pool)
        .saturating_add(replacement_count(managers))
        .saturating_add(opening_deficit)
}

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
        RefillStage::AfterActions,
        Vec::new(),
    )
}
