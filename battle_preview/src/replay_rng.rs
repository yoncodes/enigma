use battle::tooling::{
    moxie_down_resistance_attribute_id, opening_hand_size,
    replay::{
        CrystalReuseChoice, HandRankChoice, MoxieReductionResistanceChoice, ReplayBattle,
        RoundDeterminism, SkillTargetChoice,
    },
    scan::{SkillEffectCatalog, SkillEffectSlot},
    ultimate_ignores_limit,
};
use sonettobuf::{CardInfo, Fight, FightRound, FightStep};

mod conduit_ex_point_rolls;
mod opening_buffs;
mod periodic_origin_crit;
mod random_arsenal;
mod random_buff;

/// Imports observed random choices while leaving card construction and validation to the engine.
pub fn opening_determinism(
    game_data: &'static config::GameDB,
    fight: &Fight,
    round: &FightRound,
) -> Result<RoundDeterminism, String> {
    let mut determinism =
        RoundDeterminism::with_seed(fight.battle_id.unwrap_or_default().max(0) as u64);
    let catalog = SkillEffectCatalog::from_fight(game_data, fight);
    determinism.replace_random_buff_pools(
        random_buff::choices(&catalog, round)
            .into_iter()
            .chain(random_arsenal::choices(&catalog, round)),
    );
    determinism
        .enqueue_condition_random_choices(conduit_ex_point_rolls::choices(&catalog, round, true));
    determinism.enqueue_hand_rank_choices(opening_hand_rank_choices(game_data, fight, round));
    determinism.enqueue_hand_rank_choices(card_level_add_choices(round));
    determinism.replace_card_energy_allocations(card_energy_allocations(round));
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
    let limit_fight = opening_buffs::before_deal(fight, round);
    let reserved_ultimate_slots = draws
        .iter()
        .filter(|card| {
            card.uid
                .zip(card.skill_id)
                .is_some_and(|identity| ultimate_identities.contains(&identity))
        })
        .try_fold(0, |count, card| {
            let ignores_limit = ultimate_ignores_limit(
                battle::catalog::BattleCatalog::new(game_data),
                &limit_fight,
                card.uid.unwrap_or_default(),
                card.skill_id.unwrap_or_default(),
            )?;
            Ok::<_, String>(count + usize::from(!ignores_limit))
        })
        .map_err(|error| format!("prepare opening hand for replay setup: {error}"))?;
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
    Ok(determinism)
}

pub fn seed_round_determinism(
    runtime: &mut ReplayBattle,
    catalog: &SkillEffectCatalog,
    round: &FightRound,
) {
    // The deal after actions draws teamACards2; the round-start refill then draws teamACards1.
    runtime.seed_card_draws(captured_round_draws(round));
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
    runtime.seed_condition_random_choices(conduit_ex_point_rolls::choices(catalog, round, false));
    runtime.seed_card_energy_allocations(card_energy_allocations(round));
    seed_action_determinism(runtime, catalog, round);
}

pub fn seed_action_determinism(
    runtime: &mut ReplayBattle,
    catalog: &SkillEffectCatalog,
    round: &FightRound,
) {
    runtime.seed_random_buff_pools(
        random_buff::choices(catalog, round)
            .into_iter()
            .chain(random_arsenal::choices(catalog, round)),
    );
    runtime.seed_hand_rank_choices(card_level_add_choices(round));
    runtime.seed_skill_targets(skill_target_choices(catalog, round));
    runtime.seed_random_skills(random_skill_choices(catalog, round));
    runtime.seed_shell_moxie_choices(shell_moxie_choices(config::configs::get(), round));
    runtime.seed_crystal_reuse_choices(crystal_reuse_choices(catalog, round));
    runtime.seed_moxie_reduction_resistance_choices(moxie_reduction_resistance_choices(
        catalog, round,
    ));
    let (hidden, additional, indirect_heals) = crit_choices(config::configs::get(), round);
    runtime.seed_crits(hidden, additional, indirect_heals);
    runtime
        .seed_periodic_origin_crits(periodic_origin_crit::choices(config::configs::get(), round));
}

