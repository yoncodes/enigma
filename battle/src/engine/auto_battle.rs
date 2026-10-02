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
    },
    round::{command::RoundCommand, state::RoundState},
    runtime::{determinism::RoundDeterminism, schedule::card_skill_is_blocked},
    skill::{
        buff_act::action_point::skill_uses_action_point,
        effect::SkillEffectCatalog,
        target::{TargetContext, TargetPool, TargetRequest, TargetResolver},
    },
};

const HEAL_BELOW_HP_PERCENT: i64 = 80;
const URGENT_HEAL_BELOW_HP_PERCENT: i64 = 60;

#[derive(Debug, Clone, Copy)]
struct Candidate {
    card_index: usize,
    source_uid: i64,
    skill_id: i32,
    target_uid: i64,
    normal_ap: i32,
    chosen_skill_id: Option<i32>,
    free: bool,
    ultimate: bool,
    energy: bool,
    first_for_owner: bool,
    support: Support,
    damage_rate: i32,
    rank: i32,
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
) -> AutoRoundReply {
    // Every card resolves before any skill runs, so indices, AP and readiness use round-start state.
    let pool = TargetPool::from_fight_with_catalog(managers.catalog(), fight);
    let options: Vec<i32> = managers
        .card
        .hand()
        .iter()
        .chain(managers.card.team_cards())
        .flat_map(|card| card_choice_options(card, managers))
        .chain(
            request
                .opers
                .iter()
                .filter_map(|oper| oper.param3)
                .filter(|skill_id| *skill_id > 0),
        )
        .collect();
    let catalog = if options.is_empty() {
        std::borrow::Cow::Borrowed(catalog)
    } else {
        let mut extended = catalog.clone();
        managers
            .catalog()
            .extend_skill_roots(&mut extended, options, std::iter::empty());
        std::borrow::Cow::Owned(extended)
    };
    let catalog = catalog.as_ref();
    let mut cards = managers.card.clone();
    let mut sim = managers.clone();
    let mut sim_determinism = determinism.clone();
    let mut normal_ap = round_state.act_point.max(0);
    let mut played_owners = HashSet::new();
    if !apply_prefix(
        &mut cards,
        &mut sim,
        &mut normal_ap,
        &mut played_owners,
        &request.opers,
        managers,
        &pool,
        catalog,
        &mut sim_determinism,
    ) {
        return reply(request, Vec::new(), devices_opers);
    }

    let live_pool = pool.runtime_view(managers);
    let mut opers = Vec::new();
    let mut reported_unsupported = HashSet::new();
    loop {
        let choices: Vec<_> = cards
            .hand()
            .iter()
            .chain(cards.team_cards())
            .map(|card| random_card_choice(card, managers, &mut sim_determinism))
            .collect();
        let Some(candidate) = best_candidate(
            &cards,
            &choices,
            &played_owners,
            normal_ap,
            request.to_id,
            managers,
            &sim,
            &live_pool,
            &pool.runtime_view(&sim),
            catalog,
            &sim_determinism,
            &mut reported_unsupported,
        ) else {
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
        if !simulate_play(
            &cards,
            &mut sim,
            &pool,
            catalog,
            &mut sim_determinism,
            play.clone(),
        ) || !play_card(&mut cards, play)
        {
            break;
        }
        normal_ap = normal_ap.saturating_sub(candidate.normal_ap);
        if candidate.normal_ap > 0 {
            played_owners.insert(candidate.source_uid);
        }
        opers.push(BeginRoundOper {
            oper_type: Some(CardOpType::PlayCard.id()),
            param1: Some(candidate.card_index as i32 + 1),
            to_id: Some(candidate.target_uid),
            param3: candidate.chosen_skill_id,
            ..Default::default()
        });
    }

    reply(request, opers, devices_opers)
}

fn card_choice_options(card: &CardInfo, managers: &BattleManagers) -> Vec<i32> {
    card.skill_id
        .and_then(|skill_id| {
            managers
                .catalog()
                .game_data()
                .fight_card_choice
                .get(skill_id)
        })
        .map(|row| {
            row.choice_sk_ills
                .split('#')
                .filter_map(|option| option.trim().parse().ok())
                .collect()
        })
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
        cloth_skill: None,
        devices_opers,
    }
}

