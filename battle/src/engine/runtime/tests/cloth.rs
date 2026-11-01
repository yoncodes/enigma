use super::*;

#[test]
fn rejects_unknown_cloth_skill_type_at_runtime_boundary() {
    crate::test_support::init_config();
    let mut runtime = runtime(Fight::default());
    runtime.pending_redeal = Some(RedealCardInfoPush::default());

    assert!(
        runtime
            .use_cloth_skill(UseClothSkillRequest {
                r#type: Some(-1),
                ..Default::default()
            })
            .is_none()
    );
    assert!(runtime.pending_redeal.is_some());
}

#[test]
fn configured_cloth_skills_drive_universal_and_redeal_card_rules() {
    crate::test_support::init_config();
    let card = |skill_id| CardInfo {
        uid: Some(10),
        skill_id: Some(skill_id),
        card_effect: Some(1),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(6),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                skill_group1: vec![101, 102],
                skill_group2: vec![201, 202],
                ..Default::default()
            }],
            player_entity: Some(FightEntityInfo {
                uid: Some(0),
                current_hp: Some(100),
                ..Default::default()
            }),
            power: Some(99),
            cloth_id: Some(1),
            skill_infos: crate::engine::fight::team::Team::get_player_skills(Some(1)),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = runtime(fight);
    runtime.catalog =
        SkillEffectCatalog::from_roots(config::configs::get(), [30010201, 30010202], []);
    runtime
        .managers
        .execute_card(crate::engine::manager::card::CardCommand::Setup(
            crate::engine::manager::card::CardSetup {
                hand: vec![card(101)],
                draw_pile: vec![card(101), card(201)],
                deck_num: 16,
            },
        ))
        .unwrap();

    let universal = runtime
        .use_cloth_skill(UseClothSkillRequest {
            skill_id: Some(30010201),
            from_id: Some(0),
            to_id: Some(0),
            r#type: Some(ClothSkillType::ClothSkill as i32),
        })
        .unwrap();
    assert_eq!(
        runtime.card_hand().last().unwrap().skill_id,
        Some(30_000_001)
    );
    let universal = universal.round.unwrap();
    assert_eq!(universal.power, Some(59));
    assert_eq!(
        universal
            .skill_infos
            .iter()
            .find(|skill| skill.skill_id == Some(30010201))
            .and_then(|skill| skill.need_power),
        Some(50)
    );
    assert!(universal.fight_step.iter().any(|step| {
        step.act_effect.iter().any(|effect| {
            effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Universalcard as i32)
        })
    }));

    runtime.determinism.enqueue_card_draws(vec![card(201)]);
    let redeal = runtime
        .use_cloth_skill(UseClothSkillRequest {
            skill_id: Some(30010202),
            from_id: Some(0),
            to_id: Some(0),
            r#type: Some(ClothSkillType::ClothSkill as i32),
        })
        .unwrap();
    let redeal = redeal.round.unwrap();
    assert_eq!(redeal.power, Some(34));
    let v6_effect = &redeal.fight_step[0].act_effect[0];
    assert_eq!(
        v6_effect.effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Redealcard as i32)
    );
    assert_eq!(v6_effect.config_effect, Some(60012));
    assert_eq!(v6_effect.team_type, Some(0));
    assert!(v6_effect.card_info_list.is_empty());
    assert_eq!(runtime.card_hand()[0].skill_id, Some(201));
    assert_eq!(
        runtime.take_redeal_card_push().unwrap().card_group,
        runtime.card_hand()
    );

    runtime.fight.version = Some(7);
    let redeal = runtime
        .use_cloth_skill(UseClothSkillRequest {
            skill_id: Some(30010202),
            from_id: Some(0),
            to_id: Some(0),
            r#type: Some(ClothSkillType::ClothSkill as i32),
        })
        .unwrap()
        .round
        .unwrap();
    assert_eq!(redeal.power, Some(9));
    assert_eq!(redeal.fight_step.len(), 2);
    assert_eq!(
        redeal.fight_step[0].act_effect[0].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Afterredealcard as i32)
    );
    assert_eq!(redeal.fight_step[0].act_effect[0].team_type, Some(1));
    assert!(
        redeal.fight_step[0].act_effect[0]
            .card_info_list
            .iter()
            .map(|card| (card.uid, card.skill_id, card.temp_card.unwrap_or_default()))
            .eq(runtime.card_hand().iter().map(|card| (
                card.uid,
                card.skill_id,
                card.temp_card.unwrap_or_default()
            )))
    );
    assert_eq!(
        redeal.fight_step[1].act_effect[0].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Cardspush as i32)
    );
    assert_eq!(redeal.fight_step[1].act_effect[0].effect_num, Some(0));
    assert_eq!(redeal.fight_step[1].act_effect[0].effect_num1, Some(0));
    assert_eq!(redeal.fight_step[1].act_effect[0].team_type, Some(0));
    assert!(
        redeal.fight_step[1].act_effect[0]
            .card_info_list
            .iter()
            .map(|card| (card.uid, card.skill_id, card.temp_card.unwrap_or_default()))
            .eq(runtime.card_hand().iter().map(|card| (
                card.uid,
                card.skill_id,
                card.temp_card.unwrap_or_default()
            )))
    );
    assert!(runtime.take_redeal_card_push().is_none());
}

