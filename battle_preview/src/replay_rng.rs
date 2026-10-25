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
    seed_action_determinism(runtime, catalog, round);
}

pub fn seed_action_determinism(
    runtime: &mut ReplayBattle,
    catalog: &SkillEffectCatalog,
    round: &FightRound,
) {
    runtime.seed_skill_targets(skill_target_choices(catalog, round));
    runtime.seed_random_skills(random_skill_choices(catalog, round));
    runtime.seed_shell_moxie_choices(shell_moxie_choices(config::configs::get(), round));
    runtime.seed_crystal_reuse_choices(crystal_reuse_choices(catalog, round));
    runtime.seed_moxie_reduction_resistance_choices(moxie_reduction_resistance_choices(
        catalog, round,
    ));
    let (hidden, additional, indirect_heals) = crit_choices(config::configs::get(), round);
    runtime.seed_crits(hidden, additional, indirect_heals);
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
// behaviors 20001/90001, Spirit Shell heals (0), and crit-capable origin damage 30015/60127.
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
                                heal_crit.filter(|_| matches!(config_effect, 20001 | 90001))
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
mod tests {
    use sonettobuf::{BuffInfo, FightEntityInfo, FightHurtInfo, FightTeam};

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
    fn observed_skill_effect_targets_keep_their_first_seen_order() {
        use sonettobuf::{ActEffect, fight_hurt_info::DamageFromType};

        let effect = |target_id, damage_from_type| ActEffect {
            target_id: Some(target_id),
            hurt_info: Some(FightHurtInfo {
                damage_from_type: Some(damage_from_type as i32),
                ..Default::default()
            }),
            ..Default::default()
        };
        let round = FightRound {
            fight_step: vec![FightStep {
                act_id: Some(109380001),
                from_id: Some(-1),
                act_effect: vec![
                    effect(40, DamageFromType::SkillEffect),
                    effect(20, DamageFromType::SkillEffect),
                    effect(30, DamageFromType::SkillEffect),
                    effect(10, DamageFromType::SkillEffect),
                    effect(40, DamageFromType::SkillEffect),
                ],
                ..Default::default()
            }],
            ..Default::default()
        };

        let mut catalog = SkillEffectCatalog::default();
        catalog.insert_logic_target(109380001, 202);
        let choices = skill_target_choices(&catalog, &round);

        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].skill_id, 109380001);
        assert_eq!(choices[0].target_code, 202);
        assert_eq!(choices[0].additional_targets, vec![40, 20, 30, 10]);
    }

    #[test]
    fn moxie_reduction_import_keeps_resisted_and_applied_outcomes() {
        use battle::tooling::scan::{
            ParsedBehavior, ParsedCondition, ParsedConditionKind, ParsedSkillEffect, TargetRequest,
            behavior::classify::BehaviorSpec,
        };
        use sonettobuf::{
            ActEffect, effect_type_enum::EffectType, fight_hurt_info::DamageFromType,
        };

        let mut catalog = SkillEffectCatalog::default();
        let mut slot = SkillEffectSlot::new(
            ParsedBehavior::from_spec(
                BehaviorSpec::new(30001, "DelExPoint"),
                vec![1],
                vec!["1".into()],
            ),
            TargetRequest::self_only(),
        );
        slot.conditions = vec![ParsedCondition {
            opcode: 30208,
            type_name: "AttackCrit".into(),
            kind: ParsedConditionKind::AttackCrit,
            raw_args: Vec::new(),
        }];
        catalog.insert(ParsedSkillEffect {
            skill_id: 302524112,
            slots: vec![slot],
        });
        let action = |result: ActEffect| FightStep {
            act_id: Some(302524112),
            from_id: Some(10),
            act_effect: vec![
                ActEffect {
                    target_id: Some(-1),
                    effect_type: Some(EffectType::Crit as i32),
                    hurt_info: Some(FightHurtInfo {
                        damage_from_type: Some(DamageFromType::Skill as i32),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                result,
            ],
            ..Default::default()
        };
        let round = FightRound {
            fight_step: vec![
                action(ActEffect {
                    target_id: Some(-1),
                    effect_type: Some(EffectType::Resistances as i32),
                    effect_num: Some(409),
                    ..Default::default()
                }),
                action(ActEffect {
                    target_id: Some(-1),
                    effect_type: Some(EffectType::Expointchange as i32),
                    effect_num: Some(-1),
                    ..Default::default()
                }),
            ],
            ..Default::default()
        };

        assert_eq!(
            moxie_reduction_resistance_choices(&catalog, &round),
            vec![
                MoxieReductionResistanceChoice {
                    producer_id: 30001,
                    source_uid: 10,
                    target_uid: -1,
                    resisted: true,
                },
                MoxieReductionResistanceChoice {
                    producer_id: 30001,
                    source_uid: 10,
                    target_uid: -1,
                    resisted: false,
                },
            ]
        );
    }

    #[test]
    fn direct_child_step_imports_the_parent_target_choice() {
        use battle::tooling::scan::{ParsedBehavior, ParsedSkillEffect, TargetRequest};
        use sonettobuf::{ActEffect, fight_hurt_info::DamageFromType};

        let parent_skill_id = 10;
        let child_skill_id = 20;
        let source_uid = 30;
        let target_uid = -2;
        let mut catalog = SkillEffectCatalog::default();
        catalog.insert(ParsedSkillEffect {
            skill_id: parent_skill_id,
            slots: vec![SkillEffectSlot::new(
                ParsedBehavior::from_spec(
                    battle::tooling::scan::behavior::classify::BehaviorSpec::new(
                        50008,
                        "DirectUseSkill",
                    ),
                    vec![child_skill_id],
                    vec![child_skill_id.to_string()],
                ),
                TargetRequest {
                    code: 206,
                    raw: Vec::new(),
                },
            )],
        });
        catalog.insert_logic_target(child_skill_id, 202);
        let hurt = |target_id| ActEffect {
            target_id: Some(target_id),
            hurt_info: Some(FightHurtInfo {
                damage_from_type: Some(DamageFromType::Skill as i32),
                ..Default::default()
            }),
            ..Default::default()
        };
        let round = FightRound {
            fight_step: vec![FightStep {
                act_id: Some(parent_skill_id),
                from_id: Some(source_uid),
                act_effect: vec![ActEffect {
                    fight_step: Some(FightStep {
                        act_id: Some(child_skill_id),
                        from_id: Some(source_uid),
                        to_id: Some(target_uid),
                        act_effect: vec![hurt(target_uid), hurt(-1)],
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };

        let choices = skill_target_choices(&catalog, &round);

        assert_eq!(choices.len(), 2);
        assert_eq!(
            (
                choices[0].skill_id,
                choices[0].source_uid,
                choices[0].target_code,
                choices[0].targets.as_slice(),
            ),
            (parent_skill_id, source_uid, 206, [target_uid].as_slice())
        );
        assert_eq!(choices[1].skill_id, child_skill_id);
        assert_eq!(choices[1].targets, vec![target_uid, -1]);
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
    fn round_rng_import_excludes_only_existing_temporary_cards() {
        let after_actions = card(10, 101, false);
        let temporary = card(10, 999, true);
        let normalized_temporary = card(10, 999, false);
        let round_start = card(10, 102, false);

        assert_eq!(
            captured_round_draws(&FightRound {
                fight_step: vec![FightStep {
                    act_effect: vec![sonettobuf::ActEffect {
                        effect_type: Some(
                            sonettobuf::effect_type_enum::EffectType::Cardspush as i32,
                        ),
                        card_info_list: vec![temporary],
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                team_a_cards2: vec![
                    normalized_temporary.clone(),
                    normalized_temporary.clone(),
                    after_actions.clone(),
                ],
                team_a_cards1: vec![round_start.clone()],
                ..Default::default()
            }),
            vec![normalized_temporary, after_actions, round_start]
        );
    }

    #[test]
    fn round_rng_import_requeues_a_removed_temporary_card() {
        let temporary = card(10, 999, true);
        let persisted = card(10, 999, false);

        assert_eq!(
            captured_round_draws(&FightRound {
                fight_step: vec![FightStep {
                    act_effect: vec![
                        sonettobuf::ActEffect {
                            effect_type: Some(
                                sonettobuf::effect_type_enum::EffectType::Cardspush as i32,
                            ),
                            card_info_list: vec![temporary],
                            ..Default::default()
                        },
                        sonettobuf::ActEffect {
                            effect_type: Some(
                                sonettobuf::effect_type_enum::EffectType::Cardremove as i32,
                            ),
                            reserve_str: Some("1".to_owned()),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }],
                team_a_cards2: vec![persisted.clone()],
                ..Default::default()
            }),
            vec![persisted]
        );
    }

    #[test]
    fn action_rng_import_does_not_queue_the_cloth_replys_ai_snapshot() {
        crate::init_test_config();
        let fight = Fight {
            version: Some(7),
            battle_id: Some(109380001),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100_000),
                    skill_group1: vec![31050111],
                    skill_group2: vec![31050121],
                    attr: Some(sonettobuf::HeroAttribute {
                        hp: Some(100_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(100_000),
                    skill_group1: vec![40340111],
                    skill_group2: vec![40340121],
                    attr: Some(sonettobuf::HeroAttribute {
                        hp: Some(100_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut runtime = ReplayBattle::new(
            battle::catalog::BattleCatalog::new(config::configs::get()),
            fight,
        );
        runtime.start_round().unwrap();
        let stale = card(-1, 40340111, false);
        let expected = card(-1, 40340121, false);

        seed_action_determinism(
            &mut runtime,
            &SkillEffectCatalog::default(),
            &FightRound {
                ai_use_cards: vec![stale],
                ..Default::default()
            },
        );
        seed_round_determinism(
            &mut runtime,
            &SkillEffectCatalog::default(),
            &FightRound {
                ai_use_cards: vec![expected.clone()],
                ..Default::default()
            },
        );

        let round = runtime.advance_round(Default::default()).unwrap();

        assert_eq!(
            round
                .ai_use_cards
                .iter()
                .map(|card| (card.uid, card.skill_id))
                .collect::<Vec<_>>(),
            vec![(expected.uid, expected.skill_id)]
        );
    }

    #[test]
    fn observed_crits_follow_only_rolled_effects_of_their_own_step_in_order() {
        crate::init_test_config();
        use sonettobuf::{ActEffect, FightHurtInfo, effect_type_enum::EffectType};

        let effect = |effect_type: EffectType, target_id: i64, config_effect: i32| ActEffect {
            effect_type: Some(effect_type as i32),
            target_id: Some(target_id),
            config_effect: Some(config_effect),
            ..Default::default()
        };
        let nested = FightStep {
            act_id: Some(999),
            from_id: Some(30),
            act_effect: vec![
                ActEffect {
                    effect_type: Some(EffectType::Buffupdate as i32),
                    buff: Some(BuffInfo {
                        buff_id: Some(31090112),
                        from_uid: Some(20),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                effect(EffectType::Healcrit, 10, 0),
                effect(EffectType::Healcrit, 11, 0),
            ],
            ..Default::default()
        };
        let round = FightRound {
            fight_step: vec![FightStep {
                act_id: Some(31090111),
                from_id: Some(10),
                act_effect: vec![
                    // Life loss and plain heals are not crit rolls.
                    effect(EffectType::Damage, 10, 30006),
                    effect(EffectType::Heal, 10, 0),
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
                    ActEffect {
                        effect_type: Some(EffectType::Eziobigskilldamage as i32),
                        target_id: Some(-1),
                        config_effect: Some(100002),
                        hurt_info: Some(FightHurtInfo {
                            hurt_effect: Some(EffectType::Crit as i32),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    ActEffect {
                        effect_type: Some(EffectType::Eziobigskillorigindamage as i32),
                        target_id: Some(-1),
                        config_effect: Some(100003),
                        hurt_info: Some(FightHurtInfo {
                            hurt_effect: Some(EffectType::Damage as i32),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        };

        let (hidden, additional, indirect_heals) = crit_choices(config::configs::get(), &round);

        assert_eq!(
            hidden,
            vec![
                ((31090111, 10), true),
                ((31090111, 10), false),
                ((31090111, 10), false),
                ((31090111, 10), true),
                ((31090111, 10), false)
            ]
        );
        assert_eq!(
            additional,
            vec![((31090111, 10, -1), false), ((31090111, 10, -1), true)]
        );
        assert_eq!(
            indirect_heals,
            vec![
                ((31090111, 10, 10), false),
                ((31090112, 20, 10), true),
                ((31090112, 20, 11), true),
            ]
        );
    }

    #[test]
    fn shell_moxie_import_preserves_failed_and_successful_deployments() {
        crate::init_test_config();
        use sonettobuf::{ActEffect, effect_type_enum::EffectType};

        let shell = |buff_id, target_id, from_uid| ActEffect {
            effect_type: Some(EffectType::Buffupdate as i32),
            target_id: Some(target_id),
            buff: Some(BuffInfo {
                buff_id: Some(buff_id),
                from_uid: Some(from_uid),
                ..Default::default()
            }),
            ..Default::default()
        };
        let round = FightRound {
            fight_step: vec![FightStep {
                act_effect: vec![
                    // Retrieval updates the deployed shell before the stock and is not a roll.
                    shell(31090112, -1, 20),
                    shell(31090111, 20, -1),
                    // Deployment updates the stock before the deployed shell and rolls once.
                    shell(31090111, 20, -1),
                    shell(31090112, -1, 20),
                    shell(31090111, 20, -1),
                    shell(31090112, -1, 20),
                    ActEffect {
                        effect_type: Some(EffectType::Expointchange as i32),
                        target_id: Some(20),
                        effect_num: Some(1),
                        config_effect: Some(0),
                        ..Default::default()
                    },
                    shell(31090111, 20, -1),
                    shell(31090112, -1, 20),
                ],
                ..Default::default()
            }],
            ..Default::default()
        };

        assert_eq!(
            shell_moxie_choices(config::configs::get(), &round),
            vec![false, true, false]
        );
    }

    #[test]
    fn crystal_reuse_import_preserves_failures_before_a_success() {
        crate::init_test_config();
        let parent = |child: Option<(i32, i64)>| FightStep {
            act_id: Some(31340151),
            from_id: Some(10),
            act_effect: child
                .into_iter()
                .map(|(skill_id, source_uid)| sonettobuf::ActEffect {
                    fight_step: Some(FightStep {
                        act_id: Some(skill_id),
                        from_id: Some(source_uid),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        let round = FightRound {
            fight_step: vec![
                parent(Some((31340152, 30))),
                FightStep {
                    act_id: Some(31340151),
                    from_id: Some(10),
                    act_effect: vec![sonettobuf::ActEffect {
                        fight_step: Some(FightStep {
                            act_id: Some(1),
                            from_id: Some(10),
                            act_effect: vec![sonettobuf::ActEffect {
                                fight_step: Some(FightStep {
                                    act_id: Some(31340152),
                                    from_id: Some(10),
                                    ..Default::default()
                                }),
                                ..Default::default()
                            }],
                            ..Default::default()
                        }),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                parent(None),
                parent(Some((31340152, 10))),
            ],
            ..Default::default()
        };

        assert_eq!(
            crystal_reuse_choices(battle::tooling::scan::effect_catalog::global(), &round),
            vec![
                CrystalReuseChoice {
                    source_uid: 10,
                    skill_id: 31340151,
                    result_skill_id: 31340152,
                    success: false,
                },
                CrystalReuseChoice {
                    source_uid: 10,
                    skill_id: 31340151,
                    result_skill_id: 31340152,
                    success: false,
                },
                CrystalReuseChoice {
                    source_uid: 10,
                    skill_id: 31340151,
                    result_skill_id: 31340152,
                    success: false,
                },
                CrystalReuseChoice {
                    source_uid: 10,
                    skill_id: 31340151,
                    result_skill_id: 31340152,
                    success: true,
                },
            ]
        );
    }
}
