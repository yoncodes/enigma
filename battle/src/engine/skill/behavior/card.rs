use crate::engine::{
    manager::{
        buff::{BuffCommand, BuffSetState},
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
    let [1, delta, count] = behavior.args.as_slice() else {
        return None;
    };
    if *delta == 0 || *count <= 0 {
        return None;
    }
    Some(RuleOp::Command(BattleCommand::Card(
        CardCommand::ChangeBasicEnergy(CardEnergyChange {
            origin: super::command_origin(behavior)?,
            delta: *delta,
            count: *count,
        }),
    )))
}

pub(super) fn supports_basic_card_energy(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [1, -1, count] if *count > 0)
}

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        if matches!(
            behavior.spec.kind,
            BehaviorKind::AddQueuedSkillCard | BehaviorKind::AddSpTempCard2
        ) {
            return RuleReferences {
                skills: if behavior.spec.kind == BehaviorKind::AddSpTempCard2 {
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
        if behavior.spec.kind == BehaviorKind::ChangeHandToTemporary {
            return mark_hand_temporary_ops(context, behavior);
        }
        if behavior.spec.kind == BehaviorKind::CardLevelChange {
            return card_level_change_ops(context, behavior);
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
        if behavior.spec.kind == BehaviorKind::AddSpTempCard2 {
            let [skill_id] = behavior.args.as_slice() else {
                return None;
            };
            let reserve_id = i64::from(context.pool.entity(context.source_uid)?.model_id);
            return Some(vec![RuleOp::Command(BattleCommand::Card(
                CardCommand::AddTemporary(CardAddTemporary {
                    origin: super::command_origin(behavior)?,
                    target_uid: context.target_uid,
                    skill_id: *skill_id,
                    reserve_id,
                    team_type: context.source_team,
                }),
            ))]);
        }
        if behavior.spec.kind == BehaviorKind::BufferflyRecordSkill {
            if !behavior.raw_args.is_empty()
                || context.target.recorded_skill_id <= 0
                || context.target.recorded_skill_source_uid == 0
            {
                return Some(Vec::new());
            }
            let feature = context
                .managers
                .buff
                .active_features(&context.managers.hp)
                .into_iter()
                .find(|feature| {
                    feature.owner_uid == context.source_uid
                        && crate::engine::skill::buff_act::is_kind(
                            feature,
                            crate::engine::skill::buff_act::registry::BuffActKind::ButterflyRecordSkill,
                        )
                });
            let Some(feature) = feature
                .filter(|feature| allows_recorded_skill(feature, context.target.recorded_skill_id))
            else {
                return Some(Vec::new());
            };
            let count = *feature.values.get(1)?;
            let act_id = feature.act_id()?;
            return Some(vec![RuleOp::Command(BattleCommand::Buff(
                BuffCommand::SetState(BuffSetState {
                    origin: super::command_origin(behavior)?,
                    target_uid: context.source_uid,
                    buff_uid: feature.buff_uid,
                    ex_info: None,
                    params: None,
                    act_info: Some(vec![sonettobuf::BuffActInfo {
                        act_id: Some(act_id),
                        param: vec![count, context.target.recorded_skill_id],
                        str_param: Some(format!("{count},{}", context.target.recorded_skill_id)),
                    }]),
                }),
            ))]);
        }
        if behavior.spec.kind == BehaviorKind::AddCardRankByEffectTag {
            let effect_tags = behavior.arg_list(0)?;
            let skill_slot = behavior.arg(1)?;
            let played = context.managers.card.played();
            let upgrades = played
                .iter()
                .filter(|card| {
                    card.card.uid == Some(context.source_uid)
                        && context.pool.skill_slot(context.source_uid, card.skill_id) == skill_slot
                        && effect_tags.contains(
                            &crate::engine::skill::effect::catalog::configured_effect_tag(
                                card.skill_id,
                            ),
                        )
                })
                .filter_map(|card| {
                    let effect_tag =
                        crate::engine::skill::effect::catalog::configured_effect_tag(card.skill_id);
                    let levels = played
                        .iter()
                        .filter(|other| {
                            other.card_index != card.card_index
                                && crate::engine::skill::effect::catalog::configured_effect_tag(
                                    other.skill_id,
                                ) == effect_tag
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
                CardCommand::ChangeAroundQueuedRanks {
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
    let crate::engine::event::payload::BattleEvent::ActionQueueCommitted { team, cards, .. } =
        context.event?
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
    let first_index = cards
        .len()
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
    context: BehaviorOpContext<'_>,
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
    let mut indices = Vec::new();
    for _ in 0..usize::try_from(*count).ok()?.min(candidates.len()) {
        let hand_index = context
            .determinism
            .take_hand_card_choice(
                context.active_skill_id,
                behavior.spec.key.opcode,
                &candidates,
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
    let [selection_mode, count, levels] = behavior.args.as_slice() else {
        return None;
    };
    if *selection_mode != 1 || *count <= 0 || *levels != 1 {
        return None;
    }
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
    let mut ops = Vec::new();
    for _ in 0..usize::try_from(*count).ok()? {
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
                let rank = crate::engine::entity::skill::skill_rank(skill_id);
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

pub(super) fn supports_mark_hand_temporary(behavior: &ParsedBehavior) -> bool {
    behavior.args.as_slice() == [3, 0]
}

pub(super) fn supports_card_level_change(behavior: &ParsedBehavior) -> bool {
    matches!(
        behavior.args.as_slice(),
        [selection_mode, count, levels]
            if *selection_mode == 1 && *count > 0 && *levels == 1
    )
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

fn allows_recorded_skill(
    feature: &crate::engine::manager::buff::ActiveBuffFeature,
    skill_id: i32,
) -> bool {
    if crate::engine::skill::effect::catalog::configured_is_big_skill(skill_id) {
        return false;
    }
    let effect_tag = crate::engine::skill::effect::catalog::configured_effect_tag(skill_id);
    feature
        .values
        .get(3..)
        .is_some_and(|allowed| allowed.contains(&effect_tag))
}

pub(super) fn supports_rank_by_effect_tag(behavior: &ParsedBehavior) -> bool {
    behavior.arg_list(0).is_some() && behavior.arg(1).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_temporary_card_uses_the_source_model_as_reserve_id() {
        use sonettobuf::{Fight, FightEntityInfo, FightTeam};

        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3149),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = crate::engine::manager::BattleManagers::seeded(&fight);
        let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
        let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
        let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
        let mut target = crate::engine::skill::target::TargetContext::default();
        let behavior = ParsedBehavior::new(60300, "AddSpTempCard2", vec![31446013]);

        let ops = Handler::emit_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 0,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        )
        .unwrap();

        assert!(matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::Card(CardCommand::AddTemporary(add)))]
                if add.target_uid == 10
                    && add.skill_id == 31446013
                    && add.reserve_id == 3149
                    && add.team_type == 1
        ));
        assert_eq!(Handler::references(&behavior).skills, vec![31446013]);
    }

    #[test]
    fn queued_skill_card_uses_committed_rank_threshold_and_next_queue_index() {
        use sonettobuf::{CardInfo, Fight, FightEntityInfo, FightTeam};

        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                assist_boss: Some(FightEntityInfo {
                    uid: Some(-1),
                    model_id: Some(1),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-2),
                    team_type: Some(2),
                    current_hp: Some(100),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = crate::engine::manager::BattleManagers::seeded(&fight);
        managers.card = crate::engine::manager::card::CardManager::new(
            [30970111, 30970111, 30970111, 30970112]
                .into_iter()
                .map(|skill_id| CardInfo {
                    uid: Some(10),
                    skill_id: Some(skill_id),
                    ..Default::default()
                })
                .collect(),
        );
        for _ in 0..4 {
            managers.card.play_card(0, None, None, None).unwrap();
        }
        let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
        let event = crate::engine::event::payload::BattleEvent::ActionQueueCommitted {
            team: 1,
            emitter_uid: 1,
            cards: vec![
                CardInfo {
                    skill_id: Some(30970111),
                    ..Default::default()
                },
                CardInfo {
                    skill_id: Some(30970111),
                    ..Default::default()
                },
                CardInfo {
                    skill_id: Some(30970111),
                    ..Default::default()
                },
                CardInfo {
                    skill_id: Some(30970112),
                    ..Default::default()
                },
            ],
        };
        let behavior = ParsedBehavior::from_spec(
            crate::engine::skill::behavior::classify::BehaviorSpec::new(60070, "AddUseSkillCard"),
            vec![3, 30970171, 1, 1],
            vec![
                "3,5,7,99".into(),
                "30970171,30970172,30970173".into(),
                "1".into(),
                "1,30970111".into(),
            ],
        );
        let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
        let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
        let mut target = crate::engine::skill::target::TargetContext::default();

        let ops = Handler::emit_ops(
            BehaviorOpContext {
                source_uid: -1,
                source_team: 1,
                target_uid: -1,
                active_skill_id: 30970161,
                transfer_count: 1,
                event: Some(&event),
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        )
        .unwrap();

        assert!(matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::Card(CardCommand::QueueUseCard(
                CardQueueUse {
                    card_index: 5,
                    card,
                    team_type: 1,
                    action: Some(SkillInvocation {
                        plan: SkillRequest {
                            source_uid: -1,
                            skill_id: 30970172,
                        },
                        mode: SkillExecutionMode::Active,
                        target: crate::engine::skill::action::SkillTarget::Configured,
                        ..
                    }),
                    ..
                }
            )))] if card.skill_id == Some(30970172) && card.hero_id == Some(1)
        ));
    }

    #[test]
    fn around_change_rank_prepares_both_neighbor_mutations() {
        let behavior = ParsedBehavior::new(60075, "AroundChangeRank", vec![1, -1]);

        let ops = <Handler as BehaviorHandler>::collect_queue_preparation(6, &behavior).unwrap();

        let [
            RuleOp::Command(BattleCommand::Card(CardCommand::ChangeAroundQueuedRanks {
                changes,
                ..
            })),
        ] = ops.as_slice()
        else {
            panic!("AroundChangeRank must emit one owned card command")
        };
        assert_eq!(
            changes,
            &vec![
                crate::engine::manager::card::QueuedCardRankChange {
                    card_index: 5,
                    levels: 1,
                },
                crate::engine::manager::card::QueuedCardRankChange {
                    card_index: 7,
                    levels: -1,
                },
            ]
        );
    }

    #[test]
    fn power_card_upgrade_spends_the_rank_cost_before_upgrading_one_owned_card() {
        use sonettobuf::{CardInfo, Fight, FightEntityInfo, FightTeam, PowerInfo};

        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3065),
                    entity_type: Some(1),
                    team_type: Some(1),
                    current_hp: Some(100),
                    skill_group1: vec![30650211, 30650212, 30650213],
                    skill_group2: vec![30650221, 30650222, 30650223],
                    power_infos: vec![PowerInfo {
                        power_id: Some(EUREKA_RESOURCE_ID),
                        num: Some(2),
                        max: Some(6),
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = crate::engine::manager::BattleManagers::seeded(&fight);
        managers
            .execute_card(CardCommand::Setup(
                crate::engine::manager::card::CardSetup {
                    hand: vec![
                        CardInfo {
                            uid: Some(10),
                            skill_id: Some(30650211),
                            ..Default::default()
                        },
                        CardInfo {
                            uid: Some(10),
                            skill_id: Some(30650221),
                            ..Default::default()
                        },
                    ],
                    draw_pile: Vec::new(),
                    deck_num: 2,
                },
            ))
            .unwrap();
        let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
        let behavior = ParsedBehavior::new(50034, "ConsumePowerUpgradeSkillCard", vec![2, 3]);
        let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
        determinism.enqueue_hand_rank_choices([
            crate::engine::runtime::determinism::HandRankChoice {
                opcode: 50034,
                owner_uid: 10,
                hand_index: 1,
            },
        ]);
        let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
        let mut target = crate::engine::skill::target::TargetContext::default();

        let ops = super::super::rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 30650243,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        )
        .unwrap();

        assert!(matches!(
            ops.as_slice(),
            [
                RuleOp::Command(BattleCommand::Eureka(EurekaCommand::Change(EurekaChange {
                    delta: -2,
                    ..
                }))),
                RuleOp::Command(BattleCommand::Card(CardCommand::RankUpHand(
                    HandCardRankUp {
                        owner_uid: 10,
                        hand_index: 1,
                        ..
                    }
                )))
            ]
        ));
    }

    #[test]
    fn butterfly_records_basic_incantations_by_effect_tag() {
        crate::test_support::init_config();
        let feature = crate::engine::manager::buff::ActiveBuffFeature {
            owner_uid: 10,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 235002,
            amount: 1,
            team_type: 1,
            owner_alive: true,
            act_type: "ButterflyRecordSkill".to_owned(),
            effect_time: 105,
            effect_condition: 0,
            raw: "1104#3#10010#1,2,3,4,5,6,9,13".to_owned(),
            values: vec![1104, 3, 10010, 1, 2, 3, 4, 5, 6, 9, 13],
        };

        assert_eq!(
            crate::engine::skill::effect::catalog::configured_skill_type(31390111),
            0
        );
        assert_eq!(
            crate::engine::skill::effect::catalog::configured_effect_tag(31390111),
            3
        );
        assert!(allows_recorded_skill(&feature, 31390111));
        assert!(allows_recorded_skill(&feature, 31390121));
        assert!(!allows_recorded_skill(&feature, 31390131));
    }

    #[test]
    fn butterfly_records_the_queue_validated_skill_after_hand_removal() {
        use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(235002),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = crate::engine::manager::BattleManagers::seeded(&fight);
        let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
        let behavior = ParsedBehavior::from_spec(
            crate::engine::skill::behavior::classify::BehaviorSpec::new(
                60283,
                "BufferflyRecordSkill",
            ),
            Vec::new(),
            Vec::new(),
        );
        let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
        let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
        let mut target = crate::engine::skill::target::TargetContext {
            recorded_skill_id: 31390111,
            recorded_skill_source_uid: 10,
            ..Default::default()
        };

        let ops = Handler::emit_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 31390131,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        )
        .unwrap();

        assert!(matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::Buff(BuffCommand::SetState(state)))]
                if state.act_info.as_ref().and_then(|info| info.first())
                    .and_then(|info| info.str_param.as_deref()) == Some("3,31390111")
        ));
    }
}
