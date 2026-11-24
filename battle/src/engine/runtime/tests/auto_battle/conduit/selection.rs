use super::*;

#[test]
fn auto_round_loads_visible_conduit_card_roots() {
    let runtime = team_runtime(
        vec![team_hero(10, 3144, 1_000, [31440111, 31440121])],
        vec![hand_card(10, 31446011, false)],
    );

    let reply = runtime
        .try_plan_auto_round(&AutoRoundRequest::default())
        .unwrap();

    assert_eq!(reply.opers[0].param1, Some(1));
}

#[test]
fn auto_round_selects_conduit_groups_from_round_utility() {
    crate::test_support::init_config();
    let hero = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        team_type: Some(1),
        position: Some(uid as i32 - 9),
        current_hp: Some(1_000),
        attr: Some(HeroAttribute {
            hp: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut regulus = hero(13, 3025);
    regulus.destiny_stone = Some(302502);
    regulus.destiny_rank = Some(4);
    let fight = Fight {
        battle_id: Some(119383110),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![hero(10, 3149), hero(11, 3144), hero(12, 3145), regulus],
            device_area: Some(FightDeviceAreaInfo::default()),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                position: Some(1),
                career: Some(1),
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    hp: Some(10_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = runtime(fight);
    runtime
        .managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: [
                (11, 31446012),
                (11, 31446011),
                (13, 31447002),
                (10, 31490201),
                (13, 31446021),
                (10, 31446012),
                (11, 31446011),
                (12, 31450181),
                (13, 31446021),
                (10, 31446022),
            ]
            .into_iter()
            .map(|(uid, skill_id)| hand_card(uid, skill_id, false))
            .collect(),
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    runtime.round_state.act_point = 5;

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(
        reply
            .devices_opers
            .iter()
            .map(|oper| (oper.uid, oper.index))
            .collect::<Vec<_>>(),
        vec![
            (Some(10), Some(2)),
            (Some(11), Some(2)),
            (Some(12), Some(1)),
            (Some(13), Some(1)),
        ]
    );
    assert_eq!(
        reply
            .opers
            .iter()
            .map(|oper| oper.param1)
            .collect::<Vec<_>>(),
        vec![Some(3), Some(9), Some(7), Some(4), Some(6)]
    );
}

#[test]
fn auto_round_multi_device_plan_executes_without_cross_feeding_switches() {
    let mut runtime = team_runtime(
        vec![
            team_hero(10, 3145, 1_000, [31450111, 31450121]),
            team_hero(11, 3149, 1_000, [31490111, 31490121]),
        ],
        vec![
            hand_card(10, 31450181, false),
            hand_card(11, 31446012, false),
        ],
    );
    runtime.round_state.act_point = 2;
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

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());
    let everecho_group = reply
        .devices_opers
        .iter()
        .find(|operation| operation.uid == Some(10))
        .and_then(|operation| operation.index);
    let twins_group = reply
        .devices_opers
        .iter()
        .find(|operation| operation.uid == Some(11))
        .and_then(|operation| operation.index);
    assert_eq!(everecho_group, Some(1));
    for operation in &reply.devices_opers {
        runtime
            .managers
            .conduit
            .execute(ConduitCommand::SelectGroup {
                source_uid: operation.uid.unwrap(),
                group: operation.index.unwrap(),
            })
            .unwrap();
    }
    let pool = crate::engine::skill::target::TargetPool::from_fight_with_catalog(
        runtime.catalog_data.unwrap(),
        &runtime.fight,
    );
    let mut catalog = runtime.catalog.clone();
    runtime.managers.catalog().extend_skill_roots(
        &mut catalog,
        [31450181, 31446012],
        std::iter::empty(),
    );
    let mut determinism = runtime.determinism.clone();
    crate::engine::runtime::schedule::run_player_commands(
        &mut runtime.managers,
        &pool,
        &catalog,
        &mut determinism,
        crate::engine::skill::target::TargetContext::default(),
        reply
            .opers
            .iter()
            .filter_map(crate::engine::round::command::RoundCommand::from_oper),
        1,
        crate::engine::manager::emitter::UID,
    )
    .unwrap();

    assert_eq!(runtime.managers.conduit.selected_group(10), Some(2));
    assert_eq!(runtime.managers.conduit.selected_group(11), twins_group);
}

#[test]
fn auto_round_selects_conduit_healing_only_for_urgent_hp() {
    for (ally_hp, expected_group) in [(700, 1), (500, 2)] {
        let mut runtime = team_runtime(
            vec![
                team_hero(10, 3145, 1_000, [31450111, 31450121]),
                team_hero(11, 3023, ally_hp, [30230111, 30230121]),
            ],
            vec![hand_card(10, 31446062, false)],
        );
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
        grant_buff(&mut runtime, 10, 10, 31450121);

        let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

        assert_eq!(
            reply.devices_opers[0].index,
            Some(expected_group),
            "ally hp {ally_hp}"
        );
    }
}

#[test]
fn auto_round_keeps_a_ready_conduit_attack_while_its_setup_buff_is_expiring() {
    let mut runtime = coppelia_conduit_runtime(
        team_hero(11, 3023, 1_000, [30230111, 30230121]),
        vec![
            hand_card(10, 31446011, false),
            hand_card(10, 31446011, false),
        ],
        2,
    );
    for target_uid in [10, 11] {
        change_buff_duration(&mut runtime, target_uid, 31440111, -2);
    }

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.devices_opers[0].index, Some(1));
}

#[test]
fn auto_round_refreshes_an_expiring_buff_with_an_ordinary_card() {
    let mut runtime = coppelia_conduit_runtime(
        team_hero(11, 3023, 1_000, [30230111, 30230121]),
        vec![
            hand_card(11, 30230111, false),
            hand_card(10, 31440121, false),
            hand_card(11, 30230111, false),
        ],
        2,
    );
    for target_uid in [10, 11] {
        change_buff_duration(&mut runtime, target_uid, 31440111, -2);
    }

    let reply = runtime.plan_auto_round(&AutoRoundRequest {
        opers: vec![BeginRoundOper {
            oper_type: Some(CardOpType::PlayCard.id()),
            param1: Some(1),
            to_id: Some(-1),
            ..Default::default()
        }],
        to_id: Some(-1),
    });

    assert_eq!(reply.opers[0].param1, Some(1));
}

#[test]
fn auto_round_conduit_groups_honor_reduced_costs() {
    let reduced =
        twins_conduit_runtime(1_000, 1, true, 0).plan_auto_round(&AutoRoundRequest::default());
    assert_eq!(reduced.devices_opers[0].index, Some(2));
}

#[test]
fn auto_round_selects_ready_conduit_unique_without_a_character_ultimate() {
    let reply =
        twins_conduit_runtime(1_000, 1, false, 100).plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.devices_opers[0].index, Some(3));
}

#[test]
fn auto_round_does_not_rank_a_self_setup_buff_as_ally_support() {
    let reply =
        twins_conduit_runtime(1_000, 0, false, 70).plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.devices_opers[0].index, Some(1));
}

#[test]
fn auto_round_keeps_the_current_conduit_group_without_a_legal_target() {
    let reply = regulus_conduit_runtime(0).plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.devices_opers[0].index, Some(1));
}
