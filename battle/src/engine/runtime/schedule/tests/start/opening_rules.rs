use super::*;

#[test]
fn start_schedule_finishes_unconditional_setup_before_round_start() {
    init_config();
    let unconditional = START
        .iter()
        .position(|step| *step == (SetupStage::Unconditional, 0))
        .unwrap();
    let first_round_start = START
        .iter()
        .position(|(stage, _)| *stage == SetupStage::RoundStart)
        .unwrap();

    assert!(!START.contains(&(SetupStage::EnterBattleStatic, 0)));
    assert!(unconditional < first_round_start);
    let sync = START
        .iter()
        .position(|step| *step == (SetupStage::BuffSync, 0))
        .unwrap();
    let late = START
        .iter()
        .position(|step| *step == (SetupStage::RoundStartLate, 0))
        .unwrap();
    let settlement = START
        .iter()
        .position(|step| *step == (SetupStage::RoundStart, 3))
        .unwrap();

    assert!(sync < late && late < settlement);
}

#[test]
fn opening_sync_runs_after_round_settlement_and_before_cards() {
    let settlement = START
        .iter()
        .position(|step| *step == (SetupStage::RoundStart, 3))
        .unwrap();
    let sync = START
        .iter()
        .position(|step| *step == (SetupStage::OpeningSync, 0))
        .unwrap();
    let cards = START
        .iter()
        .position(|step| *step == (SetupStage::CardSetup, 0))
        .unwrap();

    assert!(settlement < sync && sync < cards);
}

#[test]
fn opening_sync_runs_marsha_and_isolde_in_position_order() {
    init_config();

    fn run(marsha_position: i32, isolde_position: i32) -> (Vec<i64>, Vec<i64>) {
        let mut attackers = vec![
            FightEntityInfo {
                uid: Some(10),
                model_id: Some(3127),
                position: Some(marsha_position),
                team_type: Some(1),
                current_hp: Some(100),
                passive_skill: vec![31270145],
                ..Default::default()
            },
            FightEntityInfo {
                uid: Some(20),
                model_id: Some(3081),
                position: Some(isolde_position),
                team_type: Some(1),
                current_hp: Some(100),
                passive_skill: vec![30810311],
                buffs: vec![BuffInfo {
                    uid: Some(1002),
                    buff_id: Some(30810101),
                    from_uid: Some(20),
                    ..Default::default()
                }],
                ..Default::default()
            },
        ];
        attackers.sort_by_key(|entity| entity.position);
        let fight = Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: attackers,
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: [-1_i64, -2, -3]
                    .into_iter()
                    .enumerate()
                    .map(|(position, uid)| FightEntityInfo {
                        uid: Some(uid),
                        position: Some(position as i32 + 1),
                        team_type: Some(2),
                        current_hp: Some(100),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
        let mut managers = BattleManagers::seeded(&fight);
        let (opening, _) = run_start(
            managers.catalog(),
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext {
                current_round: 1,
                ..Default::default()
            },
            CardSetup {
                hand: Vec::new(),
                draw_pile: Vec::new(),
                deck_num: 0,
            },
            0,
        )
        .unwrap();

        fn collect_owners(frame: &SemanticFrame, owners: &mut Vec<i64>) {
            if let FrameOwner::Skill {
                source_uid,
                skill_id,
                ..
            } = frame.owner
                && [31270145, 30810311].contains(&skill_id)
                && owners.last() != Some(&source_uid)
            {
                owners.push(source_uid);
            }
            for item in &frame.items {
                if let FrameItem::Child(child) = item {
                    collect_owners(child, owners);
                }
            }
        }

        let mut owners = Vec::new();
        collect_owners(
            opening
                .frames
                .iter()
                .find(|frame| frame.owner == FrameOwner::RoundPhase(RoundPhase::RoundStartSync))
                .expect("opening has a synchronization phase"),
            &mut owners,
        );
        let mut preignition_uids = [10_i64, 20]
            .into_iter()
            .flat_map(|owner_uid| {
                managers
                    .buff
                    .active_for(owner_uid)
                    .filter(|buff| buff.buff_id == Some(4150002))
                    .filter_map(|buff| buff.uid)
            })
            .collect::<Vec<_>>();
        preignition_uids.sort_unstable();
        (owners, preignition_uids)
    }

    let marsha_first = run(1, 2);
    let isolde_first = run(2, 1);

    assert_eq!(marsha_first.0, [10, 20]);
    assert_eq!(isolde_first.0, [20, 10]);
    assert_eq!(marsha_first.1, [1016, 1017]);
    assert_eq!(isolde_first.1, [1013, 1014]);
}

fn collect_effects_of_type<'a>(
    effect: &'a sonettobuf::ActEffect,
    effect_type: i32,
    matches: &mut Vec<&'a sonettobuf::ActEffect>,
) {
    if effect.effect_type == Some(effect_type) {
        matches.push(effect);
    }
    if let Some(step) = &effect.fight_step {
        for nested in &step.act_effect {
            collect_effects_of_type(nested, effect_type, matches);
        }
    }
}

fn step_contains_effect(step: &sonettobuf::FightStep, effect_type: i32) -> bool {
    let mut matches = Vec::new();
    for effect in &step.act_effect {
        collect_effects_of_type(effect, effect_type, &mut matches);
    }
    !matches.is_empty()
}

fn step_contains_act(step: &sonettobuf::FightStep, act_id: i32) -> bool {
    step.act_id == Some(act_id)
        || step
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
            .any(|nested| step_contains_act(nested, act_id))
}

fn step_deletes_buff(step: &sonettobuf::FightStep, buff_id: i32) -> bool {
    step.act_effect.iter().any(|effect| {
        (effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Buffdel as i32)
            && effect.buff.as_ref().and_then(|buff| buff.buff_id) == Some(buff_id))
            || effect
                .fight_step
                .as_ref()
                .is_some_and(|nested| step_deletes_buff(nested, buff_id))
    })
}

