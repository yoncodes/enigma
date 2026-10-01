use super::*;

#[test]
fn compiled_setup_schedule_owns_stage_order() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ex_point: Some(0),
                passive_skill: vec![100],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 5,
        type_name: "EnterFight".to_owned(),
        kind: ParsedConditionKind::Lifecycle(LifecycleMode::EnterFight),
        raw_args: Vec::new(),
    }];
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);
    let mut battle_start = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![2], Vec::new()),
        TargetRequest::self_only(),
    );
    battle_start.conditions = vec![ParsedCondition {
        opcode: 5021,
        type_name: "EnterFight".to_owned(),
        kind: ParsedConditionKind::Lifecycle(LifecycleMode::BattleStart),
        raw_args: Vec::new(),
    }];
    battle_start.compiled_route = ConditionRoute::compile(&battle_start.conditions);

    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![slot, battle_start],
    });

    let result = run_setup_schedule(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        crate::engine::runtime::schedule::START,
    )
    .unwrap();

    assert!(matches!(
        result.outcomes.as_slice(),
        [
            RuleOutcome::ExPoint(crate::engine::manager::ex_point::ExPointChanges::Value {
                change: battle_start,
                ..
            }),
            RuleOutcome::ExPoint(crate::engine::manager::ex_point::ExPointChanges::Value {
                change: enter_fight,
                ..
            })
        ] if battle_start.applied_delta == 2 && enter_fight.applied_delta == 1
    ));
    assert!(
        result
            .frames
            .iter()
            .all(|frame| matches!(frame.owner, FrameOwner::SetupSide(SetupSide::Attacker)))
    );
    for frame in &result.frames {
        let [crate::engine::runtime::record::FrameItem::Child(entity)] = frame.items.as_slice()
        else {
            panic!("setup stage should contain one entity frame")
        };
        assert!(matches!(
            entity.owner,
            FrameOwner::SetupEntity { owner_uid: 10 }
        ));
        assert!(matches!(
            entity.items.as_slice(),
            [crate::engine::runtime::record::FrameItem::Child(skill)]
                if matches!(skill.owner, FrameOwner::Skill { source_uid: 10, skill_id: 100, .. })
        ));
    }
}

#[test]
fn exact_setup_routes_share_their_skill_activation_frame() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ex_point: Some(0),
                passive_skill: vec![100],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let slot = |opcode, type_name: &str, args: &[String], amount| {
        let mut slot = SkillEffectSlot::new(
            ParsedBehavior::from_spec(
                BehaviorSpec::new(20002, "AddExPoint"),
                vec![amount],
                Vec::new(),
            ),
            TargetRequest::self_only(),
        );
        slot.conditions = vec![ParsedCondition {
            opcode,
            type_name: type_name.to_owned(),
            kind: crate::engine::skill::condition::registry::parse(opcode, type_name, args)
                .expect("test condition is registered"),
            raw_args: args.to_vec(),
        }];
        slot.compiled_route = ConditionRoute::compile(&slot.conditions);
        slot
    };
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![
            slot(5, "EnterFight", &[], 1),
            slot(57002, "NoBuffId", &["999".into()], 2),
        ],
    });

    let result = run_setup_stage(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        SetupStage::EnterFight,
        0,
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(10), 3);
    let [crate::engine::runtime::record::FrameItem::Child(entity)] =
        result.frames[0].items.as_slice()
    else {
        panic!("setup stage should contain one entity frame")
    };
    let [crate::engine::runtime::record::FrameItem::Child(skill)] = entity.items.as_slice() else {
        panic!("exact routes from one skill should share one skill frame")
    };
    assert!(matches!(
        skill.owner,
        FrameOwner::Skill {
            source_uid: 10,
            skill_id: 100,
            ..
        }
    ));
    assert_eq!(
        skill
            .items
            .iter()
            .filter(|item| matches!(item, crate::engine::runtime::record::FrameItem::Change(_)))
            .count(),
        2
    );
}