fn card_energy_allocations(round: &FightRound) -> Vec<Vec<CardInfo>> {
    fn visit(step: &FightStep, hands: &mut Vec<Vec<CardInfo>>) {
        for effect in &step.act_effect {
            if effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Allocatecardenergy as i32)
                && effect.effect_num1 == Some(1)
            {
                hands.push(effect.card_info_list.clone());
            }
            if let Some(nested) = effect.fight_step.as_ref() {
                visit(nested, hands);
            }
        }
    }

    let mut hands = Vec::new();
    for step in round.fight_step.iter().chain(&round.next_round_begin_step) {
        visit(step, &mut hands);
    }
    hands
}

fn card_level_add_choices(round: &FightRound) -> Vec<HandRankChoice> {
    use sonettobuf::effect_type_enum::EffectType;
    const CARD_LEVEL_ADD: i32 = 701;

    fn visit(step: &FightStep, choices: &mut Vec<HandRankChoice>) {
        for pair in step.act_effect.windows(2) {
            let [added, changed] = pair else { continue };
            if added.effect_type == Some(EffectType::Cardleveladd as i32)
                && added.buff_act_id == Some(CARD_LEVEL_ADD)
                && changed.effect_type == Some(EffectType::Cardlevelchange as i32)
                && let Some(owner_uid) = changed.entity.as_ref().and_then(|entity| entity.uid)
                && let Some(hand_index) = changed
                    .target_id
                    .and_then(|target| target.checked_sub(1))
                    .and_then(|index| usize::try_from(index).ok())
            {
                choices.push(HandRankChoice {
                    opcode: CARD_LEVEL_ADD,
                    owner_uid,
                    hand_index,
                });
            }
        }
        for nested in step
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
        {
            visit(nested, choices);
        }
    }

    let mut choices = Vec::new();
    for step in &round.fight_step {
        visit(step, &mut choices);
    }
    choices
}

fn captured_round_draws(round: &FightRound) -> Vec<CardInfo> {
    fn visit(step: &FightStep, hand: &mut Option<Vec<CardInfo>>) {
        for effect in &step.act_effect {
            if effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Cardspush as i32)
            {
                *hand = Some(effect.card_info_list.clone());
            }
            if effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Cardremove as i32)
                && let Some(cards) = hand.as_mut()
            {
                let mut indices = effect
                    .reserve_str
                    .as_deref()
                    .unwrap_or_default()
                    .split('#')
                    .filter_map(|index| index.parse::<usize>().ok()?.checked_sub(1))
                    .collect::<Vec<_>>();
                indices.sort_unstable();
                indices.dedup();
                for index in indices.into_iter().rev() {
                    if index < cards.len() {
                        cards.remove(index);
                    }
                }
            }
            if let Some(nested) = effect.fight_step.as_ref() {
                visit(nested, hand);
            }
        }
    }

    let mut before_refill = None;
    for step in &round.fight_step {
        visit(step, &mut before_refill);
    }
    let mut existing_temporary = before_refill
        .unwrap_or_default()
        .into_iter()
        .filter(|card| card.temp_card.unwrap_or_default())
        .collect::<Vec<_>>();
    let after_actions = round.team_a_cards2.iter().filter(|card| {
        let Some(index) = existing_temporary
            .iter()
            .position(|existing| existing.uid == card.uid && existing.skill_id == card.skill_id)
        else {
            return !card.temp_card.unwrap_or_default();
        };
        existing_temporary.remove(index);
        false
    });

    after_actions
        .chain(
            round
                .team_a_cards1
                .iter()
                .filter(|card| !card.temp_card.unwrap_or_default()),
        )
        .cloned()
        .collect()
}

