use super::*;

pub(super) fn prepare_command(
    managers: &BattleManagers,
    pool: &TargetPool,
    determinism: &mut RoundDeterminism,
    command: RuleOp,
) -> RuleOp {
    let origin = match command {
        RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Card(
            crate::engine::manager::card::CardCommand::RedealKeepRanks { origin },
        )) => origin,
        command => return command,
    };
    let draw_pile = managers.card.draw_pile().to_vec();
    let mut replacements = Vec::new();
    for _ in managers.card.redealable_cards() {
        replacements.extend(determinism.draw_cards(&draw_pile, 1));
    }
    let basics = crate::engine::mechanic::card::CardMechanic.redeal_basics(managers, pool);
    crate::engine::manager::card::start::preserve_basic_coverage(&basics, &mut replacements);
    RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Card(
        crate::engine::manager::card::CardCommand::ApplyRedealKeepRanks(
            crate::engine::manager::card::CardRedealKeepRanks {
                origin,
                replacements,
            },
        ),
    ))
}