#[test]
fn version_seven_opening_checks_round_start_skills_before_reset_and_buff_acts() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3053),
                team_type: Some(1),
                current_hp: Some(100),
                passive_skill: vec![30530150, 30530151],
                ..Default::default()
            }],
            device_area: Some(Default::default()),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);
    let (start, _) = run_start(
        managers.catalog(),
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 1,
            ..Default::default()
        },
        CardSetup {
            hand: Vec::new(),
            draw_pile: Vec::new(),
            deck_num: 0,
        },
        0,
    )
    .unwrap();
    let steps = crate::engine::packet::timeline::project_for_version(&start.frames, 7).unwrap();
    let call = steps
        .iter()
        .position(|step| step_contains_act(step, 305301511))
        .unwrap();
    let reset = steps
        .iter()
        .position(|step| {
            step_contains_effect(
                step,
                sonettobuf::effect_type_enum::EffectType::Devicepowerclear as i32,
            )
        })
        .unwrap();
    let consume = steps
        .iter()
        .position(|step| step_deletes_buff(step, 30530201))
        .unwrap();
    let call_before_reset = call < reset
        || (call == reset
            && steps[call].act_effect.iter().position(|effect| {
                effect
                    .fight_step
                    .as_ref()
                    .is_some_and(|step| step_contains_act(step, 305301511))
            }) < steps[reset].act_effect.iter().position(|effect| {
                effect.effect_type
                    == Some(sonettobuf::effect_type_enum::EffectType::Devicepowerclear as i32)
            }));

    assert!(
        call_before_reset && reset < consume,
        "call={call} reset={reset} consume={consume}"
    );
}

