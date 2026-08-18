use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    event::kind::EventKind,
    manager::card::{CardCommand, HandCardRankUp},
    skill::rule::output::{BattleCommand, RuleOp},
};

use super::registry::BuffActKind;

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [mode] if matches!(*mode, 1 | 2 | 9))
}

pub fn rule_ops(context: &mut super::registry::RuntimeContext<'_>) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(context.subscriber, BuffActKind::CardLevelAdd) {
        return None;
    }
    let [mode] = context.subscriber.args.as_slice() else {
        return None;
    };
    if !supports(context.subscriber.args.as_slice()) {
        return None;
    }
    if context.event?.kind() != EventKind::RoundStart {
        return Some(Vec::new());
    }

    let mut candidates = context
        .managers
        .card
        .hand_rank_up_candidates(context.subscriber.owner_uid);
    if candidates.is_empty() {
        return Some(Vec::new());
    }

    let count = match *mode {
        1 => 1,
        2 => 2,
        9 => candidates.len(),
        _ => unreachable!("CardLevelAdd arguments were validated above"),
    };
    let origin = super::command_origin(context.subscriber)?;
    let mut ops = Vec::with_capacity(count.saturating_mul(2));

    for _ in 0..count {
        let hand_index = context
            .determinism
            .take_hand_rank_choice(
                context.subscriber.key.definition.opcode,
                context.subscriber.owner_uid,
                &candidates,
            )
            .or_else(|| {
                context
                    .determinism
                    .lua_random_index(candidates.len())
                    .map(|chosen| candidates[chosen])
            })?;
        let chosen = candidates
            .iter()
            .position(|candidate| *candidate == hand_index)?;

        ops.push(RuleOp::BuffFeatureMarker {
            target_uid: context.subscriber.owner_uid,
            effect_type: EffectType::Cardleveladd as i32,
            effect_num: context.subscriber.buff_id,
            buff_act_id: context.subscriber.key.definition.opcode,
        });
        ops.push(RuleOp::Command(BattleCommand::Card(
            CardCommand::RankUpHand(HandCardRankUp {
                origin,
                owner_uid: context.subscriber.owner_uid,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::event::payload::BattleEvent;
    use crate::engine::{
        event::subscription::SubscriptionKey,
        manager::BattleManagers,
        runtime::determinism::{HandRankChoice, RoundDeterminism},
        skill::{
            effect::SkillEffectCatalog, rule::DefinitionKey, subscriber::BuffActSubscriber,
            target::TargetPool,
        },
    };
    use sonettobuf::{CardInfo, Fight, FightEntityInfo, FightTeam};

    fn fight() -> Fight {
        Fight {
            attacker: Some(FightTeam {
                entitys: vec![
                    FightEntityInfo {
                        uid: Some(10),
                        team_type: Some(1),
                        current_hp: Some(100),
                        skill_group1: vec![30650211, 30650212, 30650213],
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(20),
                        team_type: Some(1),
                        current_hp: Some(100),
                        skill_group1: vec![30650221, 30650222, 30650223],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn card(uid: i64, skill_id: i32, temp_card: bool) -> CardInfo {
        CardInfo {
            uid: Some(uid),
            skill_id: Some(skill_id),
            temp_card: Some(temp_card),
            ..Default::default()
        }
    }

    fn subscriber(owner_uid: i64, buff_id: i32, mode: i32) -> BuffActSubscriber {
        BuffActSubscriber {
            owner_uid,
            source_uid: owner_uid,
            buff_uid: i64::from(buff_id),
            buff_id,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::RoundStart,
                DefinitionKey::new(701, "CardLevelAdd"),
            ),
            act_type: "CardLevelAdd".to_owned(),
            effect_time: 102,
            effect_condition: 0,
            args: vec![mode],
            raw: format!("701#{mode}"),
        }
    }

    fn context<'a>(
        managers: &'a BattleManagers,
        pool: &'a TargetPool,
        catalog: &'a SkillEffectCatalog,
        determinism: &'a mut RoundDeterminism,
        subscriber: &'a BuffActSubscriber,
        event: &'a BattleEvent,
    ) -> super::super::registry::RuntimeContext<'a> {
        super::super::registry::RuntimeContext {
            managers,
            pool,
            catalog,
            determinism,
            subscriber,
            event: Some(event),
        }
    }

    fn setup_managers(hand: Vec<CardInfo>) -> (Fight, BattleManagers) {
        crate::test_support::init_config();
        let fight = fight();
        let mut managers = BattleManagers::seeded(&fight);
        managers
            .execute_card(CardCommand::Setup(
                crate::engine::manager::card::CardSetup {
                    hand,
                    draw_pile: Vec::new(),
                    deck_num: 3,
                },
            ))
            .unwrap();
        (fight, managers)
    }

    fn commands(ops: Vec<RuleOp>) -> Vec<(usize, i32)> {
        ops.chunks_exact(2)
            .map(|pair| {
                let RuleOp::BuffFeatureMarker { .. } = pair[0] else {
                    panic!("each card change starts with a marker")
                };
                let RuleOp::Command(BattleCommand::Card(CardCommand::RankUpHand(change))) =
                    &pair[1]
                else {
                    panic!("each marker is followed by a hand rank command")
                };
                (change.hand_index, change.owner_uid as i32)
            })
            .collect()
    }

    #[test]
    fn mode_one_uses_only_the_subscriber_owner_and_cards_with_a_next_rank() {
        let (fight, mut managers) = setup_managers(vec![
            card(10, 30650213, false),
            card(10, 30650211, false),
            card(20, 30650221, false),
            card(10, 30650211, true),
            card(10, 999, false),
        ]);
        let pool = TargetPool::from_fight(&fight);
        let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
        let subscriber = subscriber(10, 5021, 1);
        let event = BattleEvent::RoundStart;
        let mut determinism = RoundDeterminism::default();
        let ops = rule_ops(&mut context(
            &managers,
            &pool,
            &catalog,
            &mut determinism,
            &subscriber,
            &event,
        ))
        .unwrap();

        assert_eq!(commands(ops.clone()), vec![(1, 10)]);
        assert!(matches!(
            ops[0],
            RuleOp::BuffFeatureMarker {
                target_uid: 10,
                effect_type,
                effect_num: 5021,
                buff_act_id: 701,
            } if effect_type == EffectType::Cardleveladd as i32
        ));
        let RuleOp::Command(BattleCommand::Card(command)) = ops.into_iter().nth(1).unwrap() else {
            panic!("expected a card command")
        };
        managers.execute_card(command).unwrap();
        assert_eq!(managers.card.hand()[0].skill_id, Some(30650213));
        assert_eq!(managers.card.hand()[1].skill_id, Some(30650212));
        assert_eq!(managers.card.hand()[2].skill_id, Some(30650221));
        assert_eq!(managers.card.hand()[3].skill_id, Some(30650211));
        assert_eq!(managers.card.hand()[4].skill_id, Some(999));
    }

    #[test]
    fn mode_two_consumes_scripted_choices_without_replacement() {
        let (fight, _managers) = setup_managers(vec![
            card(10, 30650211, false),
            card(10, 30650212, false),
            card(10, 30650213, false),
        ]);
        let pool = TargetPool::from_fight(&fight);
        let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
        let subscriber = subscriber(10, 5024, 2);
        let event = BattleEvent::RoundStart;
        let mut determinism = RoundDeterminism::default();
        determinism.enqueue_hand_rank_choices([
            HandRankChoice {
                opcode: 701,
                owner_uid: 10,
                hand_index: 1,
            },
            HandRankChoice {
                opcode: 701,
                owner_uid: 10,
                hand_index: 0,
            },
        ]);
        let ops = rule_ops(&mut context(
            &_managers,
            &pool,
            &catalog,
            &mut determinism,
            &subscriber,
            &event,
        ))
        .unwrap();

        assert_eq!(commands(ops), vec![(1, 10), (0, 10)]);
    }

    #[test]
    fn mode_nine_upgrades_all_eligible_cards_and_zero_candidates_are_a_noop() {
        let (fight, managers) = setup_managers(vec![
            card(10, 30650211, false),
            card(10, 30650212, false),
            card(10, 30650213, false),
            card(20, 30650221, false),
        ]);
        let pool = TargetPool::from_fight(&fight);
        let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
        let event = BattleEvent::RoundStart;
        let mut determinism = RoundDeterminism::default();
        let owner = subscriber(10, 5027, 9);
        let ops = rule_ops(&mut context(
            &managers,
            &pool,
            &catalog,
            &mut determinism,
            &owner,
            &event,
        ))
        .unwrap();
        assert_eq!(ops.len(), 4);
        let selected = commands(ops)
            .into_iter()
            .map(|(hand_index, _)| hand_index)
            .collect::<Vec<_>>();
        let mut sorted = selected.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, vec![0, 1]);
        assert_eq!(selected.len(), 2);

        let no_candidate = subscriber(10, 5027, 1);
        let mut managers = managers;
        managers
            .execute_card(CardCommand::Setup(
                crate::engine::manager::card::CardSetup {
                    hand: vec![card(10, 30650213, false)],
                    draw_pile: Vec::new(),
                    deck_num: 1,
                },
            ))
            .unwrap();
        let mut determinism = RoundDeterminism::default();
        assert_eq!(
            rule_ops(&mut context(
                &managers,
                &pool,
                &catalog,
                &mut determinism,
                &no_candidate,
                &event,
            )),
            Some(Vec::new())
        );
    }
}
