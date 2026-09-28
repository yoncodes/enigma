use battle::engine::{
    manager::{BattleManagers, card::hand_size},
    mechanic::card::CardMechanic,
    runtime::determinism::{HandRankChoice, RoundDeterminism},
    skill::effect::SkillEffectCatalog,
};
use sonettobuf::{CardInfo, Fight, FightRound, FightStep};

/// Imports observed random choices while leaving card construction and validation to the engine.
pub fn opening_determinism(
    game_data: &'static config::GameDB,
    fight: &Fight,
    round: &FightRound,
) -> RoundDeterminism {
    let mut determinism =
        RoundDeterminism::with_seed(fight.battle_id.unwrap_or_default().max(0) as u64);
    determinism.enqueue_hand_rank_choices(opening_hand_rank_choices(game_data, fight, round));
    let draws = round
        .team_a_cards1
        .iter()
        .filter(|card| !card.temp_card.unwrap_or_default())
        .cloned()
        .collect::<Vec<_>>();
    let ultimate_identities = fight
        .attacker
        .iter()
        .flat_map(|team| &team.entitys)
        .filter_map(|entity| Some((entity.uid?, entity.ex_skill?)))
        .collect::<std::collections::HashSet<_>>();
    let managers =
        BattleManagers::seeded_with_catalog(battle::catalog::BattleCatalog::new(game_data), fight);
    let reserved_ultimate_slots = draws
        .iter()
        .filter(|card| {
            card.uid
                .zip(card.skill_id)
                .is_some_and(|identity| ultimate_identities.contains(&identity))
        })
        .filter(|card| {
            !CardMechanic.ultimate_ignores_limit(
                &managers,
                card.uid.unwrap_or_default(),
                card.skill_id.unwrap_or_default(),
            )
        })
        .count();
    let normal_draws = draws
        .into_iter()
        .filter(|card| {
            !card
                .uid
                .zip(card.skill_id)
                .is_some_and(|identity| ultimate_identities.contains(&identity))
        })
        .collect::<Vec<CardInfo>>();
    let player_seed_len = hand_size(fight).saturating_sub(reserved_ultimate_slots);

    if normal_draws.len() >= player_seed_len {
        determinism.enqueue_opening_seed(
            round.ai_use_cards.clone(),
            normal_draws.iter().take(player_seed_len).cloned().collect(),
            normal_draws,
            reserved_ultimate_slots,
        );
    }
    determinism
}

pub fn seed_round_determinism(
    runtime: &mut battle::engine::runtime::BattleRuntime,
    catalog: &SkillEffectCatalog,
    round: &FightRound,
) {
    runtime.seed_card_draws(round.team_a_cards2.clone());
    runtime.seed_crystal_cards(
        round
            .before_cards1
            .iter()
            .filter(|card| card.temp_card.unwrap_or_default())
            .cloned(),
    );
    if runtime.fight_version() == 7 {
        runtime.seed_next_ai_cards(round.ai_use_cards.clone());
    }
    runtime.seed_random_skills(random_skill_choices(catalog, round));
}

fn random_skill_choices(catalog: &SkillEffectCatalog, round: &FightRound) -> Vec<i32> {
    fn visit(catalog: &SkillEffectCatalog, step: &FightStep, choices: &mut Vec<i32>) {
        let references = catalog.random_skill_references(step.act_id.unwrap_or_default());
        for child in step
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
        {
            if child
                .act_id
                .is_some_and(|act_id| act_id > 0 && references.contains(&act_id))
            {
                choices.push(child.act_id.unwrap());
            }
            visit(catalog, child, choices);
        }
    }

    let mut choices = Vec::new();
    for step in &round.fight_step {
        visit(catalog, step, &mut choices);
    }
    choices
}

fn opening_hand_rank_choices(
    game_data: &'static config::GameDB,
    fight: &Fight,
    round: &FightRound,
) -> Vec<HandRankChoice> {
    const OPCODE: i32 = 50011;
    const EFFECT: i32 = sonettobuf::effect_type_enum::EffectType::Cardlevelchange as i32;

    fn collect_skills(step: &FightStep, skills: &mut Vec<i32>) {
        skills.extend(step.act_id.filter(|skill_id| *skill_id > 0));
        for child in step
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
        {
            collect_skills(child, skills);
        }
    }

    fn collect_choices(
        catalog: &SkillEffectCatalog,
        step: &FightStep,
        choices: &mut Vec<HandRankChoice>,
    ) {
        let random_rank = step
            .act_id
            .and_then(|skill_id| catalog.get(skill_id))
            .is_some_and(|effect| {
                let mode_one = |slot: &battle::engine::skill::effect::SkillEffectSlot| {
                    matches!(slot.behavior.args.as_slice(), [1, count, 1] if *count > 0)
                };
                let mut slots = effect.slots.iter().filter(|slot| {
                    slot.behavior.spec.key.opcode == OPCODE
                        && slot.behavior.spec.key.type_name == "CardLevelChange"
                });
                slots.next().is_some_and(|slot| mode_one(slot) && slots.all(mode_one))
            });
        if random_rank {
            choices.extend(step.act_effect.iter().filter_map(|effect| {
                if effect.effect_type != Some(EFFECT) || effect.config_effect != Some(OPCODE) {
                    return None;
                }
                Some(HandRankChoice {
                    opcode: OPCODE,
                    owner_uid: effect.entity.as_ref()?.uid?,
                    hand_index: usize::try_from(effect.target_id?.checked_sub(1)?).ok()?,
                })
            }));
        }
        for child in step
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
        {
            collect_choices(catalog, child, choices);
        }
    }

    let mut skills = Vec::new();
    for step in &round.fight_step {
        collect_skills(step, &mut skills);
    }
    let mut catalog = SkillEffectCatalog::from_fight(game_data, fight);
    catalog.extend_roots(game_data, skills, []);
    let mut choices = Vec::new();
    for step in &round.fight_step {
        collect_choices(&catalog, step, &mut choices);
    }
    choices
}

#[cfg(test)]
mod tests {
    use sonettobuf::{FightEntityInfo, FightTeam};

    use super::*;

    fn card(uid: i64, skill_id: i32, temporary: bool) -> CardInfo {
        CardInfo {
            uid: Some(uid),
            skill_id: Some(skill_id),
            temp_card: Some(temporary),
            ..Default::default()
        }
    }

    #[test]
    fn opening_rng_import_keeps_choices_and_excludes_temporary_cards() {
        crate::init_test_config();
        let fight = Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    skill_group1: vec![101],
                    skill_group2: vec![102],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(100),
                    skill_group1: vec![201],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let normal = vec![
            card(10, 101, false),
            card(10, 102, false),
            card(10, 101, false),
            card(10, 102, false),
        ];
        let ai = vec![card(-1, 201, false)];
        let mut captured = normal.clone();
        captured.insert(2, card(10, 999, true));

        let mut determinism = opening_determinism(
            config::configs::get(),
            &fight,
            &FightRound {
                team_a_cards1: captured,
                ai_use_cards: ai.clone(),
                ..Default::default()
            },
        );

        assert_eq!(
            determinism.take_start_decks(),
            Some((ai, normal.clone(), normal, 0))
        );
    }
}
