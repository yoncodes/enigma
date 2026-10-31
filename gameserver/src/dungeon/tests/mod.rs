use super::*;

fn runtime(fight: sonettobuf::Fight) -> battle::Battle {
    battle::tooling::unstarted_battle(
        battle::catalog::BattleCatalog::new(config::configs::get()),
        fight,
    )
    .unwrap()
}

fn act128_runtime_with_score(
    episode_id: i32,
    battle_id: i32,
    target_model_id: i32,
    cur_round: i32,
    target_hp: i32,
) -> ::battle::Battle {
    use ::battle::tooling::{
        into_battle,
        replay::{CardOpType, CardSetup, ReplayBattle},
    };
    use sonettobuf::{
        BeginRoundOper, BeginRoundRequest, CardInfo, Fight, FightEntityInfo, FightTeam,
    };

    let mut runtime = ReplayBattle::new(
        ::battle::catalog::BattleCatalog::new(config::configs::get()),
        Fight {
            episode_id: Some(episode_id),
            battle_id: Some(battle_id),
            version: Some(7),
            cur_round: Some(cur_round),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3023),
                    team_type: Some(1),
                    current_hp: Some(1_000_000),
                    skill_group1: vec![30230111],
                    attr: Some(sonettobuf::HeroAttribute {
                        hp: Some(1_000_000),
                        attack: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    model_id: Some(target_model_id),
                    team_type: Some(2),
                    current_hp: Some(target_hp),
                    attr: Some(sonettobuf::HeroAttribute {
                        hp: Some(target_hp),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        },
    )
    .unwrap();
    runtime
        .build_start_steps(CardSetup {
            hand: vec![CardInfo {
                uid: Some(10),
                skill_id: Some(30230111),
                temp_card: Some(false),
                ..Default::default()
            }],
            draw_pile: Vec::new(),
            deck_num: 0,
        })
        .unwrap();
    runtime
        .advance_round(BeginRoundRequest {
            opers: vec![BeginRoundOper {
                oper_type: Some(CardOpType::PlayCard.id()),
                param1: Some(1),
                to_id: Some(-1),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    assert!(runtime.activity_score() > 0);
    into_battle(runtime)
}

mod act128;
mod act229;
mod lifecycle;
mod settlement;
