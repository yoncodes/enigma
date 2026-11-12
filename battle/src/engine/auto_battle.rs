use std::{cmp::Reverse, collections::HashSet};

use sonettobuf::{
    AutoRoundReply, AutoRoundRequest, BeginRoundOper, CardInfo, Fight, FightDeviceOper,
};

use crate::engine::{
    manager::{
        BattleManagers,
        card::{
            CARD_PLAY_ORIGIN, CardCommand, CardManager, CardOpType, CardPlay, CardUseUniversal,
        },
        eureka::PowerType,
    },
    round::{command::RoundCommand, state::RoundState},
    runtime::{determinism::RoundDeterminism, schedule::card_skill_is_blocked},
    skill::{
        effect::SkillEffectCatalog,
        target::{CardSelectionResolver, TargetContext, TargetPool, TargetRequest},
    },
};

mod conduit;

#[derive(Debug)]
pub(crate) enum AutoBattleError {
    InvalidChoice,
    MissingSkill(i32),
    ConduitEnergy(conduit::CardEnergyError),
}

const HEAL_BELOW_HP_PERCENT: i64 = 80;
const URGENT_HEAL_BELOW_HP_PERCENT: i64 = 60;

#[derive(Debug, Clone)]
struct Candidate {
    card_index: usize,
    source_uid: i64,
    lineup_position: i32,
    skill_id: i32,
    target_uid: i64,
    normal_ap: i32,
    chosen_skill_id: Option<i32>,
    free: bool,
    ultimate: bool,
    temporary: bool,
    energy: bool,
    preferred_energy: bool,
    energy_amount: i32,
    flexible_energy: bool,
    special_energy: bool,
    first_for_owner: bool,
    support: Support,
    damage_rate: i32,
    rank: i32,
}

