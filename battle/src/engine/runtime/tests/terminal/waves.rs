use super::settlement_and_rounds::{
    install_be_attacked_enemy_kill, install_round_start_enemy_kill, player_card,
};
use super::*;

#[test]
fn wave_entry_round_start_condition_is_not_repeated_on_the_next_request() {
    crate::test_support::init_config();
    let (entitys, sub_entitys) =
        crate::engine::fight::defender::Defender::build_wave_entities(251401, 2, 2, 0).unwrap();
    let skill_ids = [9_900_040, 9_900_050, 9_900_060];
    let mut runtime = runtime(Fight {
        battle_id: Some(2514),
        version: Some(7),
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
    for (skill_id, opcode, kind) in [
        (
            skill_ids[0],
            727100,
            ParsedConditionKind::RoundInterval {
                start_round: 0,
                period: 1,
            },
        ),
        (
            skill_ids[1],
            101,
            ParsedConditionKind::None(NoneMode::RoundStart),
        ),
        (
            skill_ids[2],
            102,
            ParsedConditionKind::None(NoneMode::RoundStart),
        ),
    ] {
        let mut slot = SkillEffectSlot::new(
            ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
            TargetRequest::self_only(),
        );
        slot.conditions = vec![ParsedCondition {
            opcode,
            type_name: if opcode == 727100 {
                "RoundAfter".to_owned()
            } else {
                "None".to_owned()
            },
            kind,
            raw_args: Vec::new(),
        }];
        slot.compiled_route = ConditionRoute::compile(&slot.conditions);
        runtime.catalog.insert(ParsedSkillEffect {
            skill_id,
            slots: vec![slot],
        });
    }
    runtime.managers.hp.lose(-1, i32::MAX, 10);
    runtime.managers.hp.lose(-2, i32::MAX, 10);

    runtime
        .build_begin_round_from_schedule(&BeginRoundRequest::default())
        .unwrap();
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
    assert_eq!(runtime.wave_entry_condition_uids, vec![-3, -4]);
    let defender = runtime.fight.defender.as_mut().unwrap();
    for entity in &mut defender.entitys {
        entity.ex_point = Some(0);
        entity.passive_skill = skill_ids.to_vec();
        entity.skill_group1.clear();
        entity.skill_group2.clear();
    }
    let catalog = runtime.managers.catalog();
    runtime.managers.entity.replace_team_roster(
        catalog,
        2,
        &defender.entitys,
        &defender.sub_entitys,
    );
    assert_eq!(runtime.managers.ex_point.get(-3), 0);
    assert_eq!(runtime.managers.ex_point.get(-4), 0);

    let second = runtime
        .build_begin_round_from_schedule(&BeginRoundRequest::default())
        .unwrap();
    fn collect_act_ids(step: &sonettobuf::FightStep, act_ids: &mut Vec<i32>) {
        if let Some(skill_id) = step.act_id.filter(|skill_id| *skill_id > 0) {
            act_ids.push(skill_id);
        }
        for child in step
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
        {
            collect_act_ids(child, act_ids);
        }
    }
    let mut act_ids = Vec::new();
    for step in &second.fight_step {
        collect_act_ids(step, &mut act_ids);
    }
    act_ids.retain(|skill_id| skill_ids.contains(skill_id));
    assert_eq!(
        act_ids,
        vec![skill_ids[1], skill_ids[1], skill_ids[2], skill_ids[2]]
    );
    assert!(runtime.wave_entry_condition_uids.is_empty());
}

#[test]
fn twins_full_harmonization_proc_survives_a_wave_transition() {
    crate::test_support::init_config();
    let (entitys, sub_entitys) =
        crate::engine::fight::defender::Defender::build_wave_entities(251401, 2, 2, 0).unwrap();
    let mut runtime = runtime(Fight {
        battle_id: Some(2514),
        version: Some(7),
        cur_round: Some(1),
        cur_wave: Some(1),
        max_round: Some(20),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                position: Some(1),
                team_type: Some(1),
                current_hp: Some(100),
                passive_skill: vec![31490161],
                skill_group1: vec![31490111, 31490112, 31490113],
                skill_group2: vec![31490121, 31490122, 31490123],
                ex_skill: Some(31490151),
                ex_point_type: Some(4),
                ex_point_max: Some(100),
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
    runtime.managers.ex_point.set(10, 10, 100, 0);
    runtime.managers.hp.lose(-1, i32::MAX, 10);
    runtime.managers.hp.lose(-2, i32::MAX, 10);

    let round = runtime
        .build_begin_round_from_schedule(&BeginRoundRequest::default())
        .unwrap();

    fn has_switch(step: &sonettobuf::FightStep) -> bool {
        step.act_effect.iter().any(|effect| {
            (effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Deviceskillindex as i32)
                && effect.target_id == Some(10)
                && effect.effect_num == Some(3))
                || effect.fight_step.as_ref().is_some_and(has_switch)
        })
    }
    assert_eq!(runtime.fight.cur_wave, Some(2));
    assert_eq!(runtime.managers.conduit.selected_group(10), Some(3));
    assert!(
        round.fight_step.iter().any(has_switch)
            || round.next_round_begin_step.iter().any(has_switch)
    );
}

fn wave_clear_runtime(
    hand: Vec<sonettobuf::CardInfo>,
) -> (BattleRuntime, Vec<sonettobuf::CardInfo>) {
    crate::test_support::init_config();
    let (mut entitys, sub_entitys) =
        crate::engine::fight::defender::Defender::build_wave_entities(251401, 2, 2, 0).unwrap();
    for entity in &mut entitys {
        entity.current_hp = Some(1);
    }
    let dealt = vec![
        player_card(30230121),
        player_card(30230111),
        player_card(30230121),
    ];
    let mut runtime = runtime(Fight {
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
                    attack: Some(1_000),
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
                hand,
                draw_pile: dealt.clone(),
                deck_num: dealt.len() as i32,
            },
        ))
        .unwrap();
    runtime.determinism.enqueue_card_draws(dealt.clone());
    (runtime, dealt)
}

fn seed_ai_turn(runtime: &mut BattleRuntime, choices: Vec<determinism::AiSkillChoice>) {
    let cards = choices
        .iter()
        .map(|choice| sonettobuf::CardInfo {
            uid: Some(choice.source_uid),
            skill_id: Some(choice.skill_id),
            target_uid: Some(choice.target_uid),
            ..Default::default()
        })
        .collect();
    runtime
        .managers
        .execute_card(crate::engine::manager::card::CardCommand::SetAiQueue(
            crate::engine::manager::card::CardSetAiQueue {
                origin: crate::engine::skill::rule::CommandOrigin {
                    domain: crate::engine::skill::rule::RuleDomain::Lifecycle,
                    key: crate::engine::skill::rule::DefinitionKey::new(0, "AiTurnQueue"),
                },
                cards,
            },
        ))
        .unwrap();
    runtime.determinism.enqueue_ai_skills(choices);
}

#[test]
fn wave_clear_runs_phase_two_refill_before_wave_transition() {
    let remaining = player_card(30230111);
    let (mut runtime, dealt) = wave_clear_runtime(vec![
        remaining.clone(),
        player_card(30230111),
        player_card(30230121),
    ]);
    let ai_choices = runtime
        .fight
        .defender
        .as_ref()
        .unwrap()
        .entitys
        .iter()
        .map(
            |entity| crate::engine::runtime::determinism::AiSkillChoice {
                source_uid: entity.uid.unwrap(),
                skill_id: entity.skill_group1[0],
                target_uid: 10,
            },
        )
        .collect::<Vec<_>>();
    seed_ai_turn(&mut runtime, ai_choices);

    let round = runtime
        .build_begin_round_from_schedule(&BeginRoundRequest {
            opers: vec![
                BeginRoundOper {
                    oper_type: Some(crate::engine::manager::card::CardOpType::PlayCard.id()),
                    param1: Some(2),
                    to_id: Some(-1),
                    ..Default::default()
                },
                BeginRoundOper {
                    oper_type: Some(crate::engine::manager::card::CardOpType::PlayCard.id()),
                    param1: Some(2),
                    to_id: Some(-2),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .unwrap();

    assert_eq!(
        round.before_cards1,
        std::iter::once(remaining.clone())
            .chain(dealt.iter().cloned())
            .collect::<Vec<_>>()
    );
    assert!(round.team_a_cards1.is_empty());
    assert_eq!(round.before_cards2, vec![remaining]);
    assert_eq!(round.team_a_cards2, dealt);
    let effect_types = round
        .fight_step
        .iter()
        .flat_map(|step| step.act_effect.iter())
        .filter_map(|effect| effect.effect_type)
        .collect::<Vec<_>>();
    let position = |effect_type| {
        effect_types
            .iter()
            .position(|effect| *effect == effect_type)
            .unwrap()
    };
    let deal = position(sonettobuf::effect_type_enum::EffectType::Dealcard2 as i32);
    let invalidations = effect_types
        .iter()
        .enumerate()
        .filter_map(|(index, effect)| {
            (*effect == sonettobuf::effect_type_enum::EffectType::Cardinvalid as i32)
                .then_some(index)
        })
        .collect::<Vec<_>>();
    let defender_settlement = effect_types
        .iter()
        .enumerate()
        .rfind(|(_, effect)| {
            **effect == sonettobuf::effect_type_enum::EffectType::Smallroundend as i32
        })
        .map(|(index, _)| index)
        .unwrap();
    let wave = position(sonettobuf::effect_type_enum::EffectType::Newchangewave as i32);

    assert_eq!(invalidations.len(), 2);
    assert!(deal < invalidations[0]);
    assert!(invalidations[1] < defender_settlement);
    assert!(defender_settlement < wave);
}

#[test]
fn predepleted_wave_runs_phase_two_refill_before_wave_transition() {
    let remaining = player_card(30230111);
    let (mut runtime, dealt) = wave_clear_runtime(vec![remaining.clone()]);
    let ai_choices = runtime
        .fight
        .defender
        .as_ref()
        .unwrap()
        .entitys
        .iter()
        .map(
            |entity| crate::engine::runtime::determinism::AiSkillChoice {
                source_uid: entity.uid.unwrap(),
                skill_id: entity.skill_group1[0],
                target_uid: 10,
            },
        )
        .collect::<Vec<_>>();
    seed_ai_turn(&mut runtime, ai_choices);
    runtime.managers.hp.lose(-1, i32::MAX, 10);
    runtime.managers.hp.lose(-2, i32::MAX, 10);

    let round = runtime
        .build_begin_round_from_schedule(&BeginRoundRequest::default())
        .unwrap();

    assert_eq!(runtime.fight.cur_wave, Some(2));
    assert_eq!(round.before_cards2, vec![remaining]);
    assert_eq!(round.team_a_cards2, dealt);
    let effect_types = round
        .fight_step
        .iter()
        .flat_map(|step| step.act_effect.iter())
        .filter_map(|effect| effect.effect_type)
        .collect::<Vec<_>>();
    let position = |effect_type| {
        effect_types
            .iter()
            .position(|effect| *effect == effect_type)
            .unwrap()
    };
    let deal = position(sonettobuf::effect_type_enum::EffectType::Dealcard2 as i32);
    let invalidations = effect_types
        .iter()
        .enumerate()
        .filter_map(|(index, effect)| {
            (*effect == sonettobuf::effect_type_enum::EffectType::Cardinvalid as i32)
                .then_some(index)
        })
        .collect::<Vec<_>>();
    let defender_settlement = effect_types
        .iter()
        .enumerate()
        .rfind(|(_, effect)| {
            **effect == sonettobuf::effect_type_enum::EffectType::Smallroundend as i32
        })
        .map(|(index, _)| index)
        .unwrap();
    let wave = position(sonettobuf::effect_type_enum::EffectType::Newchangewave as i32);

    assert_eq!(invalidations.len(), 2);
    assert!(deal < invalidations[0]);
    assert!(invalidations[1] < defender_settlement);
    assert!(defender_settlement < wave);
    assert!(effect_types[deal..wave].iter().all(|effect| {
        *effect != sonettobuf::effect_type_enum::EffectType::Devicepowerclear as i32
    }));
}

#[test]
fn enemy_phase_reaction_clear_advances_the_configured_wave() {
    let (mut runtime, _) = wave_clear_runtime(Vec::new());
    runtime.managers.hp.set_max(10, 1_000_000);
    runtime.managers.hp.heal(10, 999_900, 0);
    install_be_attacked_enemy_kill(&mut runtime, 10, 9_900_080, 9_900_081);
    let ai_choices = runtime
        .fight
        .defender
        .as_ref()
        .unwrap()
        .entitys
        .iter()
        .map(
            |entity| crate::engine::runtime::determinism::AiSkillChoice {
                source_uid: entity.uid.unwrap(),
                skill_id: 30230111,
                target_uid: 10,
            },
        )
        .collect::<Vec<_>>();
    seed_ai_turn(&mut runtime, ai_choices);

    let round = runtime
        .build_begin_round_from_schedule(&BeginRoundRequest::default())
        .unwrap();

    assert_eq!(runtime.managers.hp.current(-1), 0);
    assert_eq!(runtime.managers.hp.current(-2), 0);
    assert_eq!(runtime.fight.cur_wave, Some(2));
    assert!(round.fight_step.iter().any(|step| {
        step.act_effect.iter().any(|effect| {
            effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Newchangewave as i32)
        })
    }));
}

#[test]
fn round_start_clear_advances_the_configured_wave() {
    let (mut runtime, _) = wave_clear_runtime(Vec::new());
    runtime.managers.hp.set_max(10, 1_000_000);
    runtime.managers.hp.heal(10, 999_900, 0);
    install_round_start_enemy_kill(&mut runtime, 10, 9_900_090);

    let round = runtime
        .build_begin_round_from_schedule(&BeginRoundRequest::default())
        .unwrap();

    assert_eq!(runtime.managers.hp.current(-1), 0);
    assert_eq!(runtime.managers.hp.current(-2), 0);
    assert_eq!(runtime.fight.cur_wave, Some(2));
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
    assert!(round.fight_step.iter().any(|step| {
        step.act_effect.iter().any(|effect| {
            effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Newchangewave as i32)
        })
    }));
}
