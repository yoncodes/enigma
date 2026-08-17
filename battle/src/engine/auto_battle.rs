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

#[derive(Debug, Clone, Copy)]
struct Candidate {
    card_index: usize,
    source_uid: i64,
    skill_id: i32,
    target_uid: i64,
    normal_ap: i32,
    ultimate: bool,
    damage_rate: i32,
    rank: i32,
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
    let pool =
        TargetPool::from_fight_with_catalog(managers.catalog(), fight).runtime_view(managers);
    let mut cards = managers.card.clone();
    let mut normal_ap = round_state.act_point.max(0);
    if !apply_prefix(
        &mut cards,
        &mut normal_ap,
        &request.opers,
        managers,
        &pool,
        catalog,
        determinism,
    ) {
        return reply(request, Vec::new(), devices_opers);
    }

    let mut opers = Vec::new();
    let mut reported_unsupported = HashSet::new();
    while let Some(candidate) = best_candidate(
        &cards,
        normal_ap,
        request.to_id,
        managers,
        &pool,
        catalog,
        determinism,
        &mut reported_unsupported,
    ) {
        let play = CardPlay {
            origin: CARD_PLAY_ORIGIN,
            hand_index: candidate.card_index,
            target_uid: Some(candidate.target_uid),
            chosen_skill_id: None,
            choice: None,
            recorded_skill: None,
        };
        if cards.execute_command(CardCommand::Play(play)).is_err() {
            break;
        }
        let _ = cards.execute_command(CardCommand::ComposeAdjacent {
            origin: CARD_PLAY_ORIGIN,
        });
        normal_ap = normal_ap.saturating_sub(candidate.normal_ap);
        opers.push(BeginRoundOper {
            oper_type: Some(CardOpType::PlayCard.id()),
            param1: Some(candidate.card_index as i32 + 1),
            to_id: Some(candidate.target_uid),
            ..Default::default()
        });
    }

    reply(request, opers, devices_opers)
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

#[allow(clippy::too_many_arguments)]
fn apply_prefix(
    cards: &mut CardManager,
    normal_ap: &mut i32,
    opers: &[BeginRoundOper],
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
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
                        managers,
                        pool,
                        catalog,
                        determinism,
                    )
                {
                    return false;
                }
                let ap_cost = action_point_cost(card, source_uid, skill_id, managers, catalog);
                (
                    CardCommand::Play(CardPlay {
                        origin: CARD_PLAY_ORIGIN,
                        hand_index: card_index,
                        target_uid,
                        chosen_skill_id,
                        choice: None,
                        recorded_skill,
                    }),
                    ap_cost,
                )
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
    normal_ap: i32,
    preferred_target: Option<i64>,
    managers: &BattleManagers,
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
            let (source_uid, skill_id) = card_identity(card, None)?;
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
            let source = pool.entity(source_uid)?;
            let ultimate = crate::engine::mechanic::card::CardMechanic
                .is_ultimate_skill(managers, skill_id, source);
            if ultimate
                && !crate::engine::mechanic::card::CardMechanic.ultimate_ready(managers, source)
            {
                return None;
            }
            let target_uid = choose_target(
                source_uid,
                skill_id,
                preferred_target,
                managers,
                pool,
                catalog,
                determinism,
            )?;
            Some(Candidate {
                card_index,
                source_uid,
                skill_id,
                target_uid,
                normal_ap: normal_ap_cost,
                ultimate,
                damage_rate: catalog.damage_rate(skill_id),
                rank: managers.catalog().card_skill_rank(card),
            })
        })
        .max_by_key(|candidate| {
            (
                candidate.ultimate,
                candidate.damage_rate,
                candidate.rank,
                Reverse(candidate.card_index),
                Reverse(candidate.source_uid),
                Reverse(candidate.skill_id),
            )
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
        return choose_target(
            source_uid,
            skill_id,
            None,
            managers,
            pool,
            catalog,
            determinism,
        )
        .is_some();
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

#[allow(clippy::too_many_arguments)]
fn choose_target(
    source_uid: i64,
    skill_id: i32,
    preferred_target: Option<i64>,
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
) -> Option<i64> {
    let targets = target_options(
        source_uid,
        skill_id,
        preferred_target,
        managers,
        pool,
        catalog,
        determinism,
    );
    let attack = catalog.is_attack(skill_id);
    targets.into_iter().min_by_key(|target_uid| {
        let target = pool.entity(*target_uid);
        let preferred = preferred_target == Some(*target_uid);
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
        active_skill_type: catalog.skill_type(skill_id),
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
