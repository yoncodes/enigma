use battle::tooling::{
    opening_hand_size,
    replay::{HandRankChoice, ReplayBattle, RoundDeterminism},
    scan::{SkillEffectCatalog, SkillEffectSlot},
    ultimate_ignores_limit,
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
    let reserved_ultimate_slots = draws
        .iter()
        .filter(|card| {
            card.uid
                .zip(card.skill_id)
                .is_some_and(|identity| ultimate_identities.contains(&identity))
        })
        .filter(|card| {
            !ultimate_ignores_limit(
                battle::catalog::BattleCatalog::new(game_data),
                fight,
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
    let player_seed_len = opening_hand_size(fight).saturating_sub(reserved_ultimate_slots);

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
    runtime: &mut ReplayBattle,
    catalog: &SkillEffectCatalog,
    round: &FightRound,
) {
    // The deal after actions draws teamACards2; the round-start refill then draws teamACards1.
    runtime.seed_card_draws(
        round
            .team_a_cards2
            .iter()
            .chain(&round.team_a_cards1)
            .cloned()
            .collect(),
    );
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
    let (hidden, additional) = crit_choices(round);
    runtime.seed_crits(hidden, additional);
}

type HiddenCrit = ((i32, i64), bool);
type AdditionalCrit = ((i32, i64, i64), bool);

// Each effect the engine rolls a crit for is rolled under its own step's skill (or buff) and
// source, in step order. Config effects name the rolling source: skill row damage (-1), healing
// behaviors 20001/90001, Spirit Shell heals (0), and crit-capable origin damage 30015/60127.
fn crit_choices(round: &FightRound) -> (Vec<HiddenCrit>, Vec<AdditionalCrit>) {
    use sonettobuf::effect_type_enum::EffectType;

    fn visit(step: &FightStep, hidden: &mut Vec<HiddenCrit>, additional: &mut Vec<AdditionalCrit>) {
        let key = step
            .act_id
            .filter(|act_id| *act_id > 0)
            .zip(step.from_id.filter(|from_id| *from_id != 0));
        for effect in &step.act_effect {
            if let Some((act_id, from_id)) = key {
                let effect_type = effect.effect_type.unwrap_or_default();
                let crit = |normal: EffectType, crit: EffectType| {
                    [(normal as i32, false), (crit as i32, true)]
                        .into_iter()
                        .find_map(|(kind, is_crit)| (kind == effect_type).then_some(is_crit))
                };
                if let Some(is_crit) = crit(
                    EffectType::Additionaldamage,
                    EffectType::Additionaldamagecrit,
                ) {
                    additional.push((
                        (act_id, from_id, effect.target_id.unwrap_or_default()),
                        is_crit,
                    ));
                } else {
                    let config_effect = effect.config_effect.unwrap_or_default();
                    let rolled = crit(EffectType::Damage, EffectType::Crit)
                        .filter(|_| config_effect == -1)
                        .or_else(|| {
                            crit(EffectType::Heal, EffectType::Healcrit)
                                .filter(|_| matches!(config_effect, 0 | 20001 | 90001))
                        })
                        .or_else(|| {
                            crit(EffectType::Origindamage, EffectType::Origincrit)
                                .filter(|_| matches!(config_effect, 30015 | 60127))
                        });
                    if let Some(is_crit) = rolled {
                        hidden.push(((act_id, from_id), is_crit));
                    }
                }
            }
            if let Some(child) = effect.fight_step.as_ref() {
                visit(child, hidden, additional);
            }
        }
    }

    let mut hidden = Vec::new();
    let mut additional = Vec::new();
    for step in &round.fight_step {
        visit(step, &mut hidden, &mut additional);
    }
    (hidden, additional)
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
                let mode_one = |slot: &SkillEffectSlot| {
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

    #[test]
    fn observed_crits_follow_only_rolled_effects_of_their_own_step_in_order() {
        use sonettobuf::{ActEffect, effect_type_enum::EffectType};

        let effect = |effect_type: EffectType, target_id: i64, config_effect: i32| ActEffect {
            effect_type: Some(effect_type as i32),
            target_id: Some(target_id),
            config_effect: Some(config_effect),
            ..Default::default()
        };
        let nested = FightStep {
            act_id: Some(31090112),
            from_id: Some(20),
            act_effect: vec![effect(EffectType::Healcrit, 10, 0)],
            ..Default::default()
        };
        let round = FightRound {
            fight_step: vec![FightStep {
                act_id: Some(31090111),
                from_id: Some(10),
                act_effect: vec![
                    // Life loss and plain heals are not crit rolls.
                    effect(EffectType::Damage, 10, 30006),
                    effect(EffectType::Crit, -1, -1),
                    effect(EffectType::Additionaldamage, -1, -1),
                    ActEffect {
                        fight_step: Some(nested),
                        ..Default::default()
                    },
                    effect(EffectType::Additionaldamagecrit, -1, -1),
                    effect(EffectType::Heal, 10, 20016),
                    effect(EffectType::Damage, -2, -1),
                    effect(EffectType::Heal, 10, 20001),
                    effect(EffectType::Buffadd, -2, 0),
                ],
                ..Default::default()
            }],
            ..Default::default()
        };

        let (hidden, additional) = crit_choices(&round);

        assert_eq!(
            hidden,
            vec![
                ((31090111, 10), true),
                ((31090112, 20), true),
                ((31090111, 10), false),
                ((31090111, 10), false)
            ]
        );
        assert_eq!(
            additional,
            vec![((31090111, 10, -1), false), ((31090111, 10, -1), true)]
        );
    }
}
