use super::*;
use sonettobuf::{AssistBossInfo, AssistBossSkillInfo, PowerInfo};

fn assist_boss_runtime(power: i32) -> BattleRuntime {
    let fight = Fight {
        battle_id: Some(77),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![team_hero(10, 3149, 1_000, [31490111, 31490121])],
            assist_boss: Some(FightEntityInfo {
                uid: Some(-100),
                team_type: Some(1),
                current_hp: Some(1_000),
                power_infos: vec![PowerInfo {
                    power_id: Some(4),
                    num: Some(power),
                    max: Some(5),
                }],
                ..Default::default()
            }),
            assist_boss_info: Some(AssistBossInfo {
                skills: vec![AssistBossSkillInfo {
                    skill_id: Some(104335001),
                    need_power: Some(5),
                    power_low: Some(0),
                    power_high: Some(5),
                }],
                curr_cd: Some(0),
                round_use_limit: Some(1),
                ..Default::default()
            }),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![enemy(-1, 1_000)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = runtime(fight);
    runtime
        .managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![hand_card(10, 31446012, false)],
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    runtime.round_state.act_point = 1;
    runtime
}

#[test]
fn auto_round_uses_a_ready_configured_assist_boss_before_conduit_cards() {
    let mut runtime = assist_boss_runtime(5);

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers.len(), 2);
    assert_eq!(reply.opers[0].oper_type, Some(CardOpType::AssistBoss.id()));
    assert_eq!(reply.opers[0].param1, Some(104335001));
    assert_eq!(reply.opers[0].to_id, Some(-100));
    assert_eq!(reply.opers[1].oper_type, Some(CardOpType::PlayCard.id()));
    let continued = runtime.plan_auto_round(&AutoRoundRequest {
        opers: vec![reply.opers[0].clone()],
        ..Default::default()
    });
    assert_eq!(continued.opers.len(), 1);
    assert_eq!(
        continued.opers[0].oper_type,
        Some(CardOpType::PlayCard.id())
    );
    runtime
        .advance_round(BeginRoundRequest {
            opers: reply.opers,
            devices_opers: reply.devices_opers,
            auto_oper: Some(true),
        })
        .unwrap();
    assert_eq!(runtime.managers.eureka.get(-100, 4).current, 0);

    let unavailable = assist_boss_runtime(4).plan_auto_round(&AutoRoundRequest::default());
    assert!(
        unavailable
            .opers
            .iter()
            .all(|operation| operation.oper_type != Some(CardOpType::AssistBoss.id()))
    );
}

#[test]
fn prefix_targets_use_queue_admission_state() {
    let mut sonetto = team_hero(11, 3023, 1_000, [30230111, 30230121]);
    sonetto.attr.as_mut().unwrap().attack = Some(1_000);
    let mut runtime = team_runtime_against(
        vec![team_hero(10, 3145, 1_000, [31450111, 31450121]), sonetto],
        vec![enemy(-1, 1), enemy(-2, 100_000)],
        vec![
            hand_card(11, 30230111, false),
            hand_card(11, 30230111, false),
            hand_card(10, 31446062, false),
        ],
    );
    runtime.round_state.act_point = 3;
    let mut request = AutoRoundRequest {
        opers: vec![
            BeginRoundOper {
                oper_type: Some(CardOpType::PlayCard.id()),
                param1: Some(1),
                to_id: Some(-1),
                ..Default::default()
            };
            2
        ],
        ..Default::default()
    };

    let reply = runtime.plan_auto_round(&request);

    assert_eq!(reply.opers.len(), 1);
    assert_eq!(reply.opers[0].param1, Some(1));
    request.opers.extend(reply.opers);
    runtime
        .advance_round(BeginRoundRequest {
            opers: request.opers,
            devices_opers: reply.devices_opers,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(runtime.managers.hp.current(-1), 0);
    assert!(runtime.managers.hp.current(-2) > 0);
}

#[test]
fn auto_round_leaves_conduit_group_changes_to_queue_rules() {
    for enabled in [false, true] {
        let mut everecho = team_hero(10, 3145, 1_000, [31450111, 31450121]);
        everecho.passive_skill = vec![31450141];
        everecho.attr.as_mut().unwrap().attack = Some(100);
        let mut runtime = team_runtime(
            vec![everecho, team_hero(11, 3023, 500, [30230111, 30230121])],
            vec![hand_card(10, 31446062, false)],
        );
        if enabled {
            grant_buff(&mut runtime, 10, 10, 31450121);
        }

        let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

        assert_eq!(reply.devices_opers[0].index, Some(2));
        assert_eq!(runtime.managers.hp.current(11), 500);
        assert_eq!(runtime.managers.buff.has_buff_id(10, 31450121), enabled);
        runtime
            .advance_round(BeginRoundRequest {
                opers: reply.opers,
                devices_opers: reply.devices_opers,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(runtime.managers.hp.current(11) > 500, enabled);
        assert_eq!(runtime.managers.conduit.selected_group(10), Some(1));
        assert!(!runtime.managers.buff.has_buff_id(10, 31450121));
    }
}

#[test]
fn auto_round_skips_special_energy_for_an_unselected_group() {
    let mut runtime = team_runtime(
        vec![team_hero(10, 3149, 1_000, [31490111, 31490121])],
        vec![
            hand_card(10, 31490211, false),
            hand_card(10, 31490211, false),
        ],
    );
    runtime.round_state.act_point = 2;

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert!(reply.opers.is_empty());
}

fn hedone_with_stored_energy() -> FightEntityInfo {
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

fn hedone_energy_card() -> sonettobuf::CardInfo {
    sonettobuf::CardInfo {
        extra_infos: vec![sonettobuf::CardExtraInfo {
            key: Some(8),
            values: vec!["HedoneCard".to_owned()],
        }],
        ..hand_card(10, 31560181, false)
    }
}

#[test]
fn auto_round_spends_available_hedone_energy() {
    let free = |skill_id| sonettobuf::CardInfo {
        extra_infos: vec![sonettobuf::CardExtraInfo {
            key: Some(8),
            values: vec!["HedoneCard".to_owned()],
        }],
        ..hand_card(10, skill_id, false)
    };
    let runtime = team_runtime(
        vec![hedone_with_stored_energy()],
        vec![free(31446011), hedone_energy_card()],
    );

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers.len(), 2);
    assert_eq!(reply.opers[0].param1, Some(2));
    assert_eq!(reply.opers[1].param1, Some(1));
    assert!(
        reply
            .opers
            .iter()
            .all(|operation| operation.to_id == Some(-1))
    );
    assert_eq!(
        reply
            .devices_opers
            .iter()
            .map(|operation| (operation.uid, operation.index))
            .collect::<Vec<_>>(),
        [(Some(10), Some(1))]
    );
}

#[test]
fn auto_round_uses_stored_hedone_energy_for_a_selected_group() {
    let runtime = team_runtime_against(
        vec![hedone_with_stored_energy()],
        (1..=4).map(|index| enemy(-index, 1_000)).collect(),
        vec![hedone_energy_card()],
    );

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers.len(), 1);
    assert_eq!(reply.opers[0].param1, Some(1));
    assert_eq!(reply.devices_opers[0].index, Some(2));
}

#[test]
fn auto_round_uses_stored_hedone_energy_when_the_unique_is_selected() {
    let mut hedone = hedone_with_stored_energy();
    hedone.ex_point = Some(100);
    hedone.ex_point_type = Some(4);
    let mut runtime = team_runtime_against(
        vec![hedone],
        vec![enemy(-1, 1_000)],
        vec![hedone_energy_card()],
    );
    runtime
        .managers
        .conduit
        .execute(ConduitCommand::SelectGroup {
            source_uid: 10,
            group: 3,
        })
        .unwrap();
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60294, "StopDeviceSkill"),
    };
    for skill_id in [31560111, 31560121] {
        runtime
            .managers
            .conduit
            .execute(ConduitCommand::StopSkill {
                origin,
                source_uid: 10,
                team: 1,
                skill_id,
            })
            .unwrap();
    }

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.devices_opers[0].index, Some(3));
    assert_eq!(reply.opers.len(), 1);
    assert_eq!(reply.opers[0].param1, Some(1));
}

#[test]
fn auto_round_rejects_missing_hedone_energy_state() {
    let runtime = team_runtime_against(
        vec![team_hero(10, 3156, 1_000, [31560111, 31560121])],
        (1..=4).map(|index| enemy(-index, 1_000)).collect(),
        vec![hedone_energy_card()],
    );

    let error = runtime
        .try_plan_auto_round(&AutoRoundRequest::default())
        .unwrap_err();

    assert!(error.contains("ConduitEnergy(Hedone(InvalidCommand))"));
}

#[test]
fn auto_round_rejects_malformed_hedone_energy_state() {
    let mut hedone = hedone_with_stored_energy();
    hedone.buffs[0].act_info[0].param = vec![2, 9, 1];
    let runtime = team_runtime_against(
        vec![hedone],
        (1..=4).map(|index| enemy(-index, 1_000)).collect(),
        vec![hedone_energy_card()],
    );

    let error = runtime
        .try_plan_auto_round(&AutoRoundRequest::default())
        .unwrap_err();

    assert!(error.contains("ConduitEnergy(Hedone(State("));
    assert!(error.contains("InvalidState"));
}

#[test]
fn auto_round_reads_hedone_power_from_the_configured_carrier() {
    use crate::engine::{
        mechanic::hedone_device_power::available_gain,
        skill::{
            behavior::hedone_device_power::card_release,
            target::{TargetPool, TargetRequest},
        },
    };

    let caster = team_hero(10, 3156, 1_000, [31560111, 31560121]);
    let mut carrier = hedone_with_stored_energy();
    carrier.uid = Some(11);
    carrier.position = Some(2);
    carrier.buffs[0].from_uid = Some(11);
    let mut runtime = team_runtime_against(
        vec![caster, carrier],
        vec![enemy(-1, 100_000)],
        vec![hedone_energy_card()],
    );
    let mut effect = runtime.catalog.get(31560181).unwrap().clone();
    effect.slots[0].target = TargetRequest {
        code: 125,
        raw: Vec::new(),
    };
    effect.slots[0].behavior.args = vec![2];
    effect.slots[0].behavior.raw_args = vec!["2".to_owned()];
    runtime.catalog.insert(effect);
    let pool = TargetPool::from_fight_with_catalog(runtime.managers.catalog(), &runtime.fight);
    let command = card_release(&runtime.managers, &pool, &runtime.catalog, 10, 31560181)
        .unwrap()
        .unwrap();
    let gain = available_gain(&runtime.managers, command).unwrap().unwrap();
    assert_eq!((gain.source_uid, gain.power_id, gain.delta), (11, 2, 2));

    let reply = runtime
        .try_plan_auto_round(&AutoRoundRequest::default())
        .unwrap();

    assert_eq!(reply.opers.len(), 1);
    assert_eq!(reply.opers[0].param1, Some(1));
    assert_eq!(runtime.managers.conduit.power(1, 2), 0);
    let round = runtime
        .advance_round(BeginRoundRequest {
            opers: reply.opers,
            devices_opers: reply.devices_opers,
            ..Default::default()
        })
        .unwrap();
    let release = round
        .fight_step
        .iter()
        .find(|step| step.act_id == Some(31560181))
        .unwrap();
    assert!(release.act_effect.iter().any(|effect| {
        effect.effect_type
            == Some(sonettobuf::effect_type_enum::EffectType::Devicepowerchange as i32)
            && effect.reserve_str.as_deref() == Some("2#2")
    }));
}

#[test]
fn queued_hedone_card_releases_energy_when_the_round_executes() {
    let mut hedone = hedone_with_stored_energy();
    hedone.buffs[0].buff_id = Some(31560181);
    hedone.buffs[0].act_info[0].param = vec![2, 7, 1];
    let mut runtime = team_runtime(
        vec![hedone],
        vec![
            hand_card(10, 31446011, false),
            hedone_energy_card(),
            hand_card(10, 31446012, false),
        ],
    );
    runtime.round_state.act_point = 2;
    let play = BeginRoundOper {
        oper_type: Some(CardOpType::PlayCard.id()),
        param1: Some(1),
        to_id: Some(-1),
        ..Default::default()
    };
    let request = AutoRoundRequest {
        opers: vec![play.clone(), play.clone()],
        ..Default::default()
    };
    let reply = runtime.try_plan_auto_round(&request).unwrap();

    assert_eq!(reply.opers, [play]);
    assert_eq!(runtime.managers.conduit.power(1, 1), 0);
    assert!(runtime.managers.buff.has_buff_id(10, 31560181));

    let round = runtime
        .advance_round(BeginRoundRequest {
            opers: request.opers.into_iter().chain(reply.opers).collect(),
            devices_opers: reply.devices_opers,
            ..Default::default()
        })
        .unwrap();
    let flame = round
        .fight_step
        .iter()
        .find(|step| step.act_id == Some(31560181))
        .expect("the queued Flame Undying was cast");
    assert!(flame.act_effect.iter().any(|effect| {
        effect.effect_type
            == Some(sonettobuf::effect_type_enum::EffectType::Devicepowerchange as i32)
            && effect.reserve_str.as_deref() == Some("1#2")
    }));
    assert!(!runtime.managers.buff.has_buff_id(10, 31560181));
}

#[test]
fn auto_round_uses_the_upcoming_conduit_state() {
    let mut runtime = team_runtime_against(
        vec![hedone_with_stored_energy()],
        (1..=4).map(|index| enemy(-index, 1_000)).collect(),
        vec![hedone_energy_card()],
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
            skill_id: 31560121,
        })
        .unwrap();

    let current = runtime.current_round();
    let planner = runtime.auto_round_planner();
    let request = AutoRoundRequest::default();
    let reply = planner.plan(&request).unwrap();

    assert_eq!(reply.devices_opers[0].index, Some(2));
    assert_eq!(runtime.try_plan_auto_round(&request).unwrap(), reply);
    assert_eq!(planner.round_state.cur_round, current + 1);
    assert_eq!(runtime.current_round(), current);
    assert!(
        runtime
            .managers
            .conduit
            .configured_skill(10, 31560121)
            .unwrap()
            .1
            .is_stopped
    );

    runtime.round_state.grant_extra_round();
    let extra = runtime.auto_round_planner();
    assert_eq!(extra.round_state.cur_round, current);
    assert_eq!(
        extra.plan(&request).unwrap().devices_opers[0].index,
        Some(2)
    );
    assert_eq!(runtime.round_state.extra_rounds, 1);
}