#[test]
fn anjo_contract_is_offered_once_during_opening_and_not_on_later_rounds() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3100),
                    position: Some(1),
                    team_type: Some(1),
                    current_hp: Some(100),
                    passive_skill: vec![31000141],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    model_id: Some(3086),
                    position: Some(2),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);
    let subscribers = crate::engine::event::dispatcher::dispatch_compiled_setup(
        &pool,
        &managers,
        &catalog,
        SetupStage::EnterBattleStatic,
        0,
    )
    .unwrap();
    assert_eq!(subscribers.len(), 1);
    assert_eq!(subscribers[0].0.owner_uid, 10);
    assert_eq!(subscribers[0].0.skill_id, 31000141);
    let mut determinism = RoundDeterminism::default();
    let (opening, _) = run_start(
        managers.catalog(),
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext {
            current_round: 1,
            ..Default::default()
        },
        CardSetup {
            hand: Vec::new(),
            draw_pile: Vec::new(),
            deck_num: 0,
        },
        0,
    )
    .unwrap();

    let contract_type = sonettobuf::effect_type_enum::EffectType::Notifiyherocontract as i32;
    let opening_steps =
        crate::engine::packet::timeline::project_for_version(&opening.frames, 7).unwrap();
    let mut opening_contracts = Vec::new();
    for effect in opening_steps.iter().flat_map(|step| &step.act_effect) {
        collect_effects_of_type(effect, contract_type, &mut opening_contracts);
    }
    assert_eq!(opening_contracts.len(), 1);
    assert_eq!(opening_contracts[0].target_id, Some(10));
    assert_eq!(opening_contracts[0].config_effect, Some(60092));
    assert_eq!(opening_contracts[0].reserve_str.as_deref(), Some("20"));
    assert!(managers.contract.selection_origin(10, 20).is_some());
    let enter_fight_deal = sonettobuf::effect_type_enum::EffectType::Enterfightdeal as i32;
    let deck_count = sonettobuf::effect_type_enum::EffectType::Carddecknum as i32;
    let enter_fight_deal = opening_steps
        .iter()
        .position(|step| step_contains_effect(step, enter_fight_deal))
        .unwrap();
    let deck_counts = opening_steps
        .iter()
        .enumerate()
        .filter(|(_, step)| step_contains_effect(step, deck_count))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let contract = opening_steps
        .iter()
        .position(|step| step_contains_effect(step, contract_type))
        .unwrap();
    assert_eq!(deck_counts.len(), 2);
    assert_eq!(
        opening_steps[deck_counts[0]]
            .act_effect
            .iter()
            .filter(|effect| effect.effect_type == Some(deck_count))
            .count(),
        1
    );
    assert_eq!(
        opening_steps[deck_counts[1]]
            .act_effect
            .iter()
            .filter(|effect| effect.effect_type == Some(deck_count))
            .count(),
        1
    );
    assert!(enter_fight_deal < deck_counts[0]);
    assert!(deck_counts[0] < contract && contract < deck_counts[1]);

    for round in [2, 3] {
        let (transition, _) = run_round_start_split(
            &mut managers,
            &pool,
            &catalog,
            &mut determinism,
            TargetContext {
                current_round: round,
                ..Default::default()
            },
            1,
        )
        .unwrap();
        let steps =
            crate::engine::packet::timeline::project_for_version(&transition.frames, 7).unwrap();
        let mut contracts = Vec::new();
        for effect in steps.iter().flat_map(|step| &step.act_effect) {
            collect_effects_of_type(effect, contract_type, &mut contracts);
        }
        assert!(
            contracts.is_empty(),
            "round {round} repeated the entry-only Contract offer"
        );
        assert!(managers.contract.selection_origin(10, 20).is_some());
    }
}

#[test]
fn early_round_start_precedes_condition_priorities() {
    assert_eq!(
        ROUND_START_BEFORE_DURATION_SETUP,
        &[
            (SetupStage::RoundStart, -1),
            (SetupStage::RoundStartCondition, 100),
            (SetupStage::RoundStartCondition, 101),
            (SetupStage::RoundStartCondition, 102),
        ]
    );
    assert!(
        START
            .iter()
            .position(|step| *step == (SetupStage::RoundStart, -1))
            < START
                .iter()
                .position(|step| *step == (SetupStage::RoundStartCondition, 102))
    );
}

