use super::*;
use sonettobuf::HeroAttribute;

#[test]
fn tts13_configured_split_then_final_victory() {
    crate::test_support::init_config();
    let (entitys, sub_entitys) =
        crate::engine::fight::defender::Defender::build_wave_entities(119_381_070, 4, 2, 0)
            .unwrap();
    assert_eq!(entitys.len(), 1);
    assert_eq!(entitys[0].model_id, Some(119_381_070));
    assert!(entitys[0].passive_skill.contains(&119_381_171));
    let mut runtime = runtime(Fight {
        episode_id: Some(3_810_113),
        battle_id: Some(119_381_070),
        version: Some(7),
        cur_round: Some(1),
        cur_wave: Some(1),
        max_round: Some(20),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3023),
                team_type: Some(1),
                position: Some(1),
                current_hp: Some(10_000_000),
                skill_group1: vec![30_230_111],
                attr: Some(HeroAttribute {
                    hp: Some(10_000_000),
                    attack: Some(100_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam { entitys, sub_entitys, ..Default::default() }),
        ..Default::default()
    });
    let card = sonettobuf::CardInfo {
        uid: Some(10),
        skill_id: Some(30_230_111),
        temp_card: Some(false),
        ..Default::default()
    };
    runtime.build_start_steps(CardSetup {
        hand: vec![card.clone(); 3],
        draw_pile: vec![card; 12],
        deck_num: 12,
    }).unwrap();
    assert!(runtime.managers.buff.has_buff_id(-1, 119_381_171));
    let play = |target| BeginRoundRequest {
        opers: vec![BeginRoundOper {
            oper_type: Some(crate::engine::manager::card::CardOpType::PlayCard.id()),
            param1: Some(1),
            to_id: Some(target),
            ..Default::default()
        }],
        ..Default::default()
    };

    let split = runtime.build_begin_round_from_schedule(&play(-1)).unwrap();
    let stones = runtime.managers.entity.ordered_uids()
        .filter(|uid| runtime.managers.entity.model_id(*uid) == Some(119_381_060))
        .filter(|uid| runtime.managers.hp.current(*uid) > 0)
        .collect::<Vec<_>>();
    assert_eq!(runtime.managers.hp.current(-1), 0, "the original boss must transform");
    assert_eq!(stones.len(), 2, "the configured boss must summon two stones");
    assert_eq!(split.is_finish, Some(false));
    assert_eq!(runtime.managers.terminal_outcome(), None);
    assert_eq!(runtime.outcome(), crate::engine::round::outcome::BattleOutcome::Unfinished);

    let first_stone = runtime.build_begin_round_from_schedule(&play(stones[0])).unwrap();
    assert_eq!(runtime.managers.hp.current(stones[0]), 0);
    assert!(runtime.managers.hp.current(stones[1]) > 0);
    assert_eq!(first_stone.is_finish, Some(false));
    assert_eq!(runtime.outcome(), crate::engine::round::outcome::BattleOutcome::Unfinished);

    let last_stone = runtime.build_begin_round_from_schedule(&play(stones[1])).unwrap();
    assert_eq!(runtime.managers.hp.current(stones[1]), 0);
    assert_eq!(last_stone.is_finish, Some(true));
    assert_eq!(runtime.outcome(), crate::engine::round::outcome::BattleOutcome::Victory);
}