fn simulate_play(
    cards: &CardManager,
    sim: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    play: CardPlay,
) -> bool {
    sim.card = cards.clone();
    crate::engine::runtime::schedule::run_player_action_queue(
        sim,
        pool,
        catalog,
        determinism,
        TargetContext::default(),
        [play],
        1,
        crate::engine::manager::emitter::UID,
    )
    .inspect_err(|error| tracing::warn!(?error, "auto-battle stopped planning at a failed play"))
    .is_ok()
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
    sim: &mut BattleManagers,
    normal_ap: &mut i32,
    played_owners: &mut HashSet<i64>,
    opers: &[BeginRoundOper],
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
) -> bool {
    for oper in opers {
        let Some(command) = RoundCommand::from_oper(oper) else {
            return false;
        };
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
            RoundCommand::UseAssistBoss { .. } => return false,
            RoundCommand::PlayCard {
                card_index,
                target_uid,
                chosen_skill_id,
                recorded_skill,
            } => {
                let Some(card) = cards.visible_card(card_index) else {
                    return false;
                };
                let Some((source_uid, skill_id)) = card_identity(card, chosen_skill_id) else {
                    return false;
                };
                if card_skill_is_blocked(managers, catalog, source_uid, skill_id)
                    || !legal_target(
                        source_uid,
                        skill_id,
                        target_uid,
                        sim,
                        &pool.runtime_view(sim),
                        catalog,
                        determinism,
                    )
                {
                    return false;
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
                if ap_cost > *normal_ap
                    || !simulate_play(cards, sim, pool, catalog, determinism, play.clone())
                    || !play_card(cards, play)
                {
                    return false;
                }
                *normal_ap = normal_ap.saturating_sub(ap_cost);
                if ap_cost > 0 {
                    played_owners.insert(source_uid);
                }
                continue;
            }
        };
        if ap_cost > *normal_ap || cards.execute_command(card_command).is_err() {
            return false;
        }
        *normal_ap = normal_ap.saturating_sub(ap_cost);
        if cards
            .execute_command(CardCommand::ComposeAdjacent {
                origin: CARD_PLAY_ORIGIN,
            })
            .is_err()
        {
            return false;
        }
    }
    true
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
    reported_unsupported: &mut HashSet<i32>,
) -> Option<Candidate> {
    cards
        .hand()
        .iter()
        .chain(cards.team_cards())
        .enumerate()
        .filter_map(|(card_index, card)| {
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
            // Stage-rule temporary cards are left for the player.
            if temporary && managers.catalog().skill_hero_id(skill_id) != Some(source.model_id) {
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
            // An enemy's debuff may come from a passive, so debuff cards never count as redundant.
            let may_be_redundant = !catalog.is_attack(skill_id)
                && effect_tag
                    != crate::engine::skill::effect::catalog::SkillEffectTag::Debuff as i32;
            let support = support_need(
                effect_tag == crate::engine::skill::effect::catalog::SkillEffectTag::Heal as i32,
                if may_be_redundant { &buffs } else { &[] },
                &targets,
                sim,
                pool,
            );
            let target_uid = choose_target(
                targets,
                skill_id,
                preferred_target,
                &buffs,
                sim,
                pool,
                catalog,
            )?;
            Some(Candidate {
                card_index,
                source_uid,
                skill_id,
                target_uid,
                normal_ap: normal_ap_cost,
                chosen_skill_id,
                free: normal_ap_cost == 0,
                ultimate,
                first_for_owner: !played_owners.contains(&source_uid)
                    && support != Support::Unneeded,
                energy: effect_tag
                    == crate::engine::skill::effect::catalog::SkillEffectTag::Device as i32,
                support,
                damage_rate: catalog.damage_rate(skill_id),
                rank: managers.catalog().card_skill_rank(card),
            })
        })
        .max_by_key(|candidate| {
            // Official auto casts ready ultimates left to right.
            let strength = (!candidate.ultimate).then_some((candidate.damage_rate, candidate.rank));
            (
                candidate.free,
                candidate.ultimate,
                candidate.support == Support::Urgent,
                // Give every hero a card before any hero plays a second one.
                candidate.first_for_owner,
                candidate.energy,
                candidate.support,
                strength,
                Reverse(candidate.card_index),
                Reverse(candidate.source_uid),
                Reverse(candidate.skill_id),
            )
        })
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
        !card.temp_card.unwrap_or_default()
            && skill_uses_action_point(
                &managers.buff.active_features(&managers.hp),
                source_uid,
                catalog.is_big_skill(skill_id),
            ),
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
        return !target_options(
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
    target_options(
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
    TargetResolver::resolve_primary_candidates(
        &request,
        skill_id,
        source_uid,
        pool,
        determinism,
        Some(managers),
        context,
    )
}
