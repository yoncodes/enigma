use super::*;
use crate::engine::skill::action::{SkillExecutionMode, SkillInvocation};

pub(in crate::engine::runtime) fn run_extra_round_skill(
    fight: &sonettobuf::Fight,
    battle: BattleAccess<'_>,
    round: ActionRound,
    card_index: i32,
) -> Result<DrainResult, DrainError> {
    let BattleAccess {
        managers,
        pool,
        catalog,
        determinism,
        context,
    } = battle;
    let ActionRound::AlliedExtra(skill) = round else {
        return Ok(DrainResult::default());
    };
    if managers.hp.current(skill.source_uid) <= 0
        || crate::engine::round::outcome::battle_ended(fight, pool, managers)
    {
        return Ok(DrainResult::default());
    }

    let mut invocation = SkillInvocation::from(skill);
    invocation.mode = SkillExecutionMode::Active;
    invocation.card_index = card_index;
    drain::run_round_action(
        BattleAccess::new(managers, pool, catalog, determinism, context),
        [],
        None,
        invocation,
    )
}
