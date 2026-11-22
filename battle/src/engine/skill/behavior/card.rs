use crate::engine::{
    manager::{
        buff::{BuffCommand, BuffConsume, BuffSelector, DepletedBuff},
        card::{
            CardAddTemporary, CardAddUniversal, CardCommand, CardEnchantHand, CardEnergyChange,
            CardMarkTemporary, CardQueueUse, EnchantedType, HandCardRankUp, QueuedCardRankUp,
        },
        eureka::{EUREKA_RESOURCE_ID, EurekaChange, EurekaCommand},
    },
    skill::{
        action::{SkillExecutionMode, SkillInvocation, SkillRequest},
        behavior::{BehaviorOpContext, classify::BehaviorKind, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::{
            RuleReferences,
            output::{BattleCommand, RuleOp},
        },
    },
};

pub fn rule_op(behavior: &ParsedBehavior) -> Option<RuleOp> {
    let [1, -1, amount] = behavior.args.as_slice() else {
        return None;
    };
    if *amount <= 0 {
        return None;
    }
    Some(RuleOp::Command(BattleCommand::Card(
        CardCommand::ChangeBasicEnergy(CardEnergyChange {
            origin: super::command_origin(behavior)?,
            delta: *amount,
            count: 1,
        }),
    )))
}

pub(super) fn supports_basic_card_energy(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [1, -1, amount] if *amount > 0)
}

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        if behavior.spec.kind == BehaviorKind::RemoveBuffToEnchant {
            return RuleReferences {
                buffs: behavior.args.first().copied().into_iter().collect(),
                ..Default::default()
            };
        }
        if matches!(
            behavior.spec.kind,
            BehaviorKind::AddQueuedSkillCard
                | BehaviorKind::AddSpTempCard
                | BehaviorKind::AddSpTempCard2
        ) {
            return RuleReferences {
                skills: if matches!(
                    behavior.spec.kind,
                    BehaviorKind::AddSpTempCard | BehaviorKind::AddSpTempCard2
                ) {
                    behavior.args.clone()
                } else {
                    behavior.arg_list(1).unwrap_or_default()
                },
                ..Default::default()
            };
        }
        RuleReferences::default()
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        if behavior.spec.kind == BehaviorKind::EnchantHand {
            return enchant_hand_ops(context, behavior);
        }
        if behavior.spec.kind == BehaviorKind::RemoveBuffToEnchant {
            return remove_buff_to_enchant_ops(context, behavior);
        }
        if behavior.spec.kind == BehaviorKind::ChangeHandToTemporary {
            return mark_hand_temporary_ops(context, behavior);
        }
        if behavior.spec.kind == BehaviorKind::CardLevelChange {
            return card_level_change_ops(context, behavior);
        }
        if behavior.spec.kind == BehaviorKind::CardDeckTopRankCorrect {
            return deck_top_rank_correct_ops(context, behavior);
        }
        if behavior.spec.kind == BehaviorKind::ConsumePowerUpgradeSkillCard {
            return power_card_upgrade_ops(context, behavior);
        }
        if behavior.spec.kind == BehaviorKind::AddUniversalCard {
            let [count, rank] = behavior.args.as_slice() else {
                return None;
            };
            return Some(vec![RuleOp::Command(BattleCommand::Card(
                CardCommand::AddUniversal(CardAddUniversal {
                    origin: super::command_origin(behavior)?,
                    count: *count,
                    rank: *rank,
                }),
            ))]);
        }
        if behavior.spec.kind == BehaviorKind::RedealCardKeepStar2 {
            if !behavior.args.is_empty() {
                return None;
            }
            return Some(vec![RuleOp::Command(BattleCommand::Card(
                CardCommand::RedealKeepRanks {
                    origin: super::command_origin(behavior)?,
                },
            ))]);
        }
        if behavior.spec.kind == BehaviorKind::AddQueuedSkillCard {
            return queued_skill_card_ops(context, behavior);
        }
        if matches!(
            behavior.spec.kind,
            BehaviorKind::AddSpTempCard | BehaviorKind::AddSpTempCard2
        ) {
            let [skill_id] = behavior.args.as_slice() else {
                return None;
            };
            let reserve_id = match context.pool.entity(context.source_uid) {
                Some(source) => i64::from(source.model_id),
                None if behavior.spec.kind == BehaviorKind::AddSpTempCard2
                    && matches!(
                        context.source_uid,
                        crate::engine::fight::rules::ATTACKER_SIDE_UID
                            | crate::engine::fight::rules::DEFENDER_SIDE_UID
                    ) =>
                {
                    0
                }
                None => return None,
            };
            let kind = match behavior.spec.kind {
                BehaviorKind::AddSpTempCard => {
                    crate::engine::manager::card::TemporaryCardKind::GenericSkill
                }
                BehaviorKind::AddSpTempCard2 => {
                    crate::engine::manager::card::TemporaryCardKind::PrecastSkill
                }
                _ => unreachable!("temporary-card branch has an exact kind guard"),
            };
            return Some(vec![RuleOp::Command(BattleCommand::Card(
                CardCommand::AddTemporary(CardAddTemporary {
                    origin: super::command_origin(behavior)?,
                    target_uid: context.target_uid,
                    skill_id: *skill_id,
                    hero_id: None,
                    reserve_id,
                    team_type: context.source_team,
                    kind,
                }),
            ))]);
        }
        if behavior.spec.kind == BehaviorKind::BufferflyRecordSkill {
            if !behavior.raw_args.is_empty() {
                return None;
            }
            return crate::engine::skill::buff_act::butterfly_record_skill::record(
                context,
                super::command_origin(behavior)?,
            );
        }
        if behavior.spec.kind == BehaviorKind::AddCardRankByEffectTag {
            let effect_tags = behavior.arg_list(0)?;
            let skill_slot = behavior.arg(1)?;
            let played = context.managers.card.played();
            let upgrades = played
                .iter()
                .filter(|card| {
                    card.card.uid == Some(context.source_uid)
                        && context.pool.skill_slot(
                            context.managers,
                            context.source_uid,
                            card.skill_id,
                        ) == skill_slot
                        && effect_tags
                            .contains(&context.managers.catalog().skill_effect_tag(card.skill_id))
                })
                .filter_map(|card| {
                    let effect_tag = context.managers.catalog().skill_effect_tag(card.skill_id);
                    let levels = played
                        .iter()
                        .filter(|other| {
                            other.card_index != card.card_index
                                && context.managers.catalog().skill_effect_tag(other.skill_id)
                                    == effect_tag
                        })
                        .count() as i32;
                    (levels > 0).then_some(QueuedCardRankUp {
                        card_index: card.card_index,
                        levels,
                    })
                })
                .collect::<Vec<_>>();
            return Some(if upgrades.is_empty() {
                Vec::new()
            } else {
                vec![RuleOp::Command(BattleCommand::Card(
                    CardCommand::RankUpQueuedCards {
                        origin: super::command_origin(behavior)?,
                        upgrades,
                        rewritten: true,
                    },
                ))]
            });
        }
        rule_op(behavior).map(|op| vec![op])
    }

    fn collect_queue_preparation(
        card_index: i32,
        behavior: &ParsedBehavior,
    ) -> Option<Vec<RuleOp>> {
        let [ahead, behind] = behavior.args.as_slice() else {
            return None;
        };
        let changes = [
            (card_index.checked_sub(1), *ahead),
            (card_index.checked_add(1), *behind),
        ]
        .into_iter()
        .filter_map(|(index, levels)| {
            let card_index = index.filter(|index| *index > 0)?;
            (levels != 0).then_some(crate::engine::manager::card::QueuedCardRankChange {
                card_index,
                levels,
            })
        })
        .collect::<Vec<_>>();
        (!changes.is_empty()).then(|| {
            vec![RuleOp::Command(BattleCommand::Card(
                CardCommand::ChangeQueuedRanks {
                    origin: super::command_origin(behavior)
                        .expect("registered queue preparation has an origin"),
                    changes,
                },
            ))]
        })
    }
}

