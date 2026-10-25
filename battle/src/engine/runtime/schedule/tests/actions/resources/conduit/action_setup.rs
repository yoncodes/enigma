use super::*;

#[test]
fn conduit_attack_does_not_begin_without_a_living_enemy() {
    init_config();
    let entity = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(100),
        attr: Some(HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut twins = entity(10, 3149);
    twins.ex_skill = Some(31490151);
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![twins],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 1001)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.hp.lose(-1, 100, 0);
    managers
        .conduit
        .execute(
            crate::engine::manager::conduit::ConduitCommand::SelectGroup {
                source_uid: 10,
                group: 3,
            },
        )
        .unwrap();
    let mut catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    let result = run_conduit_phase(
        managers.catalog(),
        &fight,
        &mut managers,
        &pool,
        &mut catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[sonettobuf::FightDeviceOper {
            uid: Some(10),
            index: Some(3),
        }],
    )
    .unwrap();

    assert!(result.frames.is_empty());
    assert_eq!(managers.conduit.uses(10), 0);
}

#[test]
fn terminal_conduit_action_closes_its_running_state() {
    init_config();
    let entity = |uid, model_id, hp| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(hp),
        attr: Some(HeroAttribute {
            hp: Some(hp),
            attack: Some(100_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut twins = entity(10, 3149, 100_000);
    twins.ex_skill = Some(31490151);
    twins.ex_point = Some(100);
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![twins],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 1001, 1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .conduit
        .execute(
            crate::engine::manager::conduit::ConduitCommand::SelectGroup {
                source_uid: 10,
                group: 3,
            },
        )
        .unwrap();
    let mut catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    let result = run_conduit_phase(
        managers.catalog(),
        &fight,
        &mut managers,
        &pool,
        &mut catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[sonettobuf::FightDeviceOper {
            uid: Some(10),
            index: Some(3),
        }],
    )
    .unwrap();

    assert_eq!(managers.hp.current(-1), 0);
    assert!(!managers.conduit.is_running(10));
    assert_eq!(
        crate::engine::packet::timeline::project(&result.frames)
            .unwrap()
            .into_iter()
            .filter_map(|step| {
                step.act_effect
                    .iter()
                    .find(|effect| {
                        effect.effect_type
                            == Some(sonettobuf::effect_type_enum::EffectType::Devicerunning as i32)
                    })
                    .map(|effect| effect.effect_num)
            })
            .collect::<Vec<_>>(),
        [Some(1), Some(0)]
    );
}

#[test]
fn regulus_device_ultimate_selects_the_group_for_the_living_enemy_count() {
    init_config();
    let entity = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            ..Default::default()
        }),
        ..Default::default()
    };

    for (enemy_count, expected_group) in [(1, 1), (2, 2)] {
        let mut regulus = entity(10, 3025);
        regulus.ex_skill_level = Some(3);
        regulus.destiny_stone = Some(302502);
        regulus.destiny_rank = Some(4);
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![regulus],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: (0..enemy_count)
                    .map(|index| entity(-1 - i64::from(index), 1001))
                    .collect(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);
        managers
            .conduit
            .execute(
                crate::engine::manager::conduit::ConduitCommand::SelectGroup {
                    source_uid: 10,
                    group: 3,
                },
            )
            .unwrap();
        let mut catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

        run_conduit_phase(
            managers.catalog(),
            &fight,
            &mut managers,
            &pool,
            &mut catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            &[sonettobuf::FightDeviceOper {
                uid: Some(10),
                index: Some(3),
            }],
        )
        .unwrap();

        assert_eq!(managers.conduit.selected_group(10), Some(expected_group));
    }
}