#[test]
fn first_melody_redeal_preserves_basic_coverage_and_ranks() {
    crate::test_support::init_config();
    let card = |uid, skill_id, rank| CardInfo {
        uid: Some(uid),
        skill_id: Some(skill_id),
        card_effect: Some(rank),
        ..Default::default()
    };
    let entity = |uid, group1, group2| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100),
        skill_group1: group1,
        skill_group2: group2,
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    ex_skill: Some(30280131),
                    ..entity(10, vec![101, 102], vec![201, 202])
                },
                entity(20, vec![301, 302], vec![401, 402]),
                entity(30, vec![501, 502], vec![601, 602]),
            ],
            player_entity: Some(FightEntityInfo {
                uid: Some(0),
                current_hp: Some(100),
                ..Default::default()
            }),
            power: Some(99),
            cloth_id: Some(1),
            skill_infos: crate::engine::fight::team::Team::get_player_skills(Some(1)),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = runtime(fight);
    runtime.catalog = SkillEffectCatalog::from_roots(config::configs::get(), [30010202], []);
    let repeated = card(10, 101, 1);
    let setup = || crate::engine::manager::card::CardSetup {
        hand: vec![
            card(10, 102, 2),
            repeated.clone(),
            card(10, 102, 2),
            card(10, 30280131, 1),
            repeated.clone(),
            repeated.clone(),
            repeated.clone(),
            repeated.clone(),
        ],
        draw_pile: vec![
            repeated.clone(),
            card(10, 201, 1),
            card(20, 301, 1),
            card(20, 401, 1),
            card(30, 501, 1),
            card(30, 601, 1),
        ],
        deck_num: 48,
    };
    runtime
        .managers
        .execute_card(crate::engine::manager::card::CardCommand::Setup(setup()))
        .unwrap();
    runtime
        .determinism
        .enqueue_card_draws(vec![repeated.clone(); 7]);

    runtime
        .use_cloth_skill(UseClothSkillRequest {
            skill_id: Some(30010202),
            from_id: Some(0),
            to_id: Some(0),
            r#type: Some(ClothSkillType::ClothSkill as i32),
        })
        .unwrap();

    let hand = runtime.card_hand();
    assert_eq!(
        hand.iter()
            .map(|card| (card.uid, card.skill_id))
            .collect::<Vec<_>>(),
        vec![
            (Some(10), Some(102)),
            (Some(10), Some(101)),
            (Some(30), Some(602)),
            (Some(10), Some(30280131)),
            (Some(20), Some(401)),
            (Some(10), Some(201)),
            (Some(30), Some(501)),
            (Some(20), Some(301)),
        ]
    );

    runtime.managers.register_entity(&FightEntityInfo {
        uid: Some(40),
        team_type: Some(1),
        position: Some(4),
        current_hp: Some(100),
        attr: Some(sonettobuf::HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    });
    runtime
        .managers
        .execute_card(crate::engine::manager::card::CardCommand::Setup(setup()))
        .unwrap();
    runtime
        .determinism
        .enqueue_card_draws(vec![repeated.clone(); 7]);
    runtime
        .use_cloth_skill(UseClothSkillRequest {
            skill_id: Some(30010202),
            from_id: Some(0),
            to_id: Some(0),
            r#type: Some(ClothSkillType::ClothSkill as i32),
        })
        .unwrap();

    assert_eq!(
        runtime
            .card_hand()
            .iter()
            .map(|card| (card.uid, card.skill_id))
            .collect::<Vec<_>>(),
        vec![
            (Some(10), Some(102)),
            (Some(10), Some(101)),
            (Some(10), Some(102)),
            (Some(10), Some(30280131)),
            (Some(10), Some(101)),
            (Some(10), Some(101)),
            (Some(10), Some(101)),
            (Some(10), Some(101)),
        ]
    );
}