fn queued_skill_card_ops(
    context: BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let crate::engine::event::payload::BattleEvent::ActionQueueCommitted {
        team, action_count, ..
    } = context.event?
    else {
        return Some(Vec::new());
    };
    if *team != context.source_team {
        return Some(Vec::new());
    }
    let (thresholds, skill_ids, count) = queued_skill_card_arguments(behavior)?;
    let total_rank = context.managers.card.total_played_rank();
    let Some(skill_id) = thresholds
        .windows(2)
        .zip(skill_ids)
        .find_map(|(range, skill_id)| {
            (total_rank >= range[0] && total_rank < range[1]).then_some(skill_id)
        })
    else {
        return Some(Vec::new());
    };
    let owner = context.pool.entity(context.source_uid)?;
    let first_index = action_count
        .checked_add(context.managers.card.queued_use_cards().len())?
        .checked_add(1)?;
    (0..usize::try_from(count).ok()?)
        .map(|offset| {
            let card_index = i32::try_from(first_index.checked_add(offset)?).ok()?;
            let card = crate::engine::manager::card::pool::card_for_target(owner, skill_id)?;
            let mut action: SkillInvocation = SkillRequest {
                source_uid: context.source_uid,
                skill_id,
            }
            .into();
            action.card_index = card_index;
            action.mode = SkillExecutionMode::Active;
            Some(RuleOp::Command(BattleCommand::Card(
                CardCommand::QueueUseCard(CardQueueUse {
                    origin: super::command_origin(behavior)?,
                    card_index,
                    card,
                    team_type: *team,
                    source_skill_id: 0,
                    reserve_id: 0,
                    action: Some(action),
                }),
            )))
        })
        .collect()
}

