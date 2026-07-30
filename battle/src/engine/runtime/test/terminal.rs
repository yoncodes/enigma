use super::*;

#[test]
fn terminal_attacker_settlement_does_not_enter_defender_card_cleanup() {
    crate::test_support::init_config();
    let entity = |uid, hp, team_type| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(hp),
        ..Default::default()
    };
    let mut runtime = BattleRuntime::new(Fight {
        cur_round: Some(1),
        version: Some(6),
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 100, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 100, 2)],
            ..Default::default()
        }),
        ..Default::default()
    });
    runtime.managers.hp.lose(-1, 100, 10);

    let round = runtime
        .build_begin_round_from_schedule(&BeginRoundRequest::default())
        .unwrap();

    assert_eq!(round.is_finish, Some(true));
    assert_eq!(round.cur_round, Some(2));
    assert!(round.fight_step.iter().all(|step| {
        step.act_effect.iter().all(|effect| {
            effect.effect_type
                != Some(sonettobuf::effect_type_enum::EffectType::Removeentitycards as i32)
        })
    }));
}

#[test]
fn version_seven_terminal_round_does_not_advance_to_an_unplayed_round() {
    crate::test_support::init_config();
    let entity = |uid, hp, team_type| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(hp),
        ..Default::default()
    };
    let mut runtime = BattleRuntime::new(Fight {
        cur_round: Some(3),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 100, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 100, 2)],
            ..Default::default()
        }),
        ..Default::default()
    });
    runtime.managers.hp.lose(-1, 100, 10);

    let round = runtime
        .build_begin_round_from_schedule(&BeginRoundRequest::default())
        .unwrap();

    assert_eq!(round.is_finish, Some(true));
    assert_eq!(round.cur_round, Some(3));
    assert_eq!(runtime.fight.cur_round, Some(3));
}

#[test]
fn next_ai_snapshot_is_published_after_current_ai_settlement() {
    crate::test_support::init_config();
    let entity = |uid, team_type, skill_group1| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(100),
        skill_group1,
        attr: Some(sonettobuf::HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = BattleRuntime::new(Fight {
        cur_round: Some(1),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, Vec::new())],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, vec![40340111])],
            ..Default::default()
        }),
        ..Default::default()
    });
    let next = sonettobuf::CardInfo {
        uid: Some(-1),
        skill_id: Some(40340111),
        ..Default::default()
    };
    runtime.determinism.enqueue_next_ai_card_snapshot(vec![
        sonettobuf::CardInfo {
            uid: Some(-1),
            skill_id: Some(999),
            ..Default::default()
        },
        next.clone(),
    ]);

    let round = runtime
        .build_begin_round_from_schedule(&BeginRoundRequest::default())
        .unwrap();

    assert_eq!(runtime.managers.ex_point.get(-1), 0);
    assert_eq!(round.ai_use_cards.len(), 1);
    assert_eq!(round.ai_use_cards[0].uid, next.uid);
    assert_eq!(round.ai_use_cards[0].skill_id, next.skill_id);
}