#[test]
fn conduit_scoring_counts_every_mass_attack_target() {
    let plan = |enemy_count| {
        let runtime = team_runtime_against(
            vec![team_hero(10, 3156, 1_000, [31560111, 31560121])],
            (1..=enemy_count)
                .map(|index| enemy(-i64::from(index), 1_000))
                .collect(),
            vec![hand_card(10, 31446012, false)],
        );
        runtime
            .plan_auto_round(&AutoRoundRequest::default())
            .devices_opers[0]
            .index
    };

    assert_eq!(plan(1), Some(1));
    assert_eq!(plan(4), Some(2));
}

#[test]
fn auto_round_reserves_projected_energy_in_lineup_order() {
    let mut runtime = team_runtime(
        vec![
            team_hero(10, 3145, 1_000, [31450111, 31450121]),
            team_hero(11, 3144, 1_000, [31440111, 31440121]),
            team_hero(12, 3149, 1_000, [31490111, 31490121]),
            team_hero(13, 3156, 1_000, [31560111, 31560121]),
        ],
        vec![
            hand_card(10, 31450181, false),
            hand_card(11, 31446012, false),
            hand_card(12, 31446011, false),
            hand_card(13, 31446012, false),
            hand_card(10, 31446022, false),
            hand_card(11, 31446011, false),
            hand_card(12, 31446012, false),
            hand_card(13, 31446011, false),
            hand_card(10, 31446021, false),
            hand_card(12, 31446022, false),
        ],
    );
    runtime.round_state.act_point = 5;

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(
        reply
            .devices_opers
            .iter()
            .map(|oper| (oper.uid, oper.index))
            .collect::<Vec<_>>(),
        [
            (Some(10), Some(1)),
            (Some(11), Some(2)),
            (Some(12), Some(1)),
            (Some(13), Some(1))
        ]
    );
    assert_eq!(
        reply
            .opers
            .iter()
            .map(|oper| (oper.oper_type, oper.param1, oper.to_id))
            .collect::<Vec<_>>(),
        [
            (Some(2), Some(4), Some(-1)),
            (Some(2), Some(6), Some(-1)),
            (Some(2), Some(2), Some(-1)),
            (Some(2), Some(1), Some(-1)),
            (Some(2), Some(4), Some(-1)),
        ]
    );
}