#[test]
fn conduit_source_target_uses_the_first_living_main_ally_as_its_frame_anchor() {
    init_config();
    let entity = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(20, 3134), entity(10, 3149)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 1001)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    let result = run_conduit_phase(
        managers.catalog(),
        &fight,
        &mut managers,
        &pool,
        &mut catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[sonettobuf::FightDeviceOper {
            uid: Some(10),
            index: Some(1),
        }],
    )
    .unwrap();

    assert!(matches!(
        result.frames.first().map(|frame| &frame.owner),
        Some(crate::engine::runtime::record::FrameOwner::ConduitAction {
            target_uid: Some(20),
            ..
        })
    ));
    assert!(result.frames[0].items.iter().any(|item| matches!(
        item,
        crate::engine::runtime::record::FrameItem::Child(frame)
            if matches!(
                frame.owner,
                crate::engine::runtime::record::FrameOwner::ConduitSkill {
                    source_uid: 10,
                    skill_id: 31490111,
                    device_index: 1,
                    target_uid: Some(20),
                    ..
                }
            )
    )));
    assert_eq!(managers.conduit.power(1, 1), 1);
}

#[test]
fn device_power_card_keeps_ally_action_without_granting_loop_chain() {
    init_config();
    let entity = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(10, 3149),
                FightEntityInfo {
                    passive_skill: vec![31430151],
                    skill_group1: vec![31430111],
                    ..entity(30, 3143)
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 1001)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![
                CardInfo {
                    uid: Some(10),
                    skill_id: Some(31446011),
                    temp_card: Some(false),
                    card_type: Some(0),
                    hero_id: Some(3149),
                    ..Default::default()
                },
                CardInfo {
                    uid: Some(30),
                    skill_id: Some(31430111),
                    hero_id: Some(3143),
                    ..Default::default()
                },
            ],
            draw_pile: Vec::new(),
            deck_num: 2,
        }))
        .unwrap();
    let mut catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    managers
        .catalog()
        .extend_skill_roots(&mut catalog, [31446011], std::iter::empty());
    let play = || CardPlay {
        origin: CARD_PLAY_ORIGIN,
        hand_index: 0,
        target_uid: None,
        chosen_skill_id: None,
        choice: None,
        recorded_skill: None,
    };

    let device = run_player_action_queue(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [play()],
        1,
        crate::engine::manager::emitter::UID,
    )
    .unwrap();

    assert!(device.outcomes.iter().any(|outcome| matches!(
        outcome,
        RuleOutcome::SkillLifecycle(SkillLifecycle::ActionCompleted(action))
            if action.skill_id == 31446011 && action.mode == SkillExecutionMode::DeviceCard
    )));
    assert!(device.events.iter().any(|event| matches!(
        event,
        BattleEvent::AllyAction(action)
            if action.skill_id == 31446011 && action.mode == SkillExecutionMode::DeviceCard
    )));
    assert_eq!(managers.buff.buff_id_amount(30, 31430151), 0);

    let active = run_player_action_queue(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [play()],
        1,
        crate::engine::manager::emitter::UID,
    )
    .unwrap();

    assert!(active.events.iter().any(|event| matches!(
        event,
        BattleEvent::AllyAction(action)
            if action.skill_id == 31430111 && action.mode == SkillExecutionMode::Active
    )));
    assert_eq!(managers.buff.buff_id_amount(30, 31430151), 1);

    fn find_step(step: &sonettobuf::FightStep, act_id: i32) -> Option<&sonettobuf::FightStep> {
        (step.act_id == Some(act_id)).then_some(step).or_else(|| {
            step.act_effect
                .iter()
                .filter_map(|effect| effect.fight_step.as_ref())
                .find_map(|nested| find_step(nested, act_id))
        })
    }

    let steps = crate::engine::packet::timeline::project(&active.frames).unwrap();
    let reaction = steps
        .iter()
        .find_map(|step| find_step(step, 31430151))
        .unwrap();
    assert_eq!(reaction.to_id, Some(-1));
    assert!(reaction.act_effect.iter().any(|effect| {
        effect.target_id == Some(30)
            && effect
                .buff
                .as_ref()
                .is_some_and(|buff| buff.buff_id == Some(31430151))
    }));
}
