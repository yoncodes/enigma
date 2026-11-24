use super::*;

pub(super) fn everecho_switch_runtime(hand: Vec<CardInfo>, act_point: i32) -> BattleRuntime {
    let mut runtime = team_runtime(
        vec![
            team_hero(10, 3145, 1_000, [31450111, 31450121]),
            team_hero(11, 3023, 500, [30230111, 30230121]),
        ],
        hand,
    );
    runtime.round_state.act_point = act_point;
    runtime
        .managers
        .conduit
        .execute(ConduitCommand::StopSkill {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60294, "StopDeviceSkill"),
            },
            source_uid: 10,
            team: 1,
            skill_id: 31450111,
        })
        .unwrap();
    runtime
}

pub(super) fn coppelia_conduit_runtime(
    ally: FightEntityInfo,
    hand: Vec<CardInfo>,
    act_point: i32,
) -> BattleRuntime {
    crate::test_support::init_config();
    let mut runtime = runtime(Fight {
        battle_id: Some(77),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3144),
                    team_type: Some(1),
                    position: Some(1),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                ally,
            ],
            device_area: Some(FightDeviceAreaInfo::default()),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                position: Some(1),
                current_hp: Some(100_000),
                attr: Some(HeroAttribute {
                    hp: Some(100_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    runtime
        .managers
        .conduit
        .execute(
            crate::engine::manager::conduit::ConduitCommand::SelectGroup {
                source_uid: 10,
                group: 2,
            },
        )
        .unwrap();
    grant_buff(&mut runtime, 10, 10, 31440111);
    grant_buff(&mut runtime, 10, 11, 31440111);
    runtime
        .managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand,
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    runtime.round_state.act_point = act_point;
    runtime
}

pub(super) fn twins_conduit_runtime(
    enemy_hp: i32,
    enemy_career: i32,
    cost_reduction: bool,
    ex_point: i32,
) -> BattleRuntime {
    twins_conduit_runtime_with_enemy_hps(&[enemy_hp], enemy_career, cost_reduction, ex_point)
}

pub(super) fn twins_conduit_runtime_with_enemy_hps(
    enemy_hps: &[i32],
    enemy_career: i32,
    cost_reduction: bool,
    ex_point: i32,
) -> BattleRuntime {
    crate::test_support::init_config();
    let twins = FightEntityInfo {
        uid: Some(10),
        model_id: Some(3149),
        team_type: Some(1),
        position: Some(1),
        current_hp: Some(1_000),
        ex_point: Some(ex_point),
        ex_point_type: Some(4),
        ex_point_max: Some(100),
        ex_skill: Some(0),
        attr: Some(HeroAttribute {
            hp: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = runtime(Fight {
        battle_id: Some(77),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![twins],
            device_area: Some(FightDeviceAreaInfo::default()),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: enemy_hps
                .iter()
                .enumerate()
                .map(|(index, hp)| FightEntityInfo {
                    uid: Some(-(index as i64) - 1),
                    team_type: Some(2),
                    position: Some(index as i32 + 1),
                    career: Some(enemy_career),
                    current_hp: Some(*hp),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    });
    grant_buff(&mut runtime, 10, 10, 31490006);
    grant_buff(&mut runtime, 10, 10, 31490007);
    if cost_reduction {
        grant_buff(&mut runtime, 10, 10, 31440113);
    }
    runtime
}

pub(super) fn regulus_conduit_runtime(enemy_hp: i32) -> BattleRuntime {
    crate::test_support::init_config();
    runtime(Fight {
        battle_id: Some(77),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3025),
                team_type: Some(1),
                position: Some(1),
                ex_skill_level: Some(3),
                destiny_stone: Some(302502),
                destiny_rank: Some(4),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            device_area: Some(FightDeviceAreaInfo::default()),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                position: Some(1),
                current_hp: Some(enemy_hp),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    })
}

pub(super) fn hedone_with_stored_energy() -> FightEntityInfo {
    let mut hedone = team_hero(10, 3156, 1_000, [31560111, 31560121]);
    hedone.buffs.push(sonettobuf::BuffInfo {
        uid: Some(20),
        buff_id: Some(31560185),
        from_uid: Some(10),
        act_info: vec![sonettobuf::BuffActInfo {
            act_id: Some(1159),
            param: vec![2, 10, 1],
            str_param: Some(String::new()),
        }],
        ..Default::default()
    });
    hedone
}

pub(super) fn hedone_energy_card() -> sonettobuf::CardInfo {
    sonettobuf::CardInfo {
        extra_infos: vec![sonettobuf::CardExtraInfo {
            key: Some(8),
            values: vec!["HedoneCard".to_owned()],
        }],
        ..hand_card(10, 31560181, false)
    }
}
