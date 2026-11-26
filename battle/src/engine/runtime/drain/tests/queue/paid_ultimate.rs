use super::*;

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
                        model_id: Some(3092),
                        passive_skill: vec![30920001],
                        ex_skill: Some(30920151),
                        ex_point_max: Some(12),
                        ex_point: Some(points),
                        ..entity(10, 1, 1)
                    },
                    FightEntityInfo {
                        passive_skill: vec![434931],
                        ..entity(11, 1, 2)
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
            assert!(rest.iter().any(|item| matches!(item,
                FrameItem::Child(frame) if matches!(frame.owner, FrameOwner::Skill { skill_id: 434931, .. })
            )));
        }
    }
}