fn moxie_reduction_resistance_choices(
    catalog: &SkillEffectCatalog,
    round: &FightRound,
) -> Vec<MoxieReductionResistanceChoice> {
    use sonettobuf::{effect_type_enum::EffectType, fight_hurt_info::DamageFromType};

    fn visit(
        catalog: &SkillEffectCatalog,
        step: &FightStep,
        choices: &mut Vec<MoxieReductionResistanceChoice>,
    ) {
        let actor = step
            .act_id
            .and_then(|skill_id| {
                catalog.get(skill_id).and_then(|effect| {
                    effect.slots.iter().find_map(|slot| {
                        (slot.behavior.spec.key.opcode == 30001
                            && slot.behavior.spec.key.type_name == "DelExPoint"
                            && slot.conditions.iter().any(|condition| {
                                condition.opcode == 30208 && condition.type_name == "AttackCrit"
                            }))
                        .then_some(slot.behavior.spec.key.opcode)
                    })
                })
            })
            .zip(step.from_id.filter(|source_uid| *source_uid != 0));
        if let Some((producer_id, source_uid)) = actor {
            let crit_targets = step.act_effect.iter().filter_map(|effect| {
                (effect.effect_type == Some(EffectType::Crit as i32)
                    && effect.hurt_info.as_ref().is_some_and(|hurt| {
                        hurt.damage_from_type == Some(DamageFromType::Skill as i32)
                    }))
                .then_some(effect.target_id)
                .flatten()
            });
            for target_uid in crit_targets {
                let resisted = step.act_effect.iter().any(|effect| {
                    effect.target_id == Some(target_uid)
                        && effect.effect_type == Some(EffectType::Resistances as i32)
                        && effect.effect_num == Some(moxie_down_resistance_attribute_id())
                });
                let reduced = step.act_effect.iter().any(|effect| {
                    effect.target_id == Some(target_uid)
                        && effect.effect_type == Some(EffectType::Expointchange as i32)
                        && effect.effect_num.is_some_and(|amount| amount < 0)
                });
                if resisted || reduced {
                    choices.push(MoxieReductionResistanceChoice {
                        producer_id,
                        source_uid,
                        target_uid,
                        resisted,
                    });
                }
            }
        }
        for child in step
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
        {
            visit(catalog, child, choices);
        }
    }

    let mut choices = Vec::new();
    for step in &round.fight_step {
        visit(catalog, step, &mut choices);
    }
    choices
}

fn skill_target_choices(
    catalog: &SkillEffectCatalog,
    round: &FightRound,
) -> Vec<SkillTargetChoice> {
    use sonettobuf::{effect_type_enum::EffectType, fight_hurt_info::DamageFromType};

    fn push_unique(targets: &mut Vec<i64>, target_uid: i64) {
        if target_uid != 0 && !targets.contains(&target_uid) {
            targets.push(target_uid);
        }
    }

    fn visit(catalog: &SkillEffectCatalog, step: &FightStep, choices: &mut Vec<SkillTargetChoice>) {
        let actor = step
            .act_id
            .filter(|skill_id| *skill_id > 0)
            .zip(step.from_id.filter(|source_uid| *source_uid != 0));
        if let Some((skill_id, source_uid)) = actor {
            let mut choice = SkillTargetChoice {
                skill_id,
                source_uid,
                target_code: catalog.logic_target(skill_id),
                targets: Vec::new(),
                additional_targets: Vec::new(),
                crit_targets: Vec::new(),
                additional_crit_targets: Vec::new(),
            };
            for effect in &step.act_effect {
                let Some(hurt) = effect.hurt_info.as_ref() else {
                    continue;
                };
                let target_uid = effect.target_id.unwrap_or_default();
                match hurt.damage_from_type {
                    Some(kind) if kind == DamageFromType::Skill as i32 => {
                        push_unique(&mut choice.targets, target_uid);
                        if effect.effect_type == Some(EffectType::Crit as i32) {
                            push_unique(&mut choice.crit_targets, target_uid);
                        }
                    }
                    Some(kind) if kind == DamageFromType::SkillEffect as i32 => {
                        push_unique(&mut choice.additional_targets, target_uid);
                        if effect.effect_type == Some(EffectType::Origincrit as i32) {
                            push_unique(&mut choice.additional_crit_targets, target_uid);
                        }
                    }
                    _ => {}
                }
            }
            if !choice.targets.is_empty() || !choice.additional_targets.is_empty() {
                choices.push(choice);
            }
            for child in step
                .act_effect
                .iter()
                .filter_map(|effect| effect.fight_step.as_ref())
            {
                let Some((child_skill_id, target_uid)) = child
                    .act_id
                    .filter(|child_skill_id| *child_skill_id > 0)
                    .zip(child.to_id.filter(|target_uid| *target_uid != 0))
                else {
                    continue;
                };
                let Some(target_code) =
                    catalog.direct_skill_target_code_for_child(skill_id, child_skill_id)
                else {
                    continue;
                };
                choices.push(SkillTargetChoice {
                    skill_id,
                    source_uid,
                    target_code,
                    targets: vec![target_uid],
                    additional_targets: Vec::new(),
                    crit_targets: Vec::new(),
                    additional_crit_targets: Vec::new(),
                });
            }
        }
        for child in step
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
        {
            visit(catalog, child, choices);
        }
    }

    let mut choices = Vec::new();
    for step in &round.fight_step {
        visit(catalog, step, &mut choices);
    }
    choices
}

