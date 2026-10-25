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
    if crate::engine::fight::versions::round_start_setup_layout(managers.fight_version())
        == Some(crate::engine::fight::versions::RoundStartSetupLayout::Version7)
    {
        let current_pool = pool.runtime_view(managers);
        let basics = crate::engine::manager::card::pool::target_basic_candidate_pool(
            &current_pool.attacker_main,
        );
        if basics.len() == current_pool.attacker_main.len().saturating_mul(2) {
            crate::engine::manager::card::start::preserve_basic_coverage(
                &basics,
                &mut replacements,
            );
        }
    }
    RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Card(
        crate::engine::manager::card::CardCommand::ApplyRedealKeepRanks(
            crate::engine::manager::card::CardRedealKeepRanks {
                origin,
                replacements,
            },
        ),
    ))
}