fn queued_skill_card_arguments(behavior: &ParsedBehavior) -> Option<(Vec<i32>, Vec<i32>, i32)> {
    let thresholds = behavior.arg_list(0)?;
    let skill_ids = behavior.arg_list(1)?;
    let count = behavior.arg(2)?;
    let reserved_skill_ids = behavior.arg_list(3)?;
    (thresholds.len() == skill_ids.len() + 1
        && thresholds.windows(2).all(|pair| pair[0] < pair[1])
        && skill_ids.iter().all(|skill_id| *skill_id > 0)
        && count > 0
        && reserved_skill_ids.iter().all(|skill_id| *skill_id > 0))
    .then_some((thresholds, skill_ids, count))
}

fn enchant_hand_ops(
    mut context: BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let [enchant_id, duration, 3, count] = behavior.args.as_slice() else {
        return None;
    };
    let enchant = EnchantedType::try_from(*enchant_id).ok()?;
    let mut candidates = context
        .managers
        .card
        .hand()
        .iter()
        .enumerate()
        .filter(|(_, card)| !card.temp_card.unwrap_or_default())
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let indices = choose_hand_indices(
        &mut context,
        behavior,
        &mut candidates,
        usize::try_from(*count).ok()?,
    )?;
    (!indices.is_empty()).then(|| {
        vec![RuleOp::Command(BattleCommand::Card(
            CardCommand::EnchantHand(CardEnchantHand {
                origin: super::command_origin(behavior)
                    .expect("registered card enchant has an origin"),
                indices,
                enchant,
                duration: *duration,
                team_type: 1,
            }),
        ))]
    })
}

fn remove_buff_to_enchant_ops(
    mut context: BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let [buff_id, enchant_id, duration] = behavior.args.as_slice() else {
        return None;
    };
    let enchant = EnchantedType::try_from(*enchant_id).ok()?;
    let count = usize::try_from(
        context
            .managers
            .buff
            .buff_id_amount(context.source_uid, *buff_id),
    )
    .ok()?;
    if count == 0 {
        return Some(Vec::new());
    }

    let catalog = context.managers.catalog();
    let hand = context.managers.card.hand();
    let (mut owned, mut allied): (Vec<_>, Vec<_>) = hand
        .iter()
        .enumerate()
        .filter(|(_, card)| {
            !card.temp_card.unwrap_or_default()
                && card
                    .skill_id
                    .is_some_and(|skill_id| !catalog.skill_is_big(skill_id))
                && (1..=3).contains(&catalog.card_skill_rank(card))
        })
        .map(|(index, _)| index)
        .partition(|index| hand[*index].uid == Some(context.source_uid));
    let mut indices = choose_hand_indices(&mut context, behavior, &mut owned, count)?;
    indices.extend(choose_hand_indices(
        &mut context,
        behavior,
        &mut allied,
        count.saturating_sub(indices.len()),
    )?);

    let origin = super::command_origin(behavior)?;
    let mut ops = vec![RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(
        BuffConsume {
            origin,
            target_uid: context.source_uid,
            selector: BuffSelector::ExactId(*buff_id),
            amount: i32::try_from(count).ok()?,
            depleted: DepletedBuff::Remove,
        },
    )))];
    if !indices.is_empty() {
        ops.push(RuleOp::Command(BattleCommand::Card(
            CardCommand::EnchantHand(CardEnchantHand {
                origin,
                indices,
                enchant,
                duration: *duration,
                team_type: context.source_team,
            }),
        )));
    }
    Some(ops)
}