fn shell_moxie_choices(game_data: &'static config::GameDB, round: &FightRound) -> Vec<bool> {
    use sonettobuf::effect_type_enum::EffectType;

    fn visit(game_data: &'static config::GameDB, step: &FightStep, choices: &mut Vec<bool>) {
        let mut pending: Option<(usize, i64, i32)> = None;
        let mut previous_shell: Option<(i32, i64)> = None;
        for effect in &step.act_effect {
            let current_shell = effect.buff.as_ref().and_then(|buff| {
                buff.buff_id
                    .zip(effect.target_id.filter(|uid| *uid != 0))
                    .zip(buff.from_uid.filter(|uid| *uid != 0))
                    .filter(|((buff_id, _), _)| {
                        battle::tooling::shell_process_spec(game_data, *buff_id).is_some()
                    })
            });
            if let Some(((deployed_buff_id, _), source_uid)) = current_shell
                && let Some((stock_buff_id, stock_owner_uid)) = previous_shell
                && stock_owner_uid == source_uid
                && let Some((expected_deployed_buff_id, moxie_delta)) =
                    battle::tooling::shell_process_spec(game_data, stock_buff_id)
                && deployed_buff_id == expected_deployed_buff_id
                && stock_buff_id != deployed_buff_id
                && moxie_delta != 0
            {
                choices.push(false);
                pending = Some((choices.len() - 1, source_uid, moxie_delta));
            }
            if effect.effect_type == Some(EffectType::Expointchange as i32)
                && effect.config_effect.unwrap_or_default() == 0
                && let Some((index, source_uid, moxie_delta)) = pending
                && effect.target_id == Some(source_uid)
                && effect.effect_num == Some(moxie_delta)
            {
                choices[index] = true;
                pending = None;
            }
            if let Some(child) = effect.fight_step.as_ref() {
                visit(game_data, child, choices);
            }
            previous_shell = current_shell.map(|((buff_id, owner_uid), _)| (buff_id, owner_uid));
        }
    }

    let mut choices = Vec::new();
    for step in &round.fight_step {
        visit(game_data, step, &mut choices);
    }
    choices
}

type HiddenCrit = ((i32, i64), bool);
type AdditionalCrit = ((i32, i64, i64), bool);
type IndirectHealCrit = ((i32, i64, i64), bool);