#[test]
fn conduit_selection_adds_the_configured_precast_and_commits_the_choice() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                current_hp: Some(100),
                buffs: vec![sonettobuf::BuffInfo {
                    uid: Some(1146),
                    buff_id: Some(31490013),
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
    let mut runtime = runtime(fight);
    let reply = runtime
        .use_cloth_skill(UseClothSkillRequest {
            skill_id: Some(0),
            from_id: Some(10),
            to_id: Some(4),
            r#type: Some(ClothSkillType::TwinsSelect as i32),
        })
        .unwrap()
        .round
        .unwrap();

    let card = runtime.card_hand().last().unwrap();
    assert_eq!(card.skill_id, Some(31446012));
    assert_eq!(card.temp_card, Some(true));
    assert_eq!(card.hero_id, Some(3149));
    assert_eq!(runtime.managers.card.normal_hand_len(), 0);
    let effects = reply
        .fight_step
        .iter()
        .flat_map(|step| &step.act_effect)
        .collect::<Vec<_>>();
    assert!(effects.iter().any(|effect| {
        effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Addhandcard as i32)
            && effect.card_info.as_ref().and_then(|card| card.skill_id) == Some(31446012)
    }));
    assert!(effects.iter().any(|effect| {
        effect.effect_type
            == Some(sonettobuf::effect_type_enum::EffectType::Buffactinfoupdate as i32)
            && effect.buff_act_info.as_ref().is_some_and(|info| {
                info.act_id == Some(10030)
                    && info.param
                        == [
                            4, 1, 31495201, 31495211, 31446012, 31446022, 31446013, 31446023,
                        ]
            })
    }));
    let selection_update = effects
        .iter()
        .position(|effect| {
            effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Buffactinfoupdate as i32)
        })
        .unwrap();
    let carrier_delete = effects
        .iter()
        .position(|effect| {
            effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Buffdel as i32)
                && effect
                    .buff
                    .as_ref()
                    .is_some_and(|buff| buff.uid == Some(1146) && buff.duration == Some(0))
        })
        .unwrap();
    assert!(selection_update < carrier_delete);
    assert!(runtime.managers.buff.snapshot(10, 1146).is_none());
    assert!(
        runtime
            .use_cloth_skill(UseClothSkillRequest {
                skill_id: Some(0),
                from_id: Some(10),
                to_id: Some(2),
                r#type: Some(ClothSkillType::TwinsSelect as i32),
            })
            .is_none()
    );
}