#[test]
fn configured_special_temp_card_runs_during_the_opening_round_start_card_event() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3114),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(12),
                    buff_id: Some(31140143),
                    from_uid: Some(10),
                    layer: Some(1),
                    duration: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::default();
    let (start, _) = run_start(
        managers.catalog(),
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 1,
            ..Default::default()
        },
        CardSetup {
            hand: (0..7)
                .map(|index| CardInfo {
                    uid: Some(10),
                    skill_id: Some(100 + index),
                    ..Default::default()
                })
                .collect(),
            draw_pile: Vec::new(),
            deck_num: 48,
        },
        7,
    )
    .unwrap();

    assert!(
        managers.card.hand().iter().any(|card| {
            card.skill_id == Some(31140151) && card.uid == Some(10) && card.temp_card == Some(true)
        }),
        "hand={:?} active_features={:?}",
        managers.card.hand(),
        managers.buff.active_features(&managers.hp),
    );
    assert!(!managers.buff.has_buff_id(10, 31140143));
    let steps = crate::engine::packet::timeline::project(&start.frames).unwrap();
    let temp = steps
        .iter()
        .flat_map(|step| &step.act_effect)
        .find_map(|effect| effect.fight_step.as_ref())
        .unwrap();
    assert_eq!(
        temp.act_effect
            .iter()
            .map(|effect| effect.effect_type)
            .collect::<Vec<_>>(),
        vec![
            Some(sonettobuf::effect_type_enum::EffectType::Spcardadd as i32),
            Some(sonettobuf::effect_type_enum::EffectType::Changetotempcard as i32),
        ]
    );
    assert_eq!(temp.act_effect[1].reserve_str.as_deref(), Some("8"));
    assert!(steps.iter().any(|step| {
        step.act_effect.iter().any(|effect| {
            effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Buffdel as i32)
                && effect.target_id == Some(10)
                && effect.config_effect == Some(0)
                && effect.buff.as_ref().and_then(|buff| buff.buff_id) == Some(31140143)
                && effect.buff.as_ref().and_then(|buff| buff.layer) == Some(1)
        })
    }));
}

#[test]
fn buff_gated_skill_rule_runs_during_the_opening_round_start_card_event() {
    init_config();
    let (fight, catalog) = buff_gated_generic_temp_card_fixture();
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let (start, _) = run_start(
        managers.catalog(),
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 1,
            ..Default::default()
        },
        CardSetup {
            hand: Vec::new(),
            draw_pile: Vec::new(),
            deck_num: 0,
        },
        1,
    )
    .unwrap();

    assert_buff_gated_generic_temp_card(&managers, &start);
}

#[test]
fn configured_hero_temp_card_uses_the_live_group_rank_and_projection() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3070),
                team_type: Some(1),
                current_hp: Some(100),
                skill_group1: vec![307001172, 307001182, 307001192],
                buffs: vec![BuffInfo {
                    uid: Some(1444),
                    buff_id: Some(307002612),
                    from_uid: Some(10),
                    duration: Some(0),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let (start, _) = run_start(
        managers.catalog(),
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 4,
            ..Default::default()
        },
        CardSetup {
            hand: Vec::new(),
            draw_pile: Vec::new(),
            deck_num: 48,
        },
        7,
    )
    .unwrap();

    assert!(managers.card.hand().iter().any(|card| {
        card.skill_id == Some(307001182) && card.uid == Some(10) && card.temp_card == Some(true)
    }));
    assert!(!managers.buff.has_buff_id(10, 307002612));

    fn collect_card_effects<'a>(
        effect: &'a sonettobuf::ActEffect,
        effects: &mut Vec<&'a sonettobuf::ActEffect>,
    ) {
        if matches!(
            effect.effect_type,
            Some(
                value
            ) if value == sonettobuf::effect_type_enum::EffectType::Spcardadd as i32
                || value
                    == sonettobuf::effect_type_enum::EffectType::Changetotempcard as i32
        ) {
            effects.push(effect);
        }
        if let Some(step) = &effect.fight_step {
            for nested in &step.act_effect {
                collect_card_effects(nested, effects);
            }
        }
    }
    let steps = crate::engine::packet::timeline::project(&start.frames).unwrap();
    let mut effects = Vec::new();
    for effect in steps.iter().flat_map(|step| &step.act_effect) {
        collect_card_effects(effect, &mut effects);
    }
    assert_eq!(effects.len(), 1);
    let effect = effects[0];
    assert_eq!(
        effect.effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Spcardadd as i32)
    );
    assert_eq!(effect.target_id, Some(10));
    assert_eq!(effect.team_type, Some(1));
    assert!(effect.card_info.as_ref().is_some_and(|card| {
        card.skill_id == Some(307001182)
            && card.temp_card == Some(true)
            && card.hero_id == Some(3070)
    }));
}

