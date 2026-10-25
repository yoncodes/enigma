use super::*;

#[test]
fn next_round_snapshot_keeps_card_not_cal_size_ultimate() {
    crate::test_support::init_config();
    let ultimate = 31390131;
    let fight = Fight {
        cur_round: Some(1),
        version: Some(6),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3139),
                position: Some(1),
                current_hp: Some(100),
                ex_point: Some(5),
                ex_skill: Some(ultimate),
                buffs: vec![sonettobuf::BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31390181),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let card = |skill_id| CardInfo {
        uid: Some(10),
        skill_id: Some(skill_id),
        ..Default::default()
    };
    let mut runtime = runtime(fight);
    runtime
        .managers
        .execute_card(crate::engine::manager::card::CardCommand::Setup(
            CardSetup {
                hand: vec![card(100), card(200), card(300)],
                draw_pile: Vec::new(),
                deck_num: 0,
            },
        ))
        .unwrap();

    let first = runtime
        .build_begin_round_from_schedule(&BeginRoundRequest::default())
        .unwrap();
    assert!(
        first
            .team_a_cards1
            .iter()
            .any(|card| { card.uid == Some(10) && card.skill_id == Some(ultimate) })
    );
    assert!(
        runtime
            .managers
            .card
            .team_cards()
            .iter()
            .any(|card| { card.uid == Some(10) && card.skill_id == Some(ultimate) })
    );
    assert!(first.next_round_begin_step.iter().any(|step| {
        step.act_effect.iter().any(|effect| {
            effect
                .card_info_list
                .iter()
                .any(|card| card.uid == Some(10) && card.skill_id == Some(ultimate))
        })
    }));

    let second = runtime
        .build_begin_round_from_schedule(&BeginRoundRequest::default())
        .unwrap();
    assert!(
        second
            .team_a_cards1
            .iter()
            .any(|card| { card.uid == Some(10) && card.skill_id == Some(ultimate) })
    );
    assert!(
        second
            .before_cards1
            .iter()
            .all(|card| card.skill_id != Some(ultimate))
    );
    assert!(second.next_round_begin_step.iter().any(|step| {
        step.act_effect.iter().any(|effect| {
            effect
                .card_info_list
                .iter()
                .filter(|card| card.uid == Some(10) && card.skill_id == Some(ultimate))
                .count()
                == 1
        })
    }));
}