fn choose_hand_indices(
    context: &mut BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
    candidates: &mut Vec<usize>,
    count: usize,
) -> Option<Vec<usize>> {
    let mut indices = Vec::new();
    for _ in 0..count.min(candidates.len()) {
        let hand_index = context
            .determinism
            .take_hand_card_choice(
                context.active_skill_id,
                behavior.spec.key.opcode,
                candidates,
            )
            .or_else(|| {
                context
                    .determinism
                    .lua_random_index(candidates.len())
                    .map(|chosen| candidates[chosen])
            })?;
        let selected = candidates
            .iter()
            .position(|candidate| *candidate == hand_index)?;
        candidates.swap_remove(selected);
        indices.push(hand_index);
    }
    Some(indices)
}

fn mark_hand_temporary_ops(
    context: BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let [3, 0] = behavior.args.as_slice() else {
        return None;
    };
    let indices = (0..context.managers.card.hand().len()).collect::<Vec<_>>();
    (!indices.is_empty()).then(|| {
        vec![RuleOp::Command(BattleCommand::Card(
            CardCommand::MarkTemporary(CardMarkTemporary {
                origin: super::command_origin(behavior)
                    .expect("registered temporary-card rule has an origin"),
                indices,
                team_type: 1,
                config_effect: behavior.spec.key.opcode,
            }),
        ))]
    })
}

fn card_level_change_ops(
    context: BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let (mode, count) = match behavior.args.as_slice() {
        [1, count, 1] if *count > 0 => (1, *count),
        [3, 1, 1] => (3, 1),
        _ => return None,
    };
    let owner = context.pool.entity(context.target_uid)?;
    let mut candidates = context
        .managers
        .card
        .hand()
        .iter()
        .enumerate()
        .filter(|(_, card)| {
            card.uid == Some(context.target_uid) && !card.temp_card.unwrap_or_default()
        })
        .filter_map(|(hand_index, card)| next_skill(owner, card.skill_id?).map(|_| hand_index))
        .collect::<Vec<_>>();
    let origin = super::command_origin(behavior)?;
    if candidates.is_empty() {
        return Some(Vec::new());
    }
    if mode == 3 {
        let hand_index = *candidates.first()?;
        return Some(vec![RuleOp::Command(BattleCommand::Card(
            CardCommand::RankUpHand(HandCardRankUp {
                origin,
                owner_uid: context.target_uid,
                hand_index,
            }),
        ))]);
    }
    let mut ops = Vec::new();
    for _ in 0..usize::try_from(count).ok()? {
        let hand_index = context
            .determinism
            .take_hand_rank_choice(behavior.spec.key.opcode, context.target_uid, &candidates)
            .or_else(|| {
                context
                    .determinism
                    .lua_random_index(candidates.len())
                    .map(|chosen| candidates[chosen])
            })?;
        let chosen = candidates
            .iter()
            .position(|candidate| *candidate == hand_index)?;
        ops.push(RuleOp::Command(BattleCommand::Card(
            CardCommand::RankUpHand(HandCardRankUp {
                origin,
                owner_uid: context.target_uid,
                hand_index,
            }),
        )));
        candidates.swap_remove(chosen);
        if candidates.is_empty() {
            break;
        }
    }
    Some(ops)
}

fn deck_top_rank_correct_ops(
    _context: BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let [from, to, rank_delta] = behavior.args.as_slice() else {
        return None;
    };
    let from = usize::try_from(*from).ok()?;
    let to = usize::try_from(*to).ok()?;
    Some(vec![RuleOp::Command(BattleCommand::Card(
        CardCommand::RankUpDeckRange(crate::engine::manager::card::CardDeckRankUpRange {
            origin: super::command_origin(behavior)?,
            from,
            to,
            rank_delta: *rank_delta,
        }),
    ))])
}

