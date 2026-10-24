use super::*;
use crate::engine::manager::buff::BuffGrant;

#[test]
fn final_settlement_advances_final_stage_durations() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(109320002),
                    duration: Some(1),
                    from_uid: Some(10),
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

    let result = run_round_end_final_settlement(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        Vec::new(),
    )
    .unwrap();

    assert!(managers.buff.snapshot(10, 20).is_none());
    let settled = result
        .events
        .iter()
        .position(|event| matches!(event, BattleEvent::BuffsSettled(_)))
        .unwrap();
    let (removed, duration_expired) = result
        .events
        .iter()
        .enumerate()
        .find_map(|(index, event)| match event {
            BattleEvent::BuffRemoved(change) if change.buff_id == 109320002 => {
                Some((index, change.duration_expired))
            }
            _ => None,
        })
        .unwrap();
    assert!(settled < removed);
    assert!(duration_expired);
}

#[test]
fn obscurity_expiry_reactions_are_sibling_skill_frames() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(10_000),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(100_000),
                passive_skill: vec![109320103, 109320104],
                buffs: vec![BuffInfo {
                    uid: Some(21),
                    buff_id: Some(109320101),
                    duration: Some(2),
                    from_uid: Some(-1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .buff
        .execute(
            &managers.hp,
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: -1,
                target_uid: crate::engine::fight::rules::ATTACKER_SIDE_UID,
                buff_id: 109320002,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();

    let result = run_round_end_final_settlement(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        Vec::new(),
    )
    .unwrap();

    assert!(!managers.buff.has_active_buff_id(-1, 109320101));
    assert!(managers.buff.has_active_buff_id(-1, 109320102));

    fn has_sibling_skills(
        frame: &crate::engine::runtime::record::SemanticFrame,
        expected: [i32; 2],
    ) -> bool {
        let skill_ids = frame
            .items
            .iter()
            .filter_map(|item| match item {
                FrameItem::Child(child) => match child.owner {
                    FrameOwner::Skill { skill_id, .. } => Some(skill_id),
                    _ => None,
                },
                FrameItem::Change(_) | FrameItem::Cue(_) => None,
            })
            .collect::<Vec<_>>();
        expected.iter().all(|skill_id| skill_ids.contains(skill_id))
            || frame.items.iter().any(|item| match item {
                FrameItem::Child(child) => has_sibling_skills(child, expected),
                FrameItem::Change(_) | FrameItem::Cue(_) => false,
            })
    }

    assert!(
        result
            .frames
            .iter()
            .any(|frame| has_sibling_skills(frame, [109320103, 109320104]))
    );
}

#[test]
fn blood_debt_replaces_its_expired_defense_debuff_after_settlement() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    hp: Some(10_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(100_000),
                attr: Some(HeroAttribute {
                    hp: Some(100_000),
                    mdefense: Some(1_400),
                    ..Default::default()
                }),
                buffs: vec![
                    BuffInfo {
                        uid: Some(20),
                        buff_id: Some(116330041),
                        duration: Some(2),
                        from_uid: Some(-1),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(21),
                        buff_id: Some(116330043),
                        duration: Some(1),
                        from_uid: Some(-1),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(22),
                        buff_id: Some(116330047),
                        duration: Some(2),
                        from_uid: Some(-1),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);

    run_after_ai_round_end(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
    )
    .unwrap();

    assert!(managers.buff.snapshot(-1, 21).is_none());
    assert_eq!(managers.buff.buff_id_amount(-1, 116330043), 1);
    assert_eq!(
        crate::engine::manager::persistent_attribute_delta(
            &managers.buff,
            &managers.hp,
            -1,
            crate::engine::entity::attr::AttrId::MentalDef,
        ),
        -300
    );
}

#[test]
fn obscurity_break_enters_the_witness_round_on_the_next_round() {
    init_config();
    let entity = |uid, buffs, passive_skill| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(10_000),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            ..Default::default()
        }),
        buffs,
        passive_skill,
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, Vec::new(), Vec::new())],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(
                -1,
                vec![BuffInfo {
                    uid: Some(21),
                    buff_id: Some(109320101),
                    duration: Some(2),
                    from_uid: Some(-1),
                    ..Default::default()
                }],
                vec![109320102, 109320105, 109320112],
            )],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .buff
        .execute(
            &managers.hp,
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: -1,
                target_uid: crate::engine::fight::rules::ATTACKER_SIDE_UID,
                buff_id: 109320001,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();
    let obscurity_uid = managers
        .buff
        .active_for(crate::engine::fight::rules::ATTACKER_SIDE_UID)
        .find(|buff| buff.buff_id == Some(109320001))
        .unwrap()
        .uid
        .unwrap();

    let result = run_round_end_final_settlement(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        Vec::new(),
    )
    .unwrap();

    // "When Obscurity reaches zero, you'll enter a bonus Witness Round during the following
    // round", in which "enemies cannot take active actions".
    assert!(
        !managers
            .buff
            .has_active_buff_id(crate::engine::fight::rules::ATTACKER_SIDE_UID, 109320001)
    );
    assert!(
        managers
            .buff
            .has_active_buff_id(crate::engine::fight::rules::ATTACKER_SIDE_UID, 109320002)
    );
    assert!(managers.buff.has_active_buff_id(-1, 109320103));
    assert_eq!(managers.buff.snapshot(-1, 21).unwrap().duration, Some(1));
    let marker = managers
        .buff
        .active_for(-1)
        .find(|buff| buff.buff_id == Some(109320112))
        .unwrap();
    assert_eq!(marker.duration, Some(1));

    fn collect_duration_owners(
        frame: &crate::engine::runtime::record::SemanticFrame,
        owners: &mut Vec<(i64, i64, i32)>,
    ) {
        if let FrameOwner::BuffDuration {
            owner_uid,
            buff_uid,
            take_stage,
        } = &frame.owner
        {
            owners.push((*owner_uid, *buff_uid, *take_stage));
        }
        for item in &frame.items {
            if let FrameItem::Child(child) = item {
                collect_duration_owners(child, owners);
            }
        }
    }
    let mut owners = Vec::new();
    for frame in &result.frames {
        collect_duration_owners(frame, &mut owners);
    }
    assert_eq!(
        owners,
        vec![
            (
                crate::engine::fight::rules::ATTACKER_SIDE_UID,
                obscurity_uid,
                effect_time::ROUND_END_FINAL_SETTLEMENT
            ),
            (-1, 21, effect_time::ROUND_END_FINAL_SETTLEMENT),
            (
                -1,
                marker.uid.unwrap(),
                effect_time::ROUND_END_FINAL_SETTLEMENT
            ),
        ]
    );

    let steps = crate::engine::packet::timeline::project_for_version(&result.frames, 7).unwrap();
    assert_eq!(steps.len(), 2);
    assert_eq!(
        steps
            .iter()
            .map(|step| {
                step.act_effect
                    .iter()
                    .map(|effect| effect.effect_type.unwrap())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>(),
        vec![vec![6, 162, 162], vec![7, 7, 162]]
    );
}

#[test]
fn attacker_settlement_groups_duration_expiry_before_owner_reactions() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    hp: Some(10_000),
                    ..Default::default()
                }),
                passive_skill: vec![433911],
                buffs: vec![
                    BuffInfo {
                        uid: Some(20),
                        buff_id: Some(109380002),
                        duration: Some(1),
                        from_uid: Some(-1),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(21),
                        buff_id: Some(433911),
                        layer: Some(3),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);

    let result = run_entity_settlement(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[10],
        SettlementSide::Attacker,
    )
    .unwrap();

    assert!(managers.buff.snapshot(10, 20).is_none());
    assert_eq!(managers.buff.buff_id_amount(10, 433911), 2);
    assert_eq!(
        result
            .output
            .frames
            .iter()
            .filter(|frame| matches!(
                frame.owner,
                FrameOwner::RoundPhase(RoundPhase::EntitySettlement)
            ))
            .count(),
        1
    );
    let steps =
        crate::engine::packet::timeline::project_for_version(&result.output.frames, 7).unwrap();

    assert_eq!(steps.len(), 1);
    assert_eq!(
        steps[0]
            .act_effect
            .iter()
            .map(|effect| effect.effect_type.unwrap())
            .collect::<Vec<_>>(),
        vec![
            sonettobuf::effect_type_enum::EffectType::Buffdel as i32,
            sonettobuf::effect_type_enum::EffectType::Fightstep as i32,
        ]
    );
}