#[test]
fn auto_round_uses_round_start_readiness_for_a_conduit_ultimate() {
    let mut twins = team_hero(10, 3149, 1_000, [31490111, 31490121]);
    twins.ex_point = Some(95);
    twins.ex_point_type = Some(4);
    twins.passive_skill = vec![31490171, 31490181, 31490191, 31490161];
    let mut runtime = team_runtime(vec![twins], vec![hand_card(10, 31446012, false)]);

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(runtime.managers.ex_point.get(10), 95);
    assert!(!reply.opers.is_empty());
    assert_ne!(reply.devices_opers[0].index, Some(3));
    runtime
        .advance_round(BeginRoundRequest {
            opers: reply.opers,
            devices_opers: reply.devices_opers,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(runtime.managers.ex_point.get(10), 100);
}

#[test]
fn auto_round_keeps_hedones_ready_unique_despite_its_post_action_reset() {
    let mut hedone = team_hero(10, 3156, 1_000, [31560111, 31560121]);
    hedone.ex_point = Some(100);
    hedone.ex_point_max = Some(100);
    hedone.ex_point_type = Some(4);
    hedone.ex_skill = Some(31560131);
    let runtime = team_runtime(vec![hedone], Vec::new());

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.devices_opers[0].index, Some(3));
}

#[test]
fn auto_round_keeps_selected_groups_without_owner_energy() {
    let mut runtime = team_runtime(
        vec![
            team_hero(10, 3149, 100_000, [31490111, 31490121]),
            team_hero(11, 3149, 100_000, [31490111, 31490121]),
        ],
        Vec::new(),
    );
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60294, "StopDeviceSkill"),
    };
    for source_uid in [10, 11] {
        runtime
            .managers
            .conduit
            .execute(ConduitCommand::SelectGroup {
                source_uid,
                group: 2,
            })
            .unwrap();
    }
    for (source_uid, skill_id) in [(10, 31490111), (11, 31490121)] {
        runtime
            .managers
            .conduit
            .execute(ConduitCommand::StopSkill {
                origin,
                source_uid,
                team: 1,
                skill_id,
            })
            .unwrap();
    }

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(
        reply
            .devices_opers
            .iter()
            .map(|operation| (operation.uid, operation.index))
            .collect::<Vec<_>>(),
        [(Some(10), Some(2)), (Some(11), Some(2))]
    );
    let pool = crate::engine::skill::target::TargetPool::from_fight_with_catalog(
        runtime.catalog_data.unwrap(),
        &runtime.fight,
    );
    let mut catalog = runtime.catalog.clone();
    crate::engine::runtime::schedule::run_conduit_phase(
        runtime.managers.catalog(),
        &runtime.fight,
        &mut runtime.managers,
        &pool,
        &mut catalog,
        &mut runtime.determinism,
        crate::engine::skill::target::TargetContext::default(),
        &reply.devices_opers,
    )
    .unwrap();
    assert_eq!(runtime.managers.conduit.uses(10), 1);
    assert_eq!(runtime.managers.conduit.uses(11), 1);
}