#[test]
fn transform_commit_publishes_the_exact_enter_fight_route() {
    crate::test_support::init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(30111001),
                team_type: Some(2),
                position: Some(1),
                current_hp: Some(100),
                ex_point: Some(0),
                attr: Some(HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 5,
        type_name: "EnterFight".to_owned(),
        kind: ParsedConditionKind::Lifecycle(LifecycleMode::EnterFight),
        raw_args: Vec::new(),
    }];
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);
    let transformed =
        crate::engine::fight::defender::Defender::build_monster_with_uid(30111005, -1, 1, 2)
            .unwrap();
    let mut catalog = SkillEffectCatalog::from_roots(
        config::configs::get(),
        transformed.passive_skill,
        std::iter::empty(),
    );
    catalog.insert(ParsedSkillEffect {
        skill_id: 530000741,
        slots: vec![slot],
    });

    let result = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Command(BattleCommand::Entity(
            crate::engine::manager::entity::EntityCommand {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(40006, "MonsterChange"),
                },
                source_uid: -1,
                target_uid: -1,
                operation: crate::engine::manager::entity::EntityOperation::Transform {
                    model_id: 30111005,
                    parameters: [1000, 0],
                },
            },
        ))],
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(-1), 1);
    assert!(
        result
            .events
            .contains(&BattleEvent::EntityTransformed { target_uid: -1 })
    );
    fn ordered_changes<'a>(
        frame: &'a crate::engine::runtime::record::SemanticFrame,
        changes: &mut Vec<&'a BattleChange>,
    ) {
        for item in &frame.items {
            match item {
                crate::engine::runtime::record::FrameItem::Change(change) => changes.push(change),
                crate::engine::runtime::record::FrameItem::Child(child) => {
                    ordered_changes(child, changes)
                }
                crate::engine::runtime::record::FrameItem::Cue(_) => {}
            }
        }
    }
    let mut changes = Vec::new();
    for frame in &result.frames {
        ordered_changes(frame, &mut changes);
    }
    let transform_index = changes
        .iter()
        .position(|change| matches!(change, BattleChange::Entity(_)))
        .unwrap();
    let reactivated_index = changes
        .iter()
        .position(|change| matches!(change, BattleChange::ExPoint(_)))
        .unwrap();
    assert!(transform_index < reactivated_index);
}

#[test]
fn battle_start_tag_threshold_applies_the_configured_team_buff() {
    crate::test_support::init_config();
    let entity = |uid, model_id, passive_skill| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        entity_type: Some(1),
        current_hp: Some(100),
        passive_skill,
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(10, 3086, Vec::new()),
                entity(11, 3107, Vec::new()),
                entity(12, 3128, Vec::new()),
                entity(13, 3143, vec![31_430_151]),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    run_setup_stage(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        SetupStage::BattleStart,
        0,
    )
    .unwrap();

    for uid in 10..=13 {
        assert!(
            managers
                .buff
                .active_for(uid)
                .any(|buff| buff.buff_id == Some(31_430_154)),
            "team member {uid} should receive the configured battle-tag buff"
        );
    }
}

#[test]
fn battle_start_tag_check_applies_the_support_role_buff() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3146),
                entity_type: Some(1),
                current_hp: Some(100),
                passive_skill: vec![103_335_200],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    run_setup_stage(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        SetupStage::BattleStart,
        0,
    )
    .unwrap();

    assert!(
        managers
            .buff
            .active_for(10)
            .any(|buff| buff.buff_id == Some(103_335_120))
    );
}

