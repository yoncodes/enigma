use super::*;

#[test]
fn delegated_ultimate_uses_the_debit_that_actually_committed() {
    use crate::engine::manager::ex_point::ExPointKind;

    crate::test_support::init_config();
    let action = SkillRequest {
        source_uid: 10,
        skill_id: 30020131,
    };
    let delegating_skill = 311401511;
    let fixed_recipient_delegation = 100;
    let mut catalog = SkillEffectCatalog::from_roots(
        config::configs::get(),
        [action.skill_id, delegating_skill],
        [],
    );
    catalog.insert(ParsedSkillEffect {
        skill_id: fixed_recipient_delegation,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::new(60175, "DirectUseBigSkill", Vec::new()),
            TargetRequest::self_only(),
        )],
    });
    for (kind, blocked) in [
        (ExPointKind::Common, false),
        (ExPointKind::Common, true),
        (ExPointKind::Faith, false),
    ] {
        let fight = Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![
                    FightEntityInfo {
                        uid: Some(10),
                        team_type: Some(1),
                        current_hp: Some(10_000),
                        ex_skill: Some(action.skill_id),
                        ex_point: Some(7),
                        ex_point_max: Some(12),
                        ex_point_type: Some(kind.as_wire()),
                        buffs: blocked
                            .then_some(BuffInfo {
                                uid: Some(100),
                                buff_id: Some(5081),
                                from_uid: Some(10),
                                ..Default::default()
                            })
                            .into_iter()
                            .collect(),
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(11),
                        team_type: Some(1),
                        current_hp: Some(10_000),
                        ex_point: Some(0),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    team_type: Some(2),
                    current_hp: Some(10_000),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        let delegation = if kind == ExPointKind::Common {
            SkillRequest {
                source_uid: 11,
                skill_id: delegating_skill,
            }
        } else {
            SkillRequest {
                source_uid: 10,
                skill_id: fixed_recipient_delegation,
            }
        };
        let result = run(
            &mut managers,
            &TargetPool::from_fight(&fight),
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            [RuleOp::Skill(delegation.into())],
        )
        .unwrap();
        let paid = if !blocked && kind == ExPointKind::Common {
            7
        } else {
            0
        };
        assert_eq!(managers.ex_point.get(10), if blocked { 7 } else { 5 });
        assert_eq!(managers.ex_point.round_ultimate_spend(1), paid);
        let order = result
            .events
            .iter()
            .filter_map(|event| match event {
                BattleEvent::AllyAction(cast) if cast.skill_id == action.skill_id => {
                    assert_eq!(cast.moxie_spent, paid);
                    assert_eq!(cast.additional_moxie, if paid == 0 { 0 } else { 2 });
                    Some(0)
                }
                BattleEvent::ExPointChanged(change)
                    if change.target_uid == 10 && change.applied_delta != 0 =>
                {
                    Some(change.applied_delta)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(order, if blocked { vec![0] } else { vec![-7, 0, 5] });
    }
}

#[test]
fn round_end_paid_ultimate_spends_before_hitting_the_marked_enemy() {
    use crate::engine::{event::kind::EventKind, runtime::record::FrameItem};

    crate::test_support::init_config();
    let entity = |uid, team, position| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team),
        position: Some(position),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [30920001, 434931], []);
    for points in [7, 10] {
        let fight = Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![
                    FightEntityInfo {
                        passive_skill: vec![434931],
                        ..entity(11, 1, 1)
                    },
                    FightEntityInfo {
                        model_id: Some(3092),
                        passive_skill: vec![30920001],
                        ex_skill: Some(30920151),
                        ex_point_max: Some(12),
                        ex_point: Some(points),
                        ..entity(10, 1, 2)
                    },
                ],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![
                    entity(-1, 2, 1),
                    FightEntityInfo {
                        buffs: vec![BuffInfo {
                            buff_id: Some(30920103),
                            uid: Some(100),
                            from_uid: Some(10),
                            duration: Some(1),
                            layer: Some(1),
                            ..Default::default()
                        }],
                        ..entity(-2, 2, 2)
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        let result = run_group_event(
            &mut managers,
            &TargetPool::from_fight(&fight),
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext {
                current_round: 2,
                ..Default::default()
            },
            BattleEvent::Kind(EventKind::RoundEnd),
            ReactionLane::Skills,
            Some(&[10]),
        )
        .unwrap();
        let cost = result.events.iter().position(|event| {
            matches!(event,
                BattleEvent::ExPointChanged(value) if value.applied_delta == -8
            )
        });
        let hit = result.events.iter().position(|event| {
            matches!(event,
                BattleEvent::Hit(value) if value.skill_id == 30920151 && value.target_uid == -2
            )
        });
        if points == 7 {
            assert_eq!((cost, hit, managers.ex_point.get(10)), (None, None, 7));
            assert_eq!(managers.ex_point.round_ultimate_spend(1), 0);
        } else {
            assert!(matches!((cost, hit), (Some(cost), Some(hit)) if cost < hit));
            assert_eq!(managers.ex_point.get(10), 2);
            assert_eq!(managers.ex_point.round_ultimate_spend(1), 8);
            let observer = result.events.iter().position(|event| matches!(event,
                BattleEvent::BuffAdded(change) if change.buff_id == 434921 && change.target_uid == 10
            ));
            assert!(matches!((cost, observer, hit),
                (Some(cost), Some(observer), Some(hit)) if cost < observer && observer < hit));
            assert_eq!(
                result
                    .events
                    .iter()
                    .filter(|event| matches!(event,
                        BattleEvent::AllyAction(action) if action.skill_id == 30920151
                            && action.target_uid == -2 && action.moxie_spent == 8
                    ))
                    .count(),
                1
            );
            let [FrameItem::Child(settlement)] = result.frames[0].items.as_slice() else {
                panic!("expected one round-end settlement event");
            };
            let is_cost = |item: &FrameItem| {
                matches!(item,
                    FrameItem::Change(change) if matches!(change.as_ref(),
                        BattleChange::ExPoint(crate::engine::manager::ex_point::ExPointChanges::Value { change, .. })
                            if change.target_uid == 10 && change.applied_delta == -8
                    )
                )
            };
            let [FrameItem::Child(passive), rest @ ..] = settlement.items.as_slice() else {
                panic!("expected the auto-cast passive before its debit");
            };
            let ultimate = passive
                .items
                .iter()
                .find_map(|item| match item {
                    FrameItem::Child(frame)
                        if matches!(
                            frame.owner,
                            FrameOwner::Skill {
                                skill_id: 30920151,
                                ..
                            }
                        ) =>
                    {
                        Some(frame)
                    }
                    _ => None,
                })
                .unwrap();
            assert!(!ultimate.items.iter().any(is_cost));
            assert!(rest.iter().any(is_cost));
            let reward = rest
                .iter()
                .find_map(|item| match item {
                    FrameItem::Child(frame)
                        if matches!(
                            frame.owner,
                            FrameOwner::Skill {
                                source_uid: 11,
                                skill_id: 434931,
                                target_uid: Some(11),
                                ..
                            }
                        ) =>
                    {
                        Some(frame)
                    }
                    _ => None,
                })
                .expect("the Moxie-spend subscriber owns the reaction");
            assert_eq!(
                reward
                    .items
                    .iter()
                    .filter(|item| matches!(item,
                        FrameItem::Child(frame) if matches!(frame.owner, FrameOwner::Skill {
                            source_uid: 11, skill_id: 434921, target_uid: Some(10), ..
                        })
                    ))
                    .count(),
                8
            );
            let payment = result
                .events
                .iter()
                .find_map(|event| match event {
                    BattleEvent::ExPointChanged(change) if change.applied_delta == -8 => {
                        Some(*change)
                    }
                    _ => None,
                })
                .unwrap();
            let common_spend = BattleEvent::ExPointChanged(payment);
            assert_eq!(common_spend.moxie_spender(), Some(10));
            let definition =
                crate::engine::skill::condition::registry::find_key(660008, "PerDecrExPoint")
                    .unwrap();
            let pool = TargetPool::from_fight(&fight);
            for (event, expected_target) in [
                (
                    BattleEvent::ExPointChanged(
                        crate::engine::event::payload::ExPointChangeEvent {
                            spent: false,
                            source_uid: -1,
                            ..payment
                        },
                    ),
                    Some(-1),
                ),
                (
                    BattleEvent::ExPointChanged(
                        crate::engine::event::payload::ExPointChangeEvent {
                            kind: crate::engine::manager::ex_point::ExPointKind::Synchronization,
                            ..payment
                        },
                    ),
                    Some(10),
                ),
            ] {
                assert_eq!(event.moxie_spender(), None);
                assert_eq!(
                    reaction_skill_target(
                        &pool,
                        &event,
                        11,
                        definition.reaction_frame_target,
                        None,
                    ),
                    expected_target,
                );
            }
        }
    }
}