#[test]
fn owned_charge_starts_the_captured_extra_round_flow() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3146),
                current_hp: Some(100),
                buffs: vec![sonettobuf::BuffInfo {
                    uid: Some(1008),
                    buff_id: Some(31460143),
                    from_uid: Some(10),
                    act_info: vec![sonettobuf::BuffActInfo {
                        act_id: Some(1139),
                        param: vec![102_000],
                        str_param: Some(String::new()),
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(100_000),
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(100_000),
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
        .execute_card(crate::engine::manager::card::CardCommand::Setup(
            crate::engine::manager::card::CardSetup {
                hand: vec![CardInfo {
                    uid: Some(10),
                    skill_id: Some(31460111),
                    ..Default::default()
                }],
                draw_pile: Vec::new(),
                deck_num: 59,
            },
        ))
        .unwrap();

    runtime.managers.execute_card(crate::engine::manager::card::CardCommand::SetAiQueue(
        crate::engine::manager::card::CardSetAiQueue {
            origin: crate::engine::skill::rule::CommandOrigin {
                domain: crate::engine::skill::rule::RuleDomain::Lifecycle,
                key: crate::engine::skill::rule::DefinitionKey::new(0, "TestAiQueue"),
            },
            cards: vec![CardInfo { uid: Some(-1), skill_id: Some(31460111), target_uid: Some(10), ..Default::default() }],
        },
    )).unwrap();
    let mut normal = runtime.clone();
    let normal_round = normal.advance_round(sonettobuf::BeginRoundRequest::default()).unwrap();
    assert!(normal_round.fight_step.iter().any(|step| step.from_id == Some(-1) && step.act_id == Some(31460111)),
        "the configured opponent must act in an ordinary round");

    let round = runtime
        .use_cloth_skill(UseClothSkillRequest {
            skill_id: Some(0),
            from_id: Some(10),
            to_id: Some(0),
            r#type: Some(ClothSkillType::ExtraRound as i32),
        })
        .unwrap()
        .round
        .unwrap();

    let info = runtime
        .managers
        .buff
        .snapshot(10, 1008)
        .unwrap()
        .act_info
        .into_iter()
        .find(|info| info.act_id == Some(1139))
        .unwrap();
    assert_eq!(info.param, [2_000]);
    assert_eq!(round.fight_step.len(), 2);
    assert_eq!(round.fight_step[0].from_id, Some(10));
    assert_eq!(round.fight_step[0].to_id, Some(0));
    assert_eq!(
        round.fight_step[0]
            .act_effect
            .iter()
            .map(|effect| effect.effect_type.unwrap_or_default())
            .collect::<Vec<_>>(),
        [
            sonettobuf::effect_type_enum::EffectType::Buffactinfoupdate as i32,
            sonettobuf::effect_type_enum::EffectType::Fightparamchange as i32,
        ]
    );
    assert!(round.fight_step[0].act_effect.iter().any(|effect| {
        effect.effect_type
            == Some(sonettobuf::effect_type_enum::EffectType::Buffactinfoupdate as i32)
            && effect
                .buff_act_info
                .as_ref()
                .is_some_and(|info| info.act_id == Some(1139) && info.param == [2_000])
    }));
    assert!(round.fight_step[0].act_effect.iter().any(|effect| {
        effect.effect_type
            == Some(sonettobuf::effect_type_enum::EffectType::Fightparamchange as i32)
            && effect.reserve_str.as_deref() == Some("18#1")
    }));
    assert_eq!(
        round.fight_step[1]
            .act_effect
            .iter()
            .map(|effect| effect.effect_type.unwrap_or_default())
            .collect::<Vec<_>>(),
        [
            sonettobuf::effect_type_enum::EffectType::Cardspush as i32,
            sonettobuf::effect_type_enum::EffectType::Carddecknum as i32,
        ]
    );
    assert!(
        runtime
            .use_cloth_skill(UseClothSkillRequest {
                skill_id: Some(0),
                from_id: Some(10),
                to_id: Some(0),
                r#type: Some(ClothSkillType::ExtraRound as i32),
            })
            .is_none()
    );
    let active_round = runtime.current_round();
    let mut with_play = runtime.clone();
    let played_round = with_play.advance_round(sonettobuf::BeginRoundRequest {
        opers: vec![sonettobuf::BeginRoundOper {
            oper_type: Some(2), param1: Some(1), to_id: Some(-1), ..Default::default()
        }], ..Default::default()
    }).unwrap();
    let feast = played_round.fight_step.iter().position(|step| step.act_id == Some(31460183)).unwrap();
    let selected = played_round.fight_step.iter().position(|step| step.act_id == Some(31460111)).unwrap();
    assert!(selected < feast, "the generated attack must execute after selected incantations");
    assert!(!played_round.fight_step.iter().any(|step| step.from_id == Some(-1) && step.act_id == Some(31460111)),
        "opponents cannot act during the extra round");
    let extra_round = runtime.advance_round(sonettobuf::BeginRoundRequest::default()).unwrap();
    assert_eq!(extra_round.cur_round, Some(active_round));
    assert!(extra_round.fight_step.iter().any(|step| step.act_id == Some(31460183)),
        "the charge's configured Feast Call must execute during the extra round");
    assert!(extra_round.fight_step.iter().flat_map(|step| &step.act_effect).any(|effect|
        effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Addusecard as i32)
            && effect.card_info.as_ref().is_some_and(|card| card.skill_id == Some(31460183))));
}

#[test]
fn contract_selection_uses_the_offered_uid_and_fight_const_buff_pair() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    career: Some(4),
                    ex_skill_level: Some(0),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    career: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = runtime(fight);
    runtime
        .managers
        .contract
        .execute(crate::engine::manager::contract::ContractCommand::Offer {
            origin: crate::engine::skill::rule::CommandOrigin {
                domain: crate::engine::skill::rule::RuleDomain::Behavior,
                key: crate::engine::skill::rule::DefinitionKey::new(60092, "NotifyHeroContract"),
            },
            owner_uid: -1,
            candidates: vec![20],
        })
        .unwrap();

    let round = runtime
        .use_cloth_skill(UseClothSkillRequest {
            skill_id: Some(0),
            from_id: Some(-1),
            to_id: Some(20),
            r#type: Some(ClothSkillType::Contract as i32),
        })
        .unwrap()
        .round
        .unwrap();

    assert_eq!(runtime.managers.contract.bound_uid(-1), Some(20));
    assert!(runtime.managers.buff.has_buff_id(-1, 31000221));
    assert!(runtime.managers.buff.has_buff_id(20, 31000191));
    let effects = round
        .fight_step
        .iter()
        .flat_map(|step| &step.act_effect)
        .collect::<Vec<_>>();
    let kinds = effects
        .iter()
        .map(|effect| effect.effect_type.unwrap_or_default())
        .collect::<Vec<_>>();
    let owner_buff = effects
        .iter()
        .position(|effect| effect.buff.as_ref().and_then(|buff| buff.buff_id) == Some(31000221))
        .unwrap();
    let owner = kinds
        .iter()
        .position(|kind| *kind == sonettobuf::effect_type_enum::EffectType::Contranct as i32)
        .unwrap();
    let bound = kinds
        .iter()
        .position(|kind| *kind == sonettobuf::effect_type_enum::EffectType::Becontrancted as i32)
        .unwrap();
    let bound_buff = effects
        .iter()
        .position(|effect| effect.buff.as_ref().and_then(|buff| buff.buff_id) == Some(31000191))
        .unwrap();
    assert!(owner_buff < owner && owner < bound_buff && bound_buff < bound);
}

#[test]
fn configured_upgrade_and_contract_prompts_resolve_in_opening_order() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3086),
                    position: Some(1),
                    team_type: Some(1),
                    career: Some(6),
                    current_hp: Some(100),
                    passive_skill: vec![30864156, 30864112],
                    enhance_info_box: Some(sonettobuf::EnhanceInfoBox {
                        uid: Some(10),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    model_id: Some(3100),
                    position: Some(2),
                    team_type: Some(1),
                    career: Some(4),
                    ex_skill_level: Some(0),
                    current_hp: Some(100),
                    passive_skill: vec![31000141],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(30),
                    position: Some(3),
                    team_type: Some(1),
                    career: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = runtime(fight);
    let opening = runtime.start_round().unwrap();
    let upgrade = sonettobuf::effect_type_enum::EffectType::Notifyupgradehero as i32;
    let contract = sonettobuf::effect_type_enum::EffectType::Notifiyherocontract as i32;
    fn collect_prompts(
        effect: &sonettobuf::ActEffect,
        upgrade: i32,
        contract: i32,
        prompts: &mut Vec<(i32, i64)>,
    ) {
        if effect
            .effect_type
            .is_some_and(|kind| kind == upgrade || kind == contract)
        {
            prompts.push((
                effect.effect_type.unwrap(),
                effect.target_id.unwrap_or_default(),
            ));
        }
        if let Some(step) = &effect.fight_step {
            for nested in &step.act_effect {
                collect_prompts(nested, upgrade, contract, prompts);
            }
        }
    }
    let mut prompts = Vec::new();
    for effect in opening.fight_step.iter().flat_map(|step| &step.act_effect) {
        collect_prompts(effect, upgrade, contract, &mut prompts);
    }
    let contract_prompt = prompts
        .iter()
        .position(|prompt| *prompt == (contract, 20))
        .unwrap();
    assert!(contract_prompt > 0);
    assert!(
        prompts[..contract_prompt]
            .iter()
            .all(|prompt| *prompt == (upgrade, 10))
    );
    assert_eq!(&prompts[contract_prompt..], &[(contract, 20)]);

    assert!(
        runtime
            .use_cloth_skill(UseClothSkillRequest {
                skill_id: Some(308665),
                from_id: Some(10),
                to_id: Some(3086515),
                r#type: Some(ClothSkillType::HeroUpgrade as i32),
            })
            .is_some()
    );
    assert!(
        runtime
            .use_cloth_skill(UseClothSkillRequest {
                skill_id: Some(0),
                from_id: Some(20),
                to_id: Some(30),
                r#type: Some(ClothSkillType::Contract as i32),
            })
            .is_some()
    );
    assert_eq!(runtime.managers.upgrade.selected_option(10), Some(3086515));
    assert_eq!(runtime.managers.contract.bound_uid(20), Some(30));
}


fn unfinished_tune_fixture(power: i32) -> BattleRuntime {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10), model_id: Some(3141), team_type: Some(1),
                current_hp: Some(100), position: Some(1),
                ex_skill: Some(31410131), skill_group1: vec![31410111, 31410112],
                skill_group2: vec![31410121, 31410122], ..Default::default()
            }],
            player_entity: Some(FightEntityInfo { uid: Some(0), current_hp: Some(100), ..Default::default() }),
            power: Some(power), cloth_id: Some(6),
            skill_infos: crate::engine::fight::team::Team::get_player_skills(Some(6)),
            ..Default::default()
        }), ..Default::default()
    };
    let mut runtime = runtime(fight);
    runtime.catalog = SkillEffectCatalog::from_roots(config::configs::get(),
        [30010501, 30010502, 31410111, 31410121, 31410131], []);
    runtime
}