#[test]
fn defender_round_end_keeps_buff_acts_and_owner_passives_in_one_frame() {
    init_config();
    let burn = |uid| BuffInfo {
        uid: Some(uid),
        buff_id: Some(4150001),
        count: Some(1),
        layer: Some(1),
        from_uid: Some(10),
        duration: Some(2),
        ..Default::default()
    };
    let defender = |uid, passive_skill, buff_uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(120_000),
        attr: Some(HeroAttribute {
            attack: Some(if uid == -1 { 2_000 } else { 1_000 }),
            hp: Some(120_000),
            ..Default::default()
        }),
        passive_skill,
        buffs: vec![burn(buff_uid)],
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    attack: Some(1_000),
                    hp: Some(10_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                defender(-1, Vec::new(), 31),
                defender(-2, vec![116330101], 32),
                defender(-3, vec![116330101], 33),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);

    let result = run_after_ai_round_end(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
    )
    .unwrap();
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let round_end = steps
        .iter()
        .find(|step| {
            step.act_effect
                .iter()
                .filter(|effect| {
                    effect
                        .fight_step
                        .as_ref()
                        .is_some_and(|step| step.act_id == Some(4150001))
                })
                .count()
                == 3
        })
        .unwrap();
    assert_eq!(round_end.act_effect.len(), 5);

    let passives = round_end.act_effect[3..]
        .iter()
        .map(|effect| {
            effect.fight_step.as_ref().unwrap().act_effect[0]
                .fight_step
                .as_ref()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        passives
            .iter()
            .map(|step| (step.from_id, step.to_id, step.act_id))
            .collect::<Vec<_>>(),
        vec![
            (Some(-2), Some(-2), Some(116330101)),
            (Some(-3), Some(-3), Some(116330101)),
        ]
    );
    let buffs = passives
        .iter()
        .map(|step| {
            step.act_effect
                .iter()
                .find_map(|effect| effect.buff.as_ref())
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        buffs
            .iter()
            .map(|buff| (buff.buff_id, buff.layer))
            .collect::<Vec<_>>(),
        vec![(Some(116330002), Some(1)), (Some(116330002), Some(2))]
    );
    assert_eq!(buffs[0].uid, buffs[1].uid);
    assert_eq!(managers.buff.buff_id_amount(-1, 116330002), 2);
    assert_eq!(managers.buff.buff_id_amount(-2, 116330002), 0);
    assert_eq!(managers.buff.buff_id_amount(-3, 116330002), 0);
}

#[test]
fn defender_killed_by_settlement_does_not_run_later_owner_events() {
    init_config();
    let burn = |uid| BuffInfo {
        uid: Some(uid),
        buff_id: Some(4150001),
        count: Some(1),
        layer: Some(1),
        from_uid: Some(10),
        duration: Some(2),
        ..Default::default()
    };
    let defender = |uid, current_hp, passive_skill, buffs| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(current_hp),
        attr: Some(HeroAttribute {
            attack: Some(1_000),
            hp: Some(120_000),
            ..Default::default()
        }),
        passive_skill,
        buffs,
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    attack: Some(1_000),
                    hp: Some(10_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                defender(-1, 120_000, Vec::new(), Vec::new()),
                defender(-2, 120_000, vec![100], vec![burn(31)]),
                defender(-3, 1, vec![100], vec![burn(32)]),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::new(1, "AddBuff", vec![101]),
        TargetRequest::self_only(),
    );
    slot.conditions =
        crate::engine::skill::condition::parse::parse_conditions(config::configs::get(), "303");
    slot.compiled_route =
        ConditionRoute::compile_for_behavior(&slot.conditions, &slot.behavior.spec);
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![slot],
    });
    let mut managers = BattleManagers::seeded(&fight);

    run_after_ai_round_end(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
    )
    .unwrap();

    assert_eq!(managers.hp.current(-3), 0);
    assert_eq!(managers.buff.buff_id_amount(-3, 4150001), 1);
    assert!(managers.buff.has_buff_id(-2, 101));
    assert!(!managers.buff.has_buff_id(-3, 101));
}

#[test]
fn defender_round_end_replaces_the_defeated_third_add() {
    init_config();
    let defender = |uid, model_id, position, current_hp, passive_skill| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        team_type: Some(2),
        position: Some(position),
        current_hp: Some(current_hp),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            ..Default::default()
        }),
        passive_skill,
        ..Default::default()
    };
    let fight = Fight {
        battle_id: Some(116330001),
        cur_wave: Some(1),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
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
        defender: Some(FightTeam {
            entitys: vec![
                defender(-1, 116330001, 1, 100_000, vec![116330031]),
                defender(-2, 116330100, 2, 100_000, Vec::new()),
                defender(-3, 116330100, 3, 0, Vec::new()),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);

    run_after_ai_round_end(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
    )
    .unwrap();

    let summoned = managers.entity.snapshot(-4).unwrap();
    assert_eq!(summoned.model_id, Some(116330100));
    assert_eq!(summoned.position, Some(3));
    assert!(managers.entity.snapshot(-5).is_none());
}