#[test]
fn grant_time_buff_acts_stay_in_the_granting_skill_frame() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                passive_skill: vec![31140141],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);

    let result = run_setup_stage(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        SetupStage::EnterFight,
        0,
    )
    .unwrap();

    let [crate::engine::runtime::record::FrameItem::Child(entity)] =
        result.frames[0].items.as_slice()
    else {
        panic!("expected setup entity frame")
    };
    let [crate::engine::runtime::record::FrameItem::Child(skill)] = entity.items.as_slice() else {
        panic!("expected one granting skill frame")
    };
    assert!(matches!(
        skill.owner,
        FrameOwner::Skill {
            source_uid: 10,
            skill_id: 31140141,
            ..
        }
    ));
    assert!(skill.items.iter().any(|item| matches!(
        item,
        crate::engine::runtime::record::FrameItem::Change(change)
            if matches!(change.as_ref(), BattleChange::Buff(_))
    )));
    assert!(skill.items.iter().any(|item| matches!(
        item,
        crate::engine::runtime::record::FrameItem::Change(change)
            if matches!(
                change.as_ref(),
                BattleChange::ExPoint(
                    crate::engine::manager::ex_point::ExPointChanges::Max { .. }
                )
            )
    )));
}

#[test]
fn repeated_power_skill_interleaves_each_spend_with_its_cast() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3117),
                current_hp: Some(100),
                attr: Some(HeroAttribute {
                    hp: Some(100),
                    attack: Some(100),
                    ..Default::default()
                }),
                passive_skill: vec![311701433],
                power_infos: vec![PowerInfo {
                    power_id: Some(EUREKA_RESOURCE_ID),
                    num: Some(4),
                    max: Some(5),
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);

    let result = run_setup_stage(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        SetupStage::RoundStart,
        3,
    )
    .unwrap();

    fn collect(effects: &[sonettobuf::ActEffect], sequence: &mut Vec<&'static str>) {
        for effect in effects {
            if effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Powerchange as i32)
                && effect.effect_num == Some(-2)
            {
                sequence.push("spend");
            }
            if let Some(step) = &effect.fight_step {
                if step.act_id == Some(311701210) {
                    sequence.push("cast");
                }
                collect(&step.act_effect, sequence);
            }
        }
    }

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let mut sequence = Vec::new();
    for step in &steps {
        collect(&step.act_effect, &mut sequence);
    }
    assert_eq!(sequence, ["spend", "cast", "spend", "cast"]);
}

#[test]
fn channel_round_start_casts_once_per_converted_incantation() {
    crate::test_support::init_config();
    // Returns the cards left in hand, the casts, and whether the enemy was hit.
    let run = |channel_duration: i32| {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3105),
                    current_hp: Some(100),
                    attr: Some(HeroAttribute {
                        hp: Some(100),
                        attack: Some(100),
                        ..Default::default()
                    }),
                    skill_group1: vec![31050111, 31050112],
                    skill_group2: vec![31050121, 31050122],
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(31050131),
                        from_uid: Some(10),
                        duration: Some(channel_duration),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
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
        };
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);
        let card = |skill_id| CardInfo {
            uid: Some(10),
            skill_id: Some(skill_id),
            ..Default::default()
        };
        managers
            .execute_card(CardCommand::Setup(CardSetup {
                hand: vec![card(31050111), card(31050121)],
                draw_pile: Vec::new(),
                deck_num: 0,
            }))
            .unwrap();
        let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

        let result = run_buff_act_setup_stage_for_owners(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            SetupStage::RoundStart,
            2,
            &[10],
        )
        .unwrap();

        fn casts(effects: &[sonettobuf::ActEffect]) -> usize {
            effects
                .iter()
                .filter_map(|effect| effect.fight_step.as_ref())
                .map(|step| usize::from(step.act_id == Some(31050151)) + casts(&step.act_effect))
                .sum()
        }
        let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
        (
            managers.card.hand().len(),
            steps
                .iter()
                .map(|step| casts(&step.act_effect))
                .sum::<usize>(),
            managers.hp.current(-1) < 100_000,
        )
    };

    assert_eq!(run(2), (0, 2, true));
}