#[test]
fn configured_skill3_buff_adds_the_captured_card_and_removes_its_carrier() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3124),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1622),
                    buff_id: Some(312451467),
                    from_uid: Some(10),
                    duration: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let (start, _) = run_start(
        managers.catalog(),
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 2,
            ..Default::default()
        },
        CardSetup {
            hand: Vec::new(),
            draw_pile: Vec::new(),
            deck_num: 48,
        },
        7,
    )
    .unwrap();

    let card = managers
        .card
        .hand()
        .iter()
        .find(|card| card.skill_id == Some(312451036))
        .expect("configured Skill 3 card is added to the hand");
    assert_eq!(card.uid, Some(10));
    assert_eq!(card.hero_id, Some(3124));
    assert_eq!(card.temp_card, Some(true));
    assert_eq!(
        card.card_type,
        Some(sonettobuf::card_info::CardType::Skill3 as i32)
    );
    assert!(!managers.buff.has_buff_id(10, 312451467));

    fn captured_step(effect: &sonettobuf::ActEffect) -> Option<&sonettobuf::FightStep> {
        let step = effect.fight_step.as_ref()?;
        if step.act_id == Some(312451467)
            && step.from_id == Some(10)
            && step.to_id == Some(10)
            && step.act_effect.iter().any(|effect| {
                effect.effect_type
                    == Some(sonettobuf::effect_type_enum::EffectType::Addhandcard as i32)
            })
        {
            return Some(step);
        }
        step.act_effect.iter().find_map(captured_step)
    }
    let steps = crate::engine::packet::timeline::project(&start.frames).unwrap();
    let nested = steps
        .iter()
        .flat_map(|step| &step.act_effect)
        .find_map(captured_step)
        .expect("configured Skill 3 uses the carrier-owned nested frame");
    let card_effect = nested
        .act_effect
        .iter()
        .find(|effect| {
            effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Addhandcard as i32)
        })
        .unwrap();
    assert_eq!(card_effect.target_id, Some(10));
    assert_eq!(card_effect.effect_num, Some(0));
    assert_eq!(card_effect.reserve_id, Some(0));
    assert_eq!(card_effect.team_type, Some(1));
    assert!(card_effect.card_info_list.is_empty());
    assert!(card_effect.card_info.as_ref().is_some_and(|card| {
        card.uid == Some(10)
            && card.skill_id == Some(312451036)
            && card.hero_id == Some(3124)
            && card.temp_card == Some(true)
            && card.card_type == Some(sonettobuf::card_info::CardType::Skill3 as i32)
    }));
    assert!(steps.iter().any(|step| {
        step.act_effect.iter().any(|effect| {
            effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Buffdel as i32)
                && effect.target_id == Some(10)
                && effect.buff.as_ref().and_then(|buff| buff.buff_id) == Some(312451467)
                && effect.buff.as_ref().and_then(|buff| buff.uid) == Some(1622)
        })
    }));
}