// Each effect the engine rolls a crit for is rolled under its own step's skill (or buff) and
// source, in step order. Config effects name the rolling source: skill row damage (-1), healing
// behaviors 20001/90001/60179, Spirit Shell heals (0), and crit-capable origin damage 30015/60127.
fn crit_choices(
    game_data: &'static config::GameDB,
    round: &FightRound,
) -> (Vec<HiddenCrit>, Vec<AdditionalCrit>, Vec<IndirectHealCrit>) {
    use sonettobuf::effect_type_enum::EffectType;

    fn visit(
        game_data: &'static config::GameDB,
        step: &FightStep,
        hidden: &mut Vec<HiddenCrit>,
        additional: &mut Vec<AdditionalCrit>,
        indirect_heals: &mut Vec<IndirectHealCrit>,
    ) {
        let key = step
            .act_id
            .filter(|act_id| *act_id > 0)
            .zip(step.from_id.filter(|from_id| *from_id != 0));
        let mut shell_heal_source = None;
        let mut shell_healing_started = false;
        for effect in &step.act_effect {
            let heal_crit = [
                (EffectType::Heal as i32, false),
                (EffectType::Healcrit as i32, true),
            ]
            .into_iter()
            .find_map(|(kind, is_crit)| (effect.effect_type == Some(kind)).then_some(is_crit));
            if shell_healing_started && heal_crit.is_none() {
                shell_heal_source = None;
                shell_healing_started = false;
            }
            if let Some(buff) = effect.buff.as_ref()
                && let Some(buff_id) = buff.buff_id
                && battle::tooling::shell_process_spec(game_data, buff_id)
                    .is_some_and(|(deployed_buff_id, _)| deployed_buff_id == buff_id)
                && let Some(source_uid) = buff.from_uid.filter(|uid| *uid != 0)
            {
                // Shell healing is projected under the attack that caused retrieval. The
                // preceding deployed-buff update retains the semantic producer identity.
                shell_heal_source = Some((buff_id, source_uid));
            }
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
                    if config_effect == 0
                        && let Some(is_crit) = heal_crit
                    {
                        let (producer_id, source_uid) = match shell_heal_source {
                            Some(source) => {
                                shell_healing_started = true;
                                source
                            }
                            None => (act_id, from_id),
                        };
                        indirect_heals.push((
                            (
                                producer_id,
                                source_uid,
                                effect.target_id.unwrap_or_default(),
                            ),
                            is_crit,
                        ));
                    } else {
                        let rolled = crit(EffectType::Damage, EffectType::Crit)
                            .filter(|_| config_effect == -1)
                            .or_else(|| {
                                matches!(
                                    effect_type,
                                    value
                                        if value == EffectType::Eziobigskilldamage as i32
                                            || value
                                                == EffectType::Eziobigskillorigindamage as i32
                                )
                                .then(|| {
                                    effect.hurt_info.as_ref().is_some_and(|hurt| {
                                        hurt.hurt_effect == Some(EffectType::Crit as i32)
                                    })
                                })
                            })
                            .or_else(|| {
                                heal_crit.filter(|_| matches!(config_effect, 20001 | 60179 | 90001))
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
            }
            if let Some(child) = effect.fight_step.as_ref() {
                visit(game_data, child, hidden, additional, indirect_heals);
            }
        }
    }

    let mut hidden = Vec::new();
    let mut additional = Vec::new();
    let mut indirect_heals = Vec::new();
    for step in &round.fight_step {
        visit(
            game_data,
            step,
            &mut hidden,
            &mut additional,
            &mut indirect_heals,
        );
    }
    (hidden, additional, indirect_heals)
}

fn random_skill_choices(catalog: &SkillEffectCatalog, round: &FightRound) -> Vec<i32> {
    fn visit(catalog: &SkillEffectCatalog, step: &FightStep, choices: &mut Vec<i32>) {
        let references = catalog.random_skill_choice_references(step.act_id.unwrap_or_default());
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

fn crystal_reuse_choices(
    catalog: &SkillEffectCatalog,
    round: &FightRound,
) -> Vec<CrystalReuseChoice> {
    fn visit(
        catalog: &SkillEffectCatalog,
        step: &FightStep,
        choices: &mut Vec<CrystalReuseChoice>,
    ) {
        if let Some((skill_id, source_uid)) = step.act_id.zip(step.from_id) {
            let child_skills = step
                .act_effect
                .iter()
                .filter_map(|effect| {
                    let child = effect.fight_step.as_ref()?;
                    Some((child.act_id?, child.from_id?))
                })
                .collect::<Vec<_>>();
            choices.extend(catalog.crystal_reuse_references(skill_id).into_iter().map(
                |result_skill_id| CrystalReuseChoice {
                    source_uid,
                    skill_id,
                    result_skill_id,
                    success: child_skills.contains(&(result_skill_id, source_uid)),
                },
            ));
        }
        for child in step
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
        {
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
#[path = "replay_rng/tests.rs"]
mod tests;