fn power_card_upgrade_ops(
    context: BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let [rank_one_cost, rank_two_cost] = behavior.args.as_slice() else {
        return None;
    };
    if *rank_one_cost <= 0 || *rank_two_cost <= 0 {
        return None;
    }
    let owner = context.pool.entity(context.source_uid)?;
    let mut hand = context.managers.card.hand().to_vec();
    let mut power = context
        .managers
        .eureka
        .get(context.source_uid, EUREKA_RESOURCE_ID)
        .current;
    let origin = super::command_origin(behavior)?;
    let mut ops = Vec::new();

    loop {
        let mut candidates = hand
            .iter()
            .enumerate()
            .filter(|(_, card)| {
                card.uid == Some(context.source_uid) && !card.temp_card.unwrap_or_default()
            })
            .filter_map(|(hand_index, card)| {
                let skill_id = card.skill_id?;
                let rank = context.managers.catalog().skill_rank(skill_id);
                let cost = match rank {
                    1 => *rank_one_cost,
                    2 => *rank_two_cost,
                    _ => return None,
                };
                let next_skill_id = next_skill(owner, skill_id)?;
                (cost <= power).then_some((hand_index, rank, cost, next_skill_id))
            })
            .collect::<Vec<_>>();
        let Some(lowest_rank) = candidates.iter().map(|(_, rank, _, _)| *rank).min() else {
            break;
        };
        candidates.retain(|(_, rank, _, _)| *rank == lowest_rank);
        let candidate_indices = candidates
            .iter()
            .map(|(hand_index, _, _, _)| *hand_index)
            .collect::<Vec<_>>();
        let hand_index = context
            .determinism
            .take_hand_rank_choice(
                behavior.spec.key.opcode,
                context.source_uid,
                &candidate_indices,
            )
            .or_else(|| {
                context
                    .determinism
                    .lua_random_index(candidates.len())
                    .map(|chosen| candidates[chosen].0)
            })?;
        let (_, _, cost, next_skill_id) = candidates
            .iter()
            .find(|(candidate_index, _, _, _)| *candidate_index == hand_index)
            .copied()?;

        ops.push(RuleOp::Command(BattleCommand::Eureka(
            EurekaCommand::Change(EurekaChange {
                origin,
                source_uid: context.source_uid,
                target_uid: context.source_uid,
                power_id: EUREKA_RESOURCE_ID,
                delta: -cost,
                effect_type: sonettobuf::effect_type_enum::EffectType::Powerchange as i32,
            }),
        )));
        ops.push(RuleOp::Command(BattleCommand::Card(
            CardCommand::RankUpHand(HandCardRankUp {
                origin,
                owner_uid: context.source_uid,
                hand_index,
            }),
        )));
        hand[hand_index].skill_id = Some(next_skill_id);
        power -= cost;
    }

    Some(ops)
}

fn next_skill(owner: &crate::engine::skill::target::TargetEntity, skill_id: i32) -> Option<i32> {
    owner
        .skill_group1
        .windows(2)
        .chain(owner.skill_group2.windows(2))
        .find_map(|pair| (pair[0] == skill_id).then_some(pair[1]))
}

pub(super) fn supports_around_change_rank(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [ahead, behind] if *ahead != 0 || *behind != 0)
}

pub(super) fn supports_enchant_hand(behavior: &ParsedBehavior) -> bool {
    matches!(
        behavior.args.as_slice(),
        [enchant_id, _, 3, count]
            if EnchantedType::try_from(*enchant_id).is_ok() && *count > 0
    )
}

pub(super) fn supports_remove_buff_to_enchant(behavior: &ParsedBehavior) -> bool {
    (behavior.raw_args.is_empty() || behavior.raw_args.len() == 3)
        && matches!(
            behavior.args.as_slice(),
            [buff_id, enchant_id, duration]
                if *buff_id > 0
                    && *enchant_id == EnchantedType::Ramona.id()
                    && matches!(*duration, 1 | 2)
        )
}

pub(super) fn supports_mark_hand_temporary(behavior: &ParsedBehavior) -> bool {
    behavior.args.as_slice() == [3, 0]
}

pub(super) fn supports_card_level_change(behavior: &ParsedBehavior) -> bool {
    match behavior.args.as_slice() {
        [1, count, 1] => *count > 0,
        [3, 1, 1] => true,
        _ => false,
    }
}

pub(super) fn supports_deck_top_rank_correct(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [1, 1, 1] | [2, 3, 1] | [4, 4, 1])
}

pub(super) fn supports_power_card_upgrade(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [rank_one_cost, rank_two_cost] if *rank_one_cost > 0 && *rank_two_cost > 0)
}

pub(super) fn supports_queued_skill_card(behavior: &ParsedBehavior) -> bool {
    queued_skill_card_arguments(behavior).is_some()
}

pub(super) fn supports_temporary_skill_card(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [skill_id] if *skill_id > 0)
}

pub(super) fn supports_rank_by_effect_tag(behavior: &ParsedBehavior) -> bool {
    behavior.arg_list(0).is_some() && behavior.arg(1).is_some()
}

#[cfg(test)]
mod tests;