#[test]
fn unfinished_tune_generates_selected_ally_rank_one_cards_and_spends_power() {
    let mut runtime = unfinished_tune_fixture(99);
    let request = |target| UseClothSkillRequest { skill_id: Some(30010501), from_id: Some(0),
        to_id: Some(target), r#type: Some(0), ..Default::default() };
    let initial = runtime.round_state.power;
    assert!(runtime.use_cloth_skill(request(-1)).is_none());
    assert_eq!(runtime.round_state.power, initial);
    assert!(runtime.card_hand().is_empty());
    let reply = runtime.use_cloth_skill(request(10)).unwrap().round.unwrap();
    assert_eq!(reply.power, Some(initial - 40));
    assert_eq!(runtime.card_hand().iter().map(|card| card.skill_id.unwrap()).collect::<Vec<_>>(),
        vec![31410111, 31410121]);
    assert!(runtime.card_hand().iter().all(|card| card.uid == Some(10) && card.temp_card == Some(true)));
    let adds: Vec<_> = reply.fight_step.iter().flat_map(|step| &step.act_effect)
        .filter(|effect| effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Masteraddhandcard as i32))
        .collect();
    assert_eq!(adds.len(), 2);
    assert!(adds.iter().all(|effect| effect.team_type == Some(1)));
    assert_eq!(adds.iter().map(|effect| effect.card_info.as_ref().unwrap().skill_id.unwrap()).collect::<Vec<_>>(),
        vec![31410111, 31410121]);
}