#[test]
fn round_start_generated_card_is_committed_after_the_before_cards_snapshot() {
    crate::test_support::init_config();
    let generated_skill = 312451035;
    let fight = Fight {
        cur_round: Some(1),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3124),
                current_hp: Some(100),
                ex_point: Some(10),
                ex_point_type: Some(3),
                buffs: vec![sonettobuf::BuffInfo {
                    uid: Some(20),
                    buff_id: Some(312451407),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let card = |skill_id| CardInfo {
        uid: Some(10),
        skill_id: Some(skill_id),
        ..Default::default()
    };
    let mut runtime = runtime(fight);
    runtime
        .managers
        .execute_card(crate::engine::manager::card::CardCommand::Setup(
            CardSetup {
                hand: vec![card(100), card(200), card(300)],
                draw_pile: Vec::new(),
                deck_num: 0,
            },
        ))
        .unwrap();

    let round = runtime
        .build_begin_round_from_schedule(&BeginRoundRequest::default())
        .unwrap();

    assert!(
        round
            .before_cards1
            .iter()
            .all(|card| card.skill_id != Some(generated_skill))
    );
    assert!(
        runtime
            .managers
            .card
            .hand()
            .iter()
            .any(|card| card.skill_id == Some(generated_skill))
    );
    assert!(round.next_round_begin_step.iter().any(|step| {
        step.act_effect.iter().any(|effect| {
            effect
                .card_info_list
                .iter()
                .any(|card| card.skill_id == Some(generated_skill))
                || effect.fight_step.as_ref().is_some_and(|nested| {
                    nested.act_effect.iter().any(|effect| {
                        effect
                            .card_info_list
                            .iter()
                            .any(|card| card.skill_id == Some(generated_skill))
                    })
                })
        })
    }));
}

#[test]
fn lorentz_team_ultimate_spends_beryl_moxie_and_is_not_regenerated() {
    crate::test_support::init_config();
    let entity =
        |uid, model_id, ex_point, ex_skill, passive_skill, skill1, skill2| FightEntityInfo {
            uid: Some(uid),
            model_id: Some(model_id),
            position: Some(uid as i32),
            team_type: Some(1),
            current_hp: Some(100_000),
            attr: Some(sonettobuf::HeroAttribute {
                hp: Some(100_000),
                attack: Some(1_000),
                ..Default::default()
            }),
            ex_point: Some(ex_point),
            ex_skill: Some(ex_skill),
            passive_skill,
            skill_group1: skill1,
            skill_group2: skill2,
            ..Default::default()
        };
    let passives = |hero_id, destiny| {
        crate::engine::entity::passive::Passive::for_config(hero_id, None, destiny)
            .into_iter()
            .map(|passive| passive.skill_id)
            .collect::<Vec<_>>()
    };
    let mut isolde = entity(
        10,
        3081,
        0,
        30810131,
        passives(3081, Some((308101, 4))),
        vec![30810111, 30810112, 30810113],
        vec![30810121, 30810122, 30810123],
    );
    isolde.destiny_stone = Some(308101);
    isolde.destiny_rank = Some(4);
    let fight = Fight {
        battle_id: Some(6),
        version: Some(6),
        attacker: Some(FightTeam {
            entitys: vec![
                isolde,
                entity(
                    11,
                    3127,
                    0,
                    31270131,
                    passives(3127, None),
                    vec![31270111, 31270112, 31270113],
                    vec![31270121, 31270122, 31270123],
                ),
                entity(
                    12,
                    3134,
                    5,
                    31345131,
                    passives(3134, None),
                    vec![31345111, 31345112, 31345113],
                    vec![31344121, 31344122, 31344123],
                ),
                entity(
                    13,
                    3139,
                    0,
                    31390131,
                    passives(3139, None),
                    vec![31390111, 31390112, 31390113],
                    vec![31390121, 31390122, 31390123],
                ),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(1000),
                team_type: Some(2),
                current_hp: Some(1_000_000),
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(1_000_000),
                    defense: Some(1_000),
                    mdefense: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let normal_cards = [
        (10, 30810111),
        (11, 31270111),
        (12, 31345111),
        (13, 31390111),
        (10, 30810121),
        (11, 31270121),
        (12, 31344121),
        (13, 31390121),
        (10, 30810111),
    ]
    .into_iter()
    .map(|(uid, skill_id)| CardInfo {
        uid: Some(uid),
        skill_id: Some(skill_id),
        ..Default::default()
    })
    .collect::<Vec<_>>();
    let mut determinism = RoundDeterminism::with_seed(6);
    determinism.enqueue_start_decks(Vec::new(), normal_cards.clone());
    determinism.enqueue_card_draws(normal_cards);
    let mut runtime = runtime(fight);
    runtime.start_round_with_determinism(determinism).unwrap();
    assert!(
        runtime
            .use_cloth_skill(UseClothSkillRequest {
                from_id: Some(12),
                to_id: Some(220),
                r#type: Some(ClothSkillType::SelectCrystal as i32),
                ..Default::default()
            })
            .is_some()
    );
    runtime
        .build_begin_round_from_schedule(&BeginRoundRequest::default())
        .unwrap();
    let beryl_precast_index = runtime
        .managers
        .card
        .hand()
        .iter()
        .position(|card| card.uid == Some(12) && card.temp_card.unwrap_or_default())
        .unwrap();
    let beryl_ultimate_index_after_precast = runtime.managers.card.hand().len() - 1
        + runtime
            .managers
            .card
            .team_cards()
            .iter()
            .position(|card| card.uid == Some(12) && card.skill_id == Some(31345131))
            .unwrap();

    let round = runtime
        .build_begin_round_from_schedule(&BeginRoundRequest {
            opers: vec![
                BeginRoundOper {
                    oper_type: Some(2),
                    param1: Some(beryl_precast_index as i32 + 1),
                    to_id: Some(-1),
                    ..Default::default()
                },
                BeginRoundOper {
                    oper_type: Some(2),
                    param1: Some(beryl_ultimate_index_after_precast as i32 + 1),
                    to_id: Some(-1),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .unwrap();

    // Beryl's eligibility marker is removed when she casts her Ultimate, so
    // Lorentz's team passive does not refund her spent Moxie. Lorentz did not
    // cast an Ultimate, so her +1 team grant and additional +2 self grant both
    // apply after the +2 she gained on entering battle.
    assert_eq!(runtime.managers.ex_point.get(12), 0);
    assert_eq!(runtime.managers.ex_point.get(13), 5);
    assert!(
        runtime
            .managers
            .card
            .team_cards()
            .iter()
            .all(|card| { card.uid != Some(12) || card.skill_id != Some(31345131) })
    );
    assert!(round.fight_step.iter().any(|step| {
        step.act_effect.iter().any(|effect| {
            effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Expointchange as i32)
                && effect.target_id == Some(12)
                && effect.effect_num == Some(-5)
        })
    }));
}