#[test]
fn configured_wave_advances_before_the_next_round_cue() {
    crate::test_support::init_config();
    let (entitys, sub_entitys) =
        crate::engine::fight::defender::Defender::build_wave_entities(251401, 2, 2, 0).unwrap();
    let mut runtime = BattleRuntime::new(Fight {
        battle_id: Some(2514),
        episode_id: Some(20514),
        version: Some(6),
        cur_round: Some(1),
        cur_wave: Some(1),
        max_round: Some(20),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                position: Some(1),
                team_type: Some(1),
                current_hp: Some(100),
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys,
            sub_entitys,
            ..Default::default()
        }),
        ..Default::default()
    });
    runtime.catalog = SkillEffectCatalog::default();
    runtime.managers.card.set_deck_num(44);
    runtime.round_state.power = 31;
    runtime.managers.hp.lose(-1, i32::MAX, 10);
    runtime.managers.hp.lose(-2, i32::MAX, 10);

    let round = runtime
        .build_begin_round_from_schedule(&BeginRoundRequest::default())
        .unwrap();

    assert_eq!(runtime.fight.cur_wave, Some(2));
    assert_eq!(runtime.fight.cur_round, Some(2));
    assert!(!runtime.round_state.is_finish);
    assert_eq!(
        runtime
            .fight
            .defender
            .as_ref()
            .unwrap()
            .entitys
            .iter()
            .filter_map(|entity| entity.uid)
            .collect::<Vec<_>>(),
        vec![-3, -4]
    );
    assert!(
        runtime
            .managers
            .card
            .ai_queue()
            .iter()
            .filter_map(|card| card.skill_id)
            .all(|skill_id| runtime.catalog.get(skill_id).is_some())
    );
    let effect_types = round
        .fight_step
        .iter()
        .flat_map(|step| step.act_effect.iter())
        .filter_map(|effect| effect.effect_type)
        .collect::<Vec<_>>();
    let wave_snapshot = round
        .fight_step
        .iter()
        .flat_map(|step| step.act_effect.iter())
        .find(|effect| {
            effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Newchangewave as i32)
        })
        .and_then(|effect| effect.fight.as_ref())
        .and_then(|fight| fight.attacker.as_ref())
        .unwrap();
    assert_eq!(wave_snapshot.card_deck_size, Some(44));
    assert_eq!(wave_snapshot.power, Some(31));
    let wave = effect_types
        .iter()
        .position(|effect| {
            *effect == sonettobuf::effect_type_enum::EffectType::Newchangewave as i32
        })
        .unwrap();
    let round = effect_types
        .iter()
        .position(|effect| *effect == sonettobuf::effect_type_enum::EffectType::Changeround as i32)
        .unwrap();
    assert!(wave < round);
}

#[test]
fn wave_clear_defers_card_refill_to_the_next_round_deal() {
    crate::test_support::init_config();
    let (entitys, sub_entitys) =
        crate::engine::fight::defender::Defender::build_wave_entities(251401, 2, 2, 0).unwrap();
    let card = |skill_id| sonettobuf::CardInfo {
        uid: Some(10),
        skill_id: Some(skill_id),
        temp_card: Some(false),
        energy: Some(0),
        ..Default::default()
    };
    let remaining = card(30230111);
    let dealt = vec![card(30230121), card(30230111)];
    let mut runtime = BattleRuntime::new(Fight {
        battle_id: Some(2514),
        version: Some(7),
        cur_round: Some(1),
        cur_wave: Some(1),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                position: Some(1),
                team_type: Some(1),
                current_hp: Some(100),
                skill_group1: vec![30230111, 30230112, 30230113],
                skill_group2: vec![30230121, 30230122, 30230123],
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys,
            sub_entitys,
            ..Default::default()
        }),
        ..Default::default()
    });
    runtime
        .managers
        .execute_card(crate::engine::manager::card::CardCommand::Setup(
            CardSetup {
                hand: vec![remaining.clone()],
                draw_pile: dealt.clone(),
                deck_num: dealt.len() as i32,
            },
        ))
        .unwrap();
    runtime.determinism.enqueue_card_draws(dealt.clone());
    runtime.managers.hp.lose(-1, i32::MAX, 10);
    runtime.managers.hp.lose(-2, i32::MAX, 10);

    let round = runtime
        .build_begin_round_from_schedule(&BeginRoundRequest::default())
        .unwrap();

    assert_eq!(round.before_cards1, vec![remaining]);
    assert_eq!(round.team_a_cards1, dealt);
    assert!(round.before_cards2.is_empty());
    assert!(round.team_a_cards2.is_empty());
    assert!(round.fight_step.iter().all(|step| {
        step.act_effect.iter().all(|effect| {
            effect.effect_type != Some(sonettobuf::effect_type_enum::EffectType::Dealcard2 as i32)
        })
    }));
}