#[test]
fn unfinished_tune_removes_rightmost_non_ultimate_and_recovers_capped_power() {
    let mut runtime = unfinished_tune_fixture(20);
    let card = |skill_id| CardInfo { uid: Some(10), hero_id: Some(3141), skill_id: Some(skill_id),
        card_effect: Some(1), ..Default::default() };
    runtime.managers.execute_card(crate::engine::manager::card::CardCommand::Setup(CardSetup {
        hand: vec![card(31410131), card(31410111), card(31410121)], draw_pile: vec![], deck_num: 0,
    })).unwrap();
    let initial = runtime.round_state.power;
    let reply = runtime.use_cloth_skill(UseClothSkillRequest { skill_id: Some(30010502),
        from_id: Some(0), to_id: Some(0), r#type: Some(0), ..Default::default() }).unwrap().round.unwrap();
    assert_eq!(runtime.card_hand().iter().map(|card| card.skill_id.unwrap()).collect::<Vec<_>>(),
        vec![31410131, 31410121]);
    assert_eq!(reply.power, Some((initial + 10).min(99)));
    let effect = reply.fight_step.iter().flat_map(|step| &step.act_effect)
        .find(|effect| effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Mastercardremove as i32))
        .expect("client receives the animated master-card removal effect");
    assert_eq!(effect.reserve_str.as_deref(), Some("2"));
    assert_eq!(effect.team_type, Some(1));
    assert_eq!(effect.config_effect, Some(60031));
    assert!(reply.fight_step.iter().flat_map(|step| &step.act_effect).any(|effect| {
        effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Cardspush as i32)
            && effect.card_info_list.iter().map(|card| card.skill_id).eq(runtime.card_hand().iter().map(|card| card.skill_id))
    }));
}