struct CardPlan {
    opers: Vec<BeginRoundOper>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Support {
    Unneeded,
    None,
    Needed,
    Urgent,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn plan(
    request: &AutoRoundRequest,
    fight: &Fight,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    round_state: &RoundState,
    determinism: &RoundDeterminism,
    devices_opers: Vec<FightDeviceOper>,
) -> Result<AutoRoundReply, AutoBattleError> {
    // Every card resolves before any skill runs, so indices, AP and readiness use round-start state.
    let pool = TargetPool::from_fight_with_catalog(managers.catalog(), fight);
    let preferred_target = request
        .to_id
        .filter(|uid| pool.entity(*uid).is_some())
        .or_else(|| {
            let model_id = managers
                .catalog()
                .battle_win_target_model(fight.battle_id?)?;
            pool.entities()
                .find(|entity| entity.model_id == model_id && pool.team_type(entity.uid) == Some(2))
                .map(|entity| entity.uid)
        });
    let mut missing_roots: Vec<i32> = managers
        .card
        .hand()
        .iter()
        .chain(managers.card.team_cards())
        .filter_map(|card| card.skill_id)
        .filter(|skill_id| *skill_id > 0)
        .chain(
            managers
                .card
                .hand()
                .iter()
                .chain(managers.card.team_cards())
                .flat_map(|card| card_choice_options(card, managers)),
        )
        .collect::<HashSet<_>>()
        .into_iter()
        .filter(|skill_id| catalog.get(*skill_id).is_none())
        .collect();
    missing_roots.sort_unstable();
    if let Some(skill_id) = missing_roots.first().copied() {
        return Err(AutoBattleError::MissingSkill(skill_id));
    }
    let context = crate::engine::skill::target::TargetContext {
        battle_id: fight.battle_id.unwrap_or_default(),
        current_round: round_state.cur_round,
        ..Default::default()
    };
    let mut devices = devices_opers;
    conduit::select_groups(managers, &pool, catalog, determinism, context, &mut devices);
    let card_plan = plan_cards(
        request,
        fight,
        managers,
        catalog,
        round_state,
        determinism,
        &pool,
        preferred_target,
        &devices,
    )?
    .unwrap_or(CardPlan { opers: Vec::new() });
    Ok(reply(request, card_plan.opers, devices))
}

#[allow(clippy::too_many_arguments)]
fn plan_cards(
    request: &AutoRoundRequest,
    fight: &Fight,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    round_state: &RoundState,
    determinism: &RoundDeterminism,
    pool: &TargetPool,
    preferred_target: Option<i64>,
    planned_devices: &[FightDeviceOper],
) -> Result<Option<CardPlan>, AutoBattleError> {
    plan_cards_inner(
        request,
        fight,
        managers,
        catalog,
        round_state,
        determinism,
        pool,
        preferred_target,
        planned_devices,
    )
}

#[allow(clippy::too_many_arguments)]
fn plan_cards_inner(
    request: &AutoRoundRequest,
    fight: &Fight,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    round_state: &RoundState,
    determinism: &RoundDeterminism,
    pool: &TargetPool,
    preferred_target: Option<i64>,
    planned_devices: &[FightDeviceOper],
) -> Result<Option<CardPlan>, AutoBattleError> {
    let mut cards = managers.card.clone();
    let mut choice_determinism = determinism.clone();
    let mut normal_ap = round_state.act_point.max(0);
    let mut played_owners = HashSet::new();
    if !apply_prefix(
        &mut cards,
        &mut normal_ap,
        &mut played_owners,
        &request.opers,
        managers,
        catalog,
        pool,
        determinism,
    )? {
        return Ok(None);
    }
    let live_pool = pool.runtime_view(managers);
    let selected_power_types = conduit::selected_power_types(managers, planned_devices);
    let mut opers = assist_boss_operation(
        fight,
        managers,
        &live_pool,
        catalog,
        determinism,
        preferred_target,
        &request.opers,
    )
    .into_iter()
    .collect::<Vec<_>>();
    let mut reported_unsupported = HashSet::new();
    loop {
        let choices = cards
            .hand()
            .iter()
            .chain(cards.team_cards())
            .map(|card| {
                cards
                    .can_use_card(card)
                    .then(|| random_card_choice(card, managers, &mut choice_determinism))
                    .flatten()
            })
            .collect::<Vec<_>>();
        let Some(candidate) = best_candidate(
            &cards,
            &choices,
            &played_owners,
            normal_ap,
            preferred_target,
            managers,
            managers,
            &live_pool,
            &live_pool,
            catalog,
            determinism,
            &selected_power_types,
            &mut reported_unsupported,
        )?
        else {
            break;
        };
        let play = CardPlay {
            origin: CARD_PLAY_ORIGIN,
            hand_index: candidate.card_index,
            target_uid: Some(candidate.target_uid),
            chosen_skill_id: candidate.chosen_skill_id,
            choice: None,
            recorded_skill: None,
        };
        let operation = play_operation(
            candidate.card_index,
            candidate.target_uid,
            candidate.chosen_skill_id,
        );
        if !play_card(&mut cards, play) {
            break;
        }
        normal_ap = normal_ap.saturating_sub(candidate.normal_ap);
        if candidate.normal_ap > 0 {
            played_owners.insert(candidate.source_uid);
        }
        opers.push(operation);
    }
    Ok(Some(CardPlan { opers }))
}

fn card_choice_options(card: &CardInfo, managers: &BattleManagers) -> Vec<i32> {
    card.skill_id
        .map(|skill_id| managers.catalog().card_choice_skills(skill_id))
        .unwrap_or_default()
}

// Cards with a `fight_card_choice` row get one of their options at random.
fn random_card_choice(
    card: &CardInfo,
    managers: &BattleManagers,
    determinism: &mut RoundDeterminism,
) -> Option<i32> {
    let options = card_choice_options(card, managers);
    options
        .get(determinism.card_random_index(options.len())?)
        .copied()
}
fn reply(
    request: &AutoRoundRequest,
    opers: Vec<BeginRoundOper>,
    devices_opers: Vec<FightDeviceOper>,
) -> AutoRoundReply {
    AutoRoundReply {
        opers,
        to_id: request.to_id,
        cloth_skill: Some(sonettobuf::ClothSkillInfo {
            skill_id: Some(0),
            from_id: Some(0),
            to_id: Some(0),
            r#type: Some(0),
        }),
        devices_opers,
    }
}

#[allow(clippy::too_many_arguments)]
fn assist_boss_operation(
    fight: &Fight,
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
    preferred_target: Option<i64>,
    prefix: &[BeginRoundOper],
) -> Option<BeginRoundOper> {
    let team = fight.attacker.as_ref()?;
    let source_uid = team.assist_boss.as_ref()?.uid?;
    let info = team.assist_boss_info.as_ref()?;
    if info.curr_cd.unwrap_or_default() > 0 {
        return None;
    }
    let used = prefix
        .iter()
        .filter_map(RoundCommand::from_oper)
        .filter(|command| matches!(command, RoundCommand::UseAssistBoss { .. }))
        .count() as i32;
    if info
        .round_use_limit
        .is_some_and(|limit| limit > 0 && used >= limit)
    {
        return None;
    }
    let power = managers
        .eureka
        .get(source_uid, PowerType::AssistBoss.id())
        .current;
    info.skills.iter().find_map(|skill| {
        let skill_id = skill.skill_id.filter(|skill_id| *skill_id > 0)?;
        let cost = skill.need_power.unwrap_or_default().max(0);
        if power < cost
            || skill.power_low.is_some_and(|low| power < low)
            || skill.power_high.is_some_and(|high| power > high)
            || catalog.get(skill_id).is_none()
            || !catalog.issues(skill_id).is_empty()
            || card_skill_is_blocked(managers, catalog, source_uid, skill_id)
        {
            return None;
        }
        let targets = target_options(
            source_uid,
            skill_id,
            preferred_target,
            managers,
            pool,
            catalog,
            determinism,
        );
        let buffs = certain_buffs(skill_id, catalog);
        let target_uid = choose_target(
            targets,
            skill_id,
            preferred_target,
            &buffs,
            managers,
            pool,
            catalog,
        )?;
        Some(BeginRoundOper {
            oper_type: Some(CardOpType::AssistBoss.id()),
            param1: Some(skill_id),
            to_id: Some(target_uid),
            ..Default::default()
        })
    })
}

fn play_operation(
    card_index: usize,
    target_uid: i64,
    chosen_skill_id: Option<i32>,
) -> BeginRoundOper {
    BeginRoundOper {
        oper_type: Some(CardOpType::PlayCard.id()),
        param1: Some(card_index as i32 + 1),
        to_id: Some(target_uid),
        param3: chosen_skill_id,
        ..Default::default()
    }
}

fn play_card(cards: &mut CardManager, play: CardPlay) -> bool {
    cards.execute_command(CardCommand::Play(play)).is_ok()
        && cards
            .execute_command(CardCommand::ComposeAdjacent {
                origin: CARD_PLAY_ORIGIN,
            })
            .is_ok()
}

#[allow(clippy::too_many_arguments)]
fn apply_prefix(
    cards: &mut CardManager,
    normal_ap: &mut i32,
    played_owners: &mut HashSet<i64>,
    opers: &[BeginRoundOper],
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    pool: &TargetPool,
    determinism: &RoundDeterminism,
) -> Result<bool, AutoBattleError> {
    for oper in opers {
        let Some(command) = RoundCommand::from_oper(oper) else {
            return Ok(false);
        };
        if matches!(command, RoundCommand::UseAssistBoss { .. }) {
            continue;
        }
        let mut played = None;
        let (card_command, ap_cost) = match command {
            RoundCommand::MoveCard {
                from_index,
                to_index,
            } => (
                CardCommand::Move {
                    origin: CARD_PLAY_ORIGIN,
                    from_index,
                    to_index,
                },
                1,
            ),
            RoundCommand::UseUniversal {
                universal_index,
                target_index,
            } => (
                CardCommand::UseUniversal(CardUseUniversal {
                    origin: CARD_PLAY_ORIGIN,
                    universal_index,
                    target_index,
                }),
                0,
            ),
            RoundCommand::DissolveCard { card_index } => (
                CardCommand::Dissolve {
                    origin: CARD_PLAY_ORIGIN,
                    card_index,
                },
                0,
            ),
            RoundCommand::UseAssistBoss { .. } => unreachable!("handled before card commands"),
            RoundCommand::PlayCard {
                card_index,
                target_uid,
                chosen_skill_id,
                recorded_skill,
            } => {
                let Some(card) = cards.visible_card(card_index) else {
                    return Ok(false);
                };
                let Some((source_uid, skill_id)) = card_identity(card, chosen_skill_id) else {
                    return Ok(false);
                };
                if catalog.get(skill_id).is_none() {
                    let configured = chosen_skill_id.is_none()
                        || card.skill_id == Some(skill_id)
                        || card_choice_options(card, managers).contains(&skill_id);
                    return Err(if configured {
                        AutoBattleError::MissingSkill(skill_id)
                    } else {
                        AutoBattleError::InvalidChoice
                    });
                }
                let legal_target = legal_target(
                    source_uid,
                    skill_id,
                    target_uid,
                    managers,
                    &pool.runtime_view(managers),
                    catalog,
                    determinism,
                );
                if card_skill_is_blocked(managers, catalog, source_uid, skill_id) || !legal_target {
                    return Ok(false);
                }
                let ap_cost = action_point_cost(card, source_uid, skill_id, managers, catalog);
                let play = CardPlay {
                    origin: CARD_PLAY_ORIGIN,
                    hand_index: card_index,
                    target_uid,
                    chosen_skill_id,
                    choice: None,
                    recorded_skill,
                };
                played = Some(source_uid);
                (CardCommand::Play(play), ap_cost)
            }
        };
        if ap_cost > *normal_ap || cards.execute_command(card_command).is_err() {
            return Ok(false);
        }
        *normal_ap = normal_ap.saturating_sub(ap_cost);
        if cards
            .execute_command(CardCommand::ComposeAdjacent {
                origin: CARD_PLAY_ORIGIN,
            })
            .is_err()
        {
            return Ok(false);
        }
        if let Some(source_uid) = played
            && ap_cost > 0
        {
            played_owners.insert(source_uid);
        }
    }
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
fn best_candidate(
    cards: &CardManager,
    choices: &[Option<i32>],
    played_owners: &HashSet<i64>,
    normal_ap: i32,
    preferred_target: Option<i64>,
    managers: &BattleManagers,
    sim: &BattleManagers,
    live_pool: &TargetPool,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
    selected_power_types: &HashSet<(i32, i32)>,
    reported_unsupported: &mut HashSet<i32>,
) -> Result<Option<Candidate>, AutoBattleError> {
    let candidates = cards
        .hand()
        .iter()
        .enumerate()
        .filter_map(|(card_index, card)| {
            if !cards.can_use_card(card) {
                return None;
            }
            let chosen_skill_id = choices.get(card_index).copied().flatten();
            let (source_uid, skill_id) = card_identity(card, chosen_skill_id)?;
            let normal_ap_cost = action_point_cost(card, source_uid, skill_id, managers, catalog);
            if normal_ap_cost > normal_ap {
                return None;
            }
            let issues = catalog.issues(skill_id);
            if catalog.get(skill_id).is_none() || !issues.is_empty() {
                if reported_unsupported.insert(skill_id) {
                    tracing::warn!(skill_id, ?issues, "auto-battle skipped unsupported skill");
                }
                return None;
            }
            if card_skill_is_blocked(managers, catalog, source_uid, skill_id) {
                return None;
            }
            let source = live_pool.entity(source_uid)?;
            let temporary = card.temp_card.unwrap_or_default();
            let device = crate::engine::mechanic::card::CardMechanic.is_device_card(managers, card);
            // Stage-rule temporary cards are left for the player.
            if temporary
                && card.hero_id != Some(source.model_id)
                && managers.catalog().skill_hero_id(skill_id) != Some(source.model_id)
                && !device
            {
                return None;
            }
            let ultimate = crate::engine::mechanic::card::CardMechanic
                .is_ultimate_skill(managers, skill_id, source);
            if ultimate
                && !crate::engine::mechanic::card::CardMechanic.ultimate_ready(managers, source)
            {
                return None;
            }
            let targets = target_options(
                source_uid,
                skill_id,
                preferred_target,
                sim,
                pool,
                catalog,
                determinism,
            );
            let buffs = certain_buffs(skill_id, catalog);
            let effect_tag = catalog.effect_tag(skill_id);
            let energy = match conduit::card_energy(
                cards,
                managers,
                live_pool,
                catalog,
                source_uid,
                skill_id,
                temporary,
                selected_power_types,
            ) {
                Ok(energy) => energy,
                Err(error) => return Some(Err(AutoBattleError::ConduitEnergy(error))),
            };
            if energy.is_some_and(|energy| !energy.selectable()) {
                return None;
            }
            let energy_amount = energy.map_or(0, |energy| energy.amount);
            let special_energy = energy.is_some_and(|energy| energy.special);
            let preferred_energy = energy.is_some_and(|energy| energy.preferred);
            // An enemy's debuff may come from a passive, so debuff cards never count as redundant.
            let may_be_redundant = !catalog.is_attack(skill_id)
                && effect_tag
                    != crate::engine::skill::effect::catalog::SkillEffectTag::Debuff as i32;
            let setup = effect_tag
                == crate::engine::skill::effect::catalog::SkillEffectTag::CounterSpell as i32
                || catalog.get(skill_id).is_some_and(|effect| {
                    effect.slots.iter().any(|slot| {
                        slot.behavior.spec.kind
                            == crate::engine::skill::behavior::classify::BehaviorKind::AverageLife
                    })
                });
            let support = support_need(
                effect_tag == crate::engine::skill::effect::catalog::SkillEffectTag::Heal as i32,
                setup,
                if may_be_redundant { &buffs } else { &[] },
                &targets,
                sim,
                pool,
            );
            let live_preferred_target = catalog
                .is_attack(skill_id)
                .then(|| {
                    target_options(
                        source_uid,
                        skill_id,
                        preferred_target,
                        managers,
                        live_pool,
                        catalog,
                        determinism,
                    )
                })
                .and_then(|targets| preferred_target.filter(|uid| targets.contains(uid)));
            let resolved_target_uid = live_preferred_target.or_else(|| {
                choose_target(
                    targets,
                    skill_id,
                    preferred_target,
                    &buffs,
                    sim,
                    pool,
                    catalog,
                )
            })?;
            let target_uid = energy
                .is_some()
                .then(|| {
                    card_selection_target_options(
                        source_uid,
                        skill_id,
                        preferred_target,
                        managers,
                        live_pool,
                        catalog,
                        determinism,
                    )
                    .into_iter()
                    .min_by_key(|uid| {
                        let target = pool.entity(*uid);
                        (
                            preferred_target != Some(*uid),
                            target.map(|target| target.current_hp).unwrap_or(i32::MAX),
                            target.map(|target| target.position).unwrap_or(i32::MAX),
                            *uid,
                        )
                    })
                })
                .flatten()
                .unwrap_or(resolved_target_uid);
            Some(Ok(Candidate {
                card_index,
                source_uid,
                lineup_position: source.position,
                skill_id,
                target_uid,
                normal_ap: normal_ap_cost,
                chosen_skill_id,
                free: normal_ap_cost == 0,
                ultimate,
                temporary,
                first_for_owner: energy.is_none()
                    && !played_owners.contains(&source_uid)
                    && support != Support::Unneeded,
                energy: device,
                preferred_energy,
                energy_amount,
                flexible_energy: energy.is_some_and(|energy| energy.flexible),
                special_energy,
                support,
                damage_rate: catalog.damage_rate(skill_id),
                rank: managers.catalog().card_skill_rank(card),
            }))
        })
        .collect::<Result<Vec<Candidate>, AutoBattleError>>()?;
    Ok(candidates.into_iter().max_by_key(|candidate| {
        // Official auto casts ready ultimates left to right.
        let strength = (!candidate.ultimate && !candidate.energy)
            .then_some((candidate.damage_rate, candidate.rank));
        (
            candidate.free,
            candidate.ultimate,
            candidate.support == Support::Urgent,
            (
                candidate.preferred_energy,
                candidate.energy,
                candidate.temporary && candidate.special_energy,
                candidate.energy_amount,
                candidate.special_energy,
                candidate.flexible_energy && !candidate.special_energy,
                (candidate.energy && !candidate.special_energy)
                    .then_some(candidate.lineup_position),
            ),
            // Give every hero a card before any hero plays a second one when utility is equal.
            candidate.first_for_owner,
            candidate.support,
            strength,
            Reverse(candidate.lineup_position),
            Reverse(candidate.card_index),
            Reverse(candidate.source_uid),
            Reverse(candidate.skill_id),
        )
    }))
}

// Only unconditional `AddBuff` slots aimed at the card's own target count.
fn certain_buffs(skill_id: i32, catalog: &SkillEffectCatalog) -> Vec<i32> {
    catalog
        .get(skill_id)
        .into_iter()
        .flat_map(|effect| &effect.slots)
        .filter(|slot| {
            slot.behavior.spec.kind
                == crate::engine::skill::behavior::classify::BehaviorKind::AddBuff
                && (slot.target.code == 0 || slot.target.code == catalog.logic_target(skill_id))
                && slot.conditions.iter().all(|condition| {
                    matches!(
                        condition.kind,
                        crate::engine::skill::condition::parse::ParsedConditionKind::None(_)
                    )
                })
        })
        .filter_map(|slot| slot.behavior.args.first().copied())
        .collect()
}

fn support_need(
    heal: bool,
    setup: bool,
    buffs: &[i32],
    targets: &[i64],
    managers: &BattleManagers,
    pool: &TargetPool,
) -> Support {
    if heal {
        let lowest = targets
            .iter()
            .filter_map(|uid| pool.entity(*uid))
            .map(|target| i64::from(target.current_hp) * 100 / i64::from(target.max_hp.max(1)))
            .min();
        return match lowest {
            Some(percent) if percent < URGENT_HEAL_BELOW_HP_PERCENT => Support::Urgent,
            Some(percent) if percent < HEAL_BELOW_HP_PERCENT => Support::Needed,
            _ => Support::Unneeded,
        };
    }
    if !buffs.is_empty()
        && targets.iter().all(|uid| {
            buffs
                .iter()
                .all(|buff_id| buff_held(*uid, *buff_id, managers))
        })
    {
        Support::Unneeded
    } else if setup && !buffs.is_empty() {
        Support::Needed
    } else {
        Support::None
    }
}

// Shields stack, so they never count as held.
fn buff_held(uid: i64, buff_id: i32, managers: &BattleManagers) -> bool {
    !is_shield_buff(buff_id, managers)
        && managers
            .buff
            .buff_family_carrier_uid(uid, buff_id)
            .and_then(|buff_uid| managers.buff.snapshot(uid, buff_uid))
            .is_some_and(|buff| buff.duration.is_none_or(|duration| duration > 1))
}

fn is_shield_buff(buff_id: i32, managers: &BattleManagers) -> bool {
    use crate::engine::skill::buff_act::registry::{self, BuffActKind};
    managers
        .buff
        .definition_features(buff_id)
        .into_iter()
        .any(|feature| {
            feature
                .act_id()
                .and_then(|act_id| registry::find(act_id, &feature.act_type))
                .is_some_and(|definition| {
                    matches!(
                        definition.kind,
                        BuffActKind::Shield
                            | BuffActKind::ShieldByBuffLayer
                            | BuffActKind::TeamShareShield
                    )
                })
        })
}

fn card_identity(card: &CardInfo, chosen_skill_id: Option<i32>) -> Option<(i64, i32)> {
    let source_uid = card.uid?;
    let skill_id = chosen_skill_id.or(card.skill_id)?;
    (source_uid != 0 && skill_id > 0).then_some((source_uid, skill_id))
}

fn action_point_cost(
    card: &CardInfo,
    source_uid: i64,
    skill_id: i32,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
) -> i32 {
    i32::from(
        crate::engine::mechanic::card::CardMechanic
            .uses_action_point(card, source_uid, skill_id, managers, catalog),
    )
}

#[allow(clippy::too_many_arguments)]
fn legal_target(
    source_uid: i64,
    skill_id: i32,
    requested_target: Option<i64>,
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
) -> bool {
    let Some(target_uid) = requested_target else {
        return !card_selection_target_options(
            source_uid,
            skill_id,
            None,
            managers,
            pool,
            catalog,
            determinism,
        )
        .is_empty();
    };
    card_selection_target_options(
        source_uid,
        skill_id,
        Some(target_uid),
        managers,
        pool,
        catalog,
        determinism,
    )
    .contains(&target_uid)
}

fn choose_target(
    targets: Vec<i64>,
    skill_id: i32,
    preferred_target: Option<i64>,
    buffs: &[i32],
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
) -> Option<i64> {
    let attack = catalog.is_attack(skill_id);
    targets.into_iter().min_by_key(|target_uid| {
        let target = pool.entity(*target_uid);
        let preferred = preferred_target == Some(*target_uid);
        let already_buffed = buffs
            .iter()
            .any(|buff_id| buff_held(*target_uid, *buff_id, managers));
        let hp_priority = target
            .map(|target| {
                if attack {
                    target.current_hp as i64
                } else {
                    i64::from(target.current_hp) * 1_000 / i64::from(target.max_hp.max(1))
                }
            })
            .unwrap_or(i64::MAX);
        (
            !preferred,
            already_buffed,
            hp_priority,
            target.map(|target| target.position).unwrap_or(i32::MAX),
            *target_uid,
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn target_options(
    source_uid: i64,
    skill_id: i32,
    preferred_target: Option<i64>,
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
) -> Vec<i64> {
    let code = catalog.logic_target(skill_id);
    let request = TargetRequest {
        code,
        raw: Vec::new(),
    };
    let attack = catalog.is_attack(skill_id);
    let context = TargetContext {
        runtime_target_uid: preferred_target.unwrap_or_default(),
        active_skill_id: skill_id,
        active_skill_source_uid: source_uid,
        active_skill_is_attack: attack,
        active_skill_rank: managers.catalog().skill_rank(skill_id),
        active_skill_type: managers.catalog().damage_target_count_kind(code),
        active_skill_effect_tag: catalog.effect_tag(skill_id),
        damage_target_count_kind: managers.catalog().damage_target_count_kind(code),
        ..Default::default()
    };
    crate::engine::skill::target::TargetResolver::resolve_primary_candidates(
        &request,
        skill_id,
        source_uid,
        pool,
        determinism,
        Some(managers),
        context,
    )
}

#[allow(clippy::too_many_arguments)]
fn card_selection_target_options(
    source_uid: i64,
    skill_id: i32,
    preferred_target: Option<i64>,
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
) -> Vec<i64> {
    let code = catalog.logic_target(skill_id);
    CardSelectionResolver::candidates(
        &TargetRequest {
            code,
            raw: Vec::new(),
        },
        skill_id,
        source_uid,
        pool,
        catalog,
        determinism,
        Some(managers),
        TargetContext {
            runtime_target_uid: preferred_target.unwrap_or_default(),
            active_skill_id: skill_id,
            active_skill_source_uid: source_uid,
            active_skill_is_attack: catalog.is_attack(skill_id),
            active_skill_rank: managers.catalog().skill_rank(skill_id),
            active_skill_type: managers.catalog().damage_target_count_kind(code),
            active_skill_effect_tag: catalog.effect_tag(skill_id),
            damage_target_count_kind: managers.catalog().damage_target_count_kind(code),
            ..Default::default()
        },
    )
}