#[test]
fn twins_round_start_passive_adds_the_captured_precast_card() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                team_type: Some(1),
                current_hp: Some(100),
                passive_skill: vec![116385685],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [116385685], []);
    assert!(catalog.get(116385685).is_some());
    assert!(
        !crate::engine::event::dispatcher::dispatch_compiled_setup(
            &pool,
            &managers,
            &catalog,
            SetupStage::RoundStartCondition,
            101,
        )
        .unwrap()
        .is_empty()
    );
    let start = crate::engine::runtime::drain::run_setup_stage(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 1,
            ..Default::default()
        },
        SetupStage::RoundStartCondition,
        101,
    )
    .unwrap();

    assert!(managers.card.hand().iter().any(|card| {
        card.skill_id == Some(31446013) && card.uid == Some(10) && card.temp_card == Some(true)
    }));
    fn has_precast(effect: &sonettobuf::ActEffect) -> bool {
        (effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Spcardadd as i32)
            && effect.target_id == Some(10)
            && effect.effect_num == Some(31446013)
            && effect.reserve_id == Some(3149))
            || effect
                .fight_step
                .as_ref()
                .is_some_and(|step| step.act_effect.iter().any(has_precast))
    }
    let steps = crate::engine::packet::timeline::project(&start.frames).unwrap();
    assert!(
        steps
            .iter()
            .flat_map(|step| &step.act_effect)
            .any(has_precast)
    );
}

#[test]
fn side_owned_round_start_skill_only_runs_for_the_scheduled_side() {
    init_config();
    let entity = |uid, team_type| FightEntityInfo {
        uid: Some(uid),
        position: Some(1),
        team_type: Some(team_type),
        current_hp: Some(100),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.battle_rule.extend_owned_skills([
        crate::engine::fight::rules::OwnedBattleSkill {
            owner_uid: crate::engine::fight::rules::ATTACKER_SIDE_UID,
            skill_id: 40,
        },
        crate::engine::fight::rules::OwnedBattleSkill {
            owner_uid: crate::engine::fight::rules::DEFENDER_SIDE_UID,
            skill_id: 41,
        },
    ]);
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::new(1, "AddBuff", vec![31490001]),
        TargetRequest {
            code: 124,
            raw: Vec::new(),
        },
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 101,
        type_name: "None".to_owned(),
        kind: ParsedConditionKind::None(NoneMode::RoundStart),
        raw_args: Vec::new(),
    }];
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);
    let mut catalog = SkillEffectCatalog::default();
    for skill_id in [40, 41] {
        catalog.insert(ParsedSkillEffect {
            skill_id,
            slots: vec![slot.clone()],
        });
    }

    run_start(
        managers.catalog(),
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 1,
            ..Default::default()
        },
        CardSetup {
            hand: Vec::new(),
            draw_pile: Vec::new(),
            deck_num: 0,
        },
        0,
    )
    .unwrap();

    assert!(managers.buff.has_buff_id(10, 31490001));
    assert!(!managers.buff.has_buff_id(-1, 31490001));
}

#[test]
fn twins_battle_rule_unlocks_the_precast_card_after_two_round_starts() {
    init_config();
    let fight = Fight {
        battle_id: Some(116385108),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                position: Some(1),
                team_type: Some(1),
                current_hp: Some(100),
                attr: Some(sonettobuf::HeroAttribute {
                    attack: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                position: Some(1),
                team_type: Some(2),
                current_hp: Some(100),
                attr: Some(sonettobuf::HeroAttribute {
                    attack: Some(200),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(1000),
                    buff_id: Some(116385679),
                    from_uid: Some(-1),
                    layer: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    assert!(
        managers
            .battle_rule
            .owned_skills()
            .any(|owned| owned == (crate::engine::fight::rules::ATTACKER_SIDE_UID, 116385684))
    );

    run_start(
        managers.catalog(),
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 1,
            ..Default::default()
        },
        CardSetup {
            hand: Vec::new(),
            draw_pile: Vec::new(),
            deck_num: 0,
        },
        0,
    )
    .unwrap();

    assert!(managers.buff.has_buff_id(10, 116385669));
    assert!(
        !managers
            .card
            .hand()
            .iter()
            .any(|card| { card.skill_id == Some(31446013) && card.temp_card == Some(true) })
    );

    managers.begin_round();
    run_round_start_split(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 2,
            ..Default::default()
        },
        1,
    )
    .unwrap();

    assert!(managers.buff.has_buff_id(10, 116385670));
    assert!(managers.card.hand().iter().any(|card| {
        card.skill_id == Some(31446013) && card.uid == Some(10) && card.temp_card == Some(true)
    }));
}
