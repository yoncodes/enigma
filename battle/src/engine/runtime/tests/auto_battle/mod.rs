use super::*;
use crate::engine::{
    manager::{
        buff::{BuffChangeDuration, BuffCommand, BuffGrant, BuffSelector},
        card::{CardCommand, CardOpType},
        conduit::ConduitCommand,
    },
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};
use sonettobuf::{AutoRoundRequest, CardInfo, FightDeviceAreaInfo, HeroAttribute};

mod conduit;
mod dynamo_opening;
mod operations;
mod queue_setup;
mod readiness;
mod record_skill;
mod selection;
mod sovereign;
mod targeting;
mod team_cards;
mod tuning;

fn auto_runtime() -> BattleRuntime {
    crate::test_support::init_config();
    let fight = Fight {
        battle_id: Some(77),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3023),
                team_type: Some(1),
                position: Some(1),
                current_hp: Some(1_000),
                ex_point: Some(0),
                ex_skill: Some(30230131),
                skill_group1: vec![30230111],
                skill_group2: vec![30230121],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    team_type: Some(2),
                    position: Some(1),
                    current_hp: Some(1_000),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    team_type: Some(2),
                    position: Some(2),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let card = |skill_id, temp_card| CardInfo {
        uid: Some(10),
        skill_id: Some(skill_id),
        temp_card: Some(temp_card),
        ..Default::default()
    };
    let mut runtime = runtime(fight);
    runtime
        .managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![
                card(30230111, false),
                card(30230121, false),
                card(30230111, true),
            ],
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    runtime.extend_current_card_skill_roots();
    runtime.round_state.act_point = 1;
    runtime
}

fn team_hero(uid: i64, model_id: i32, hp: i32, skills: [i32; 2]) -> FightEntityInfo {
    FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        team_type: Some(1),
        position: Some(uid as i32 - 9),
        current_hp: Some(hp),
        attr: Some(HeroAttribute {
            hp: Some(1_000),
            ..Default::default()
        }),
        ex_point: Some(0),
        skill_group1: vec![skills[0]],
        skill_group2: vec![skills[1]],
        ..Default::default()
    }
}

fn support_runtime(healer_hp: i32, hand: Vec<CardInfo>) -> BattleRuntime {
    team_runtime(
        vec![
            team_hero(10, 3023, 1_000, [30230111, 30230121]),
            team_hero(11, 3082, healer_hp, [30820111, 30820121]),
        ],
        hand,
    )
}

fn team_runtime(team: Vec<FightEntityInfo>, hand: Vec<CardInfo>) -> BattleRuntime {
    team_runtime_against(team, vec![enemy(-1, 1_000)], hand)
}

fn enemy(uid: i64, hp: i32) -> FightEntityInfo {
    FightEntityInfo {
        uid: Some(uid),
        team_type: Some(2),
        position: Some(-uid as i32),
        current_hp: Some(hp),
        ..Default::default()
    }
}

fn team_runtime_against(
    team: Vec<FightEntityInfo>,
    enemies: Vec<FightEntityInfo>,
    hand: Vec<CardInfo>,
) -> BattleRuntime {
    crate::test_support::init_config();
    let fight = Fight {
        battle_id: Some(77),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: team,
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: enemies,
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = runtime(fight);
    runtime
        .managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand,
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    runtime.extend_current_card_skill_roots();
    runtime.round_state.act_point = 1;
    runtime
}

fn grant_buff(runtime: &mut BattleRuntime, source_uid: i64, target_uid: i64, buff_id: i32) {
    runtime
        .managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid,
            target_uid,
            buff_id,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();
}

fn change_buff_duration(runtime: &mut BattleRuntime, target_uid: i64, buff_id: i32, delta: i32) {
    runtime
        .managers
        .execute_buff(BuffCommand::ChangeDuration(BuffChangeDuration {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            target_uid,
            selector: BuffSelector::ExactId(buff_id),
            delta,
        }))
        .unwrap();
}

fn hand_card(uid: i64, skill_id: i32, temp_card: bool) -> CardInfo {
    CardInfo {
        uid: Some(uid),
        skill_id: Some(skill_id),
        temp_card: Some(temp_card),
        ..Default::default()
    }
}
