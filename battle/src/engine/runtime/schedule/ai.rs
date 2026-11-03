use super::*;

pub(super) fn run_ai_preparation(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
) -> Result<DrainResult, DrainError> {
    if managers.card.prepared_ai_skills().is_empty() {
        return Ok(DrainResult::default());
    }
    let live_pool = pool.runtime_view(managers);
    let mut prepared = crate::engine::manager::card::prepared::cards(
        &live_pool,
        managers.card.prepared_ai_skills(),
    )
    .map_err(DrainError::AiPreparation)?;
    crate::engine::manager::card::ai::resolve_targets(
        pool,
        managers,
        catalog,
        determinism,
        context,
        &mut prepared,
    )
    .map_err(DrainError::AiPreparation)?;
    let mut cards = managers.card.ai_queue().to_vec();
    cards.extend(prepared);
    drain::run(
        managers,
        pool,
        catalog,
        determinism,
        context,
        [RuleOp::Command(BattleCommand::Card(
            CardCommand::SetAiQueue(CardSetAiQueue {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "AiSkillPreparation"),
                },
                cards,
            }),
        ))],
    )
}