fn atonal_sequence_fixture() -> BattleRuntime {
    crate::test_support::init_config();
    let fight = Fight { version: Some(7), attacker: Some(FightTeam {
        entitys: vec![FightEntityInfo { uid: Some(10), model_id: Some(3149), team_type: Some(1),
            current_hp: Some(100), position: Some(1), skill_group1: vec![31490111,31490121],
            skill_group2: vec![31490131,31490141], ..Default::default() }],
        player_entity: Some(FightEntityInfo { uid: Some(0), current_hp: Some(100), ..Default::default() }),
        power: Some(99), cloth_id: Some(7),
        skill_infos: crate::engine::fight::team::Team::get_player_skills(Some(7)),
        ..Default::default() }), ..Default::default() };
    let mut runtime = runtime(fight);
    runtime.catalog = SkillEffectCatalog::from_roots(config::configs::get(), [30010701,30010702,31446062], []);
    runtime
}

#[test]
fn atonal_sequence_creates_energy_for_the_twins() {
    let mut runtime = atonal_sequence_fixture();
    let reply = runtime.use_cloth_skill(UseClothSkillRequest { skill_id: Some(30010702),
        from_id: Some(0), to_id: Some(0), r#type: Some(0), ..Default::default() })
        .expect("Atonal Sequence should generate the card").round.unwrap();
    assert_eq!(reply.power, Some(59));
    assert!(runtime.card_hand().iter().any(|c| c.uid == Some(10)
        && c.skill_id == Some(31446062) && c.temp_card == Some(true)));
    let effect = reply.fight_step.iter().flat_map(|step| &step.act_effect)
        .find(|effect| effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Addsphandcard as i32))
        .expect("client receives the special energy-card animation and full card data");
    assert_eq!(effect.effect_num1, Some(30010702));
    assert_eq!(effect.card_info.as_ref().unwrap().skill_id, Some(31446062));
    assert_eq!(effect.card_info.as_ref().unwrap().temp_card, Some(true));
    let skill = runtime.fight.attacker.as_mut().unwrap().skill_infos.iter_mut()
        .find(|skill| skill.skill_id == Some(30010702)).unwrap();
    assert_eq!(skill.need_power, Some(50));
    assert_eq!(skill.cd, Some(1));
    skill.cd = Some(0);
    let second = runtime.use_cloth_skill(UseClothSkillRequest { skill_id: Some(30010702),
        from_id: Some(0), to_id: Some(0), r#type: Some(0), ..Default::default() })
        .unwrap().round.unwrap();
    assert_eq!(second.power, Some(9));
    let skill = runtime.fight.attacker.as_mut().unwrap().skill_infos.iter_mut()
        .find(|skill| skill.skill_id == Some(30010702)).unwrap();
    assert_eq!(skill.need_power, Some(60));
    skill.cd = Some(0);
    assert!(runtime.use_cloth_skill(UseClothSkillRequest { skill_id: Some(30010702),
        from_id: Some(0), to_id: Some(0), r#type: Some(0), ..Default::default() }).is_none());
    assert_eq!(runtime.card_hand().len(), 2, "insufficient power must not add another card");
}

#[test]
fn atonal_sequence_exchanges_only_the_selected_energy_card() {
    let mut runtime = atonal_sequence_fixture();
    let card = |skill_id| CardInfo { uid: Some(10), hero_id: Some(3149), skill_id: Some(skill_id),
        temp_card: Some(false), ..Default::default() };
    runtime.managers.execute_card(crate::engine::manager::card::CardCommand::Setup(CardSetup {
        hand: vec![card(31446011), card(31490111)], draw_pile: vec![], deck_num: 0,
    })).unwrap();
    let request = |index| UseClothSkillRequest { skill_id: Some(30010701),
        to_id: Some(index), r#type: Some(0), ..Default::default() };
    assert!(runtime.use_cloth_skill(request(2)).is_none(), "normal cards cannot be exchanged");
    assert_eq!(runtime.card_hand().len(), 2);
    let reply = runtime.use_cloth_skill(request(1)).unwrap().round.unwrap();
    assert_eq!(reply.power, Some(99));
    assert_eq!(runtime.card_hand().len(), 2);
    assert_eq!(runtime.card_hand()[0].skill_id, Some(31490111));
    assert_eq!(runtime.card_hand()[1].uid, Some(10));
    assert!(crate::engine::mechanic::card::CardMechanic
        .is_device_card(&runtime.managers, &runtime.card_hand()[1]));
    let removal = reply.fight_step.iter().flat_map(|step| &step.act_effect)
        .find(|effect| effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Cardremove as i32))
        .expect("client receives the selected card-removal animation");
    assert_eq!(removal.effect_num, Some(30010701));
    assert_eq!(removal.reserve_str.as_deref(), Some("1"));
}

#[test]
fn atonal_sequence_precast_can_be_played_by_its_conduit_owner() {
    use crate::dungeon::{BattleFighter, BattleRoster, FightOptions, HeroBuildInput};
    crate::test_support::init_config();
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let fighter = BattleFighter { hero: HeroBuildInput { uid: 30_000_001, user_id: 1,
        hero_id: 3149, skin: 314901, level: 180, rank: 4, ex_skill_level: 1,
        talent: 15, ..Default::default() }, equips: vec![] };
    let roster = BattleRoster { user_id: 1, fighters: vec![fighter], ..Default::default() };
    let group = sonettobuf::FightGroup { hero_list: vec![30_000_001], cloth_id: Some(7),
        ..Default::default() };
    let mut built = crate::dungeon::build_fight(catalog, &roster, 90000101, 9000011,
        &group, FightOptions::default(), None).unwrap();
    let rules = std::mem::take(&mut built.battle_rule_skills);
    let mut runtime = BattleRuntime::new_with_attributes(catalog, built.fight,
        built.ex_attributes, built.sp_attributes);
    runtime.extend_battle_rule_skills(rules);
    runtime.start_round_with_determinism(RoundDeterminism::with_seed(43)).unwrap();
    runtime.round_state.power = 99;
    runtime.fight.attacker.as_mut().unwrap().power = Some(99);
    runtime.use_cloth_skill(UseClothSkillRequest { skill_id: Some(30010702),
        from_id: Some(0), to_id: Some(0), r#type: Some(0), ..Default::default() }).unwrap();
    let index = runtime.card_hand().iter().position(|card| card.skill_id == Some(31446062)).unwrap();
    let round = runtime.build_begin_round_from_schedule(&BeginRoundRequest {
        opers: vec![BeginRoundOper { oper_type: Some(2), param1: Some(index as i32 + 1),
            ..Default::default() }], ..Default::default()
    }).unwrap();
    fn contains(step: &sonettobuf::FightStep) -> bool {
        step.act_id == Some(31446062) || step.act_effect.iter()
            .filter_map(|effect| effect.fight_step.as_ref()).any(contains)
    }
    assert!(round.fight_step.iter().any(contains), "generated energy must be usable");
    fn adds_chaotic_energy(step: &sonettobuf::FightStep) -> bool {
        step.act_effect.iter().any(|effect| {
            (effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Devicepowerchange as i32)
                && effect.reserve_str.as_deref() == Some("0#2"))
                || effect.fight_step.as_ref().is_some_and(adds_chaotic_energy)
        })
    }
    assert!(round.fight_step.iter().any(adds_chaotic_energy), "energy card must grant 2 chaotic energy");
}
