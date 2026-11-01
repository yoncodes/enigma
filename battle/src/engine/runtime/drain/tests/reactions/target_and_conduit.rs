use super::*;

#[test]
fn configured_action_target_owns_the_projected_skill_frame() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    ex_point: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    ex_point: Some(3),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(12),
                    ex_point: Some(2),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
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
    let run_case = |logic_target| {
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);
        let mut catalog = SkillEffectCatalog::default();
        catalog.insert(ParsedSkillEffect {
            skill_id: 100,
            slots: vec![SkillEffectSlot::new(
                ParsedBehavior::from_spec(
                    BehaviorSpec::new(20002, "AddExPoint"),
                    vec![1],
                    Vec::new(),
                ),
                TargetRequest::self_only(),
            )],
        });
        catalog.insert_logic_target(100, logic_target);
        let mut invocation: SkillInvocation = SkillRequest {
            source_uid: 10,
            skill_id: 100,
        }
        .into();
        invocation.target = SkillTarget::Explicit(-1);
        invocation.mode = SkillExecutionMode::Active;

        let result = run(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            [RuleOp::Skill(invocation)],
        )
        .unwrap();
        let action_target = result
            .outcomes
            .iter()
            .find_map(|outcome| match outcome {
                crate::engine::runtime::executor::RuleOutcome::SkillLifecycle(
                    crate::engine::skill::action::SkillLifecycle::ActionCompleted(action),
                ) => Some(action.target_uid),
                _ => None,
            })
            .expect("the active skill completes its action");
        (
            crate::engine::packet::timeline::project(&result.frames).unwrap()[0].to_id,
            action_target,
        )
    };

    assert_eq!(run_case(101), (Some(-1), -1));
    assert_eq!(run_case(112), (Some(11), 11));
    assert_eq!(run_case(130), (Some(-1), -1));
    assert_eq!(
        run_case(crate::engine::skill::target::request::SOURCE_TARGET_CODE),
        (Some(-1), -1)
    );
}

#[test]
fn main_ally_effects_keep_the_selected_target_on_reaction_frames() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
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
                FightEntityInfo {
                    passive_skill: vec![116330011],
                    ..entity(10)
                },
                entity(11),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31270121, 116330011], []);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 31270121,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    invocation.mode = SkillExecutionMode::Active;

    let result = run_action(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [],
        invocation,
    )
    .unwrap();
    let action = crate::engine::packet::timeline::project(&result.frames)
        .unwrap()
        .into_iter()
        .find(|step| step.act_id == Some(31270121))
        .expect("the source skill projects a step");
    assert_eq!(action.to_id, Some(-1));
    let reaction = action
        .act_effect
        .iter()
        .filter_map(|effect| effect.fight_step.as_ref())
        .find(|step| step.act_id == Some(116330011))
        .expect("the effect-tag reaction projects a nested step");
    assert_eq!(reaction.to_id, Some(-1));
    assert_eq!(
        reaction
            .act_effect
            .iter()
            .filter(|effect| {
                effect.buff.as_ref().and_then(|buff| buff.buff_id) == Some(116330004)
            })
            .filter_map(|effect| effect.target_id)
            .collect::<Vec<_>>(),
        vec![10, 11]
    );
}

#[test]
fn be_attacked_reaction_commits_before_same_hit_threshold() {
    crate::test_support::init_config();
    let entity = |uid, team_type, passive_skill| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        passive_skill,
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    buffs: vec![BuffInfo {
                        uid: Some(100),
                        buff_id: Some(31430151),
                        from_uid: Some(10),
                        layer: Some(4),
                        ..Default::default()
                    }],
                    ..entity(10, 1, vec![31430141, 31430151])
                },
                entity(11, 1, Vec::new()),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                skill_group1: vec![1163855063],
                ..entity(-1, 2, Vec::new())
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::SkillAction(crate::engine::skill::action::SkillActionEvent {
            card_index: 0,
            source_uid: 10,
            skill_id: 1163855063,
            target_uid: -1,
            target_uids: vec![-1],
            attacked_target_uids: vec![-1],
            phase: crate::engine::skill::action::SkillPhase::HitPassives,
            skill_slot: 1,
            is_attack: true,
            rank: 1,
            skill_type: 1,
            effect_tag: 1,
            assassinate: false,
            ignore_riposte: false,
            damage_amount: 1,
            kill_count: 0,
            crit_count: 0,
            guard_break_count: 0,
            additional_moxie: 0,
            extra_skill_kind: 0,
            mode: SkillExecutionMode::Active,
            teammate_injury_count: 0,
            teammate_injury_count_not_reset: 0,
            team_injury_count_round: 0,
            card_enchants: Vec::new(),
            buff_additions: Vec::new(),
        }),
    )
    .unwrap();
    assert_eq!(managers.buff.buff_id_amount(10, 31430151), 4);

    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: -1,
        skill_id: 1163855063,
    }
    .into();
    invocation.target = SkillTarget::Explicit(11);
    let result = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    fn contains_skill(
        frame: &crate::engine::runtime::record::SemanticFrame,
        skill_id: i32,
    ) -> bool {
        matches!(
            frame.owner,
            crate::engine::runtime::record::FrameOwner::Skill {
                skill_id: id,
                ..
            } if id == skill_id
        ) || frame.items.iter().any(|item| {
            matches!(
                item,
                crate::engine::runtime::record::FrameItem::Child(child)
                    if contains_skill(child, skill_id)
            )
        })
    }

    assert_eq!(managers.buff.buff_id_amount(10, 31430151), 0);
    assert!(
        result
            .frames
            .iter()
            .any(|frame| contains_skill(frame, 31430181))
    );
}

#[test]
fn twins_conduit_activation_consumes_chirp_signal_through_the_captured_passive() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(if uid == 10 { 3149 } else { 3001 }),
        team_type: Some(1),
        current_hp: Some(100),
        passive_skill: (uid == 10).then_some(116385685).into_iter().collect(),
        buffs: vec![BuffInfo {
            uid: Some(uid + 100),
            buff_id: Some(116385671),
            from_uid: Some(10),
            layer: Some(3),
            ..Default::default()
        }],
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10), entity(11)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    let result = run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 1,
            ..Default::default()
        },
        BattleEvent::ConduitActivated(crate::engine::event::payload::ConduitActivatedEvent {
            source_uid: 10,
            team: 1,
            skill_id: 31490011,
            power_id: 1,
            activation_cost: 10,
            spent: 10,
        }),
    )
    .unwrap();

    assert_eq!(
        managers.buff.snapshot(10, 110).and_then(|buff| buff.layer),
        Some(1)
    );
    assert_eq!(
        managers.buff.snapshot(11, 111).and_then(|buff| buff.layer),
        Some(1)
    );
    assert!(result.frames.iter().any(|frame| matches!(
        frame.owner,
        FrameOwner::Skill {
            skill_id: 116385685,
            ..
        }
    )));
}

#[test]
fn allied_conduit_activation_adds_configured_cost_to_passive_buff_layers() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3144),
                    team_type: Some(1),
                    current_hp: Some(100),
                    passive_skill: vec![31440141],
                    buffs: vec![BuffInfo {
                        uid: Some(100),
                        buff_id: Some(31440112),
                        from_uid: Some(10),
                        layer: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
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
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::ConduitActivated(crate::engine::event::payload::ConduitActivatedEvent {
            source_uid: 11,
            team: 1,
            skill_id: 31490121,
            power_id: 1,
            activation_cost: 3,
            spent: 2,
        }),
    )
    .unwrap();

    assert_eq!(managers.buff.buff_id_amount(10, 31440112), 13);
}

#[test]
fn contract_psychube_buffs_the_owner_then_the_selected_bound_ally() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3100),
                    team_type: Some(1),
                    current_hp: Some(100),
                    passive_skill: vec![433611],
                    buffs: vec![BuffInfo {
                        uid: Some(100),
                        buff_id: Some(31000221),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(30),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
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
    let mut managers = BattleManagers::seeded(&fight);
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60092, "NotifyHeroContract"),
    };
    managers
        .contract
        .execute(crate::engine::manager::contract::ContractCommand::Offer {
            origin,
            owner_uid: 10,
            candidates: vec![20, 30],
        })
        .unwrap();
    managers
        .contract
        .execute(
            crate::engine::manager::contract::ContractCommand::SelectOwner {
                owner_uid: 10,
                bound_uid: 20,
            },
        )
        .unwrap();
    managers
        .contract
        .execute(
            crate::engine::manager::contract::ContractCommand::SelectBound {
                owner_uid: 10,
                bound_uid: 20,
            },
        )
        .unwrap();
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    let result = run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::SkillAction(crate::engine::skill::action::SkillActionEvent {
            card_index: 0,
            source_uid: 10,
            skill_id: 31000451,
            target_uid: -1,
            target_uids: vec![-1],
            attacked_target_uids: vec![-1],
            phase: crate::engine::skill::action::SkillPhase::AfterHit,
            skill_slot: 1,
            is_attack: true,
            rank: 1,
            skill_type: 0,
            effect_tag: 1,
            assassinate: false,
            ignore_riposte: false,
            damage_amount: 0,
            kill_count: 0,
            crit_count: 0,
            guard_break_count: 0,
            additional_moxie: 0,
            extra_skill_kind: 0,
            mode: crate::engine::skill::action::SkillExecutionMode::Active,
            teammate_injury_count: 0,
            teammate_injury_count_not_reset: 0,
            team_injury_count_round: 0,
            card_enchants: Vec::new(),
            buff_additions: Vec::new(),
        }),
    )
    .unwrap();

    assert!(managers.buff.has_buff_id(10, 433621));
    assert!(managers.buff.has_buff_id(20, 433621));
    assert!(!managers.buff.has_buff_id(30, 433621));

    fn collect_targets(effect: &sonettobuf::ActEffect, targets: &mut Vec<i64>) {
        if effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Buffadd as i32)
            && effect.buff.as_ref().and_then(|buff| buff.buff_id) == Some(433621)
        {
            targets.push(effect.target_id.unwrap());
        }
        if let Some(step) = &effect.fight_step {
            for nested in &step.act_effect {
                collect_targets(nested, targets);
            }
        }
    }
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let mut targets = Vec::new();
    for effect in steps.iter().flat_map(|step| &step.act_effect) {
        collect_targets(effect, &mut targets);
    }
    assert_eq!(targets, vec![10, 20]);
}

#[test]
fn received_skill_rank_applies_only_its_configured_extra_burn() {
    crate::test_support::init_config();
    let burn_layers = |skill_id| {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3002),
                    team_type: Some(1),
                    current_hp: Some(10_000),
                    skill_group1: vec![30020111, 30020112, 30020113],
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
                    model_id: Some(161301),
                    team_type: Some(2),
                    current_hp: Some(10_000),
                    passive_skill: vec![1173002],
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
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);
        let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
        run_event(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            BattleEvent::Hit(crate::engine::event::payload::HitEvent {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(10005, "Damage"),
                },
                source_uid: 10,
                target_uid: -1,
                skill_id,
                amount: 100,
                resolved_damage: 100,
                shield_absorbed: 0,
                career_restraint: false,
                damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
                share_count: 0,
                assassinate: false,
                ignore_riposte: false,
            }),
        )
        .unwrap();
        (
            managers.buff.buff_id_amount(-1, 4150001),
            managers.buff.buff_id_amount(10, 4150001),
        )
    };

    assert_eq!(burn_layers(30020111), (3, 3));
    assert_eq!(burn_layers(30020112), (8, 3));
    assert_eq!(burn_layers(30020113), (8, 3));
}

#[test]
fn death_reaction_targets_the_killer() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3002),
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
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    model_id: Some(100101),
                    team_type: Some(2),
                    current_hp: Some(1),
                    passive_skill: vec![2345],
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    model_id: Some(100101),
                    team_type: Some(2),
                    current_hp: Some(10_000),
                    passive_skill: vec![2345],
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    let result = run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::EntityDied(crate::engine::event::payload::EntityDiedEvent {
            source_uid: 10,
            target_uid: -1,
        }),
    )
    .unwrap();

    assert_eq!(managers.buff.buff_id_amount(10, 5072), 1);
    assert_eq!(managers.buff.buff_id_amount(-1, 5072), 0);

    fn collect_death_frames(
        frame: &crate::engine::runtime::record::SemanticFrame,
        owners: &mut Vec<(i64, Option<i64>)>,
    ) {
        if let crate::engine::runtime::record::FrameOwner::Skill {
            source_uid,
            skill_id: 2345,
            target_uid,
            ..
        } = frame.owner
        {
            owners.push((source_uid, target_uid));
        }
        for item in &frame.items {
            if let crate::engine::runtime::record::FrameItem::Child(child) = item {
                collect_death_frames(child, owners);
            }
        }
    }

    let mut owners = Vec::new();
    for frame in &result.frames {
        collect_death_frames(frame, &mut owners);
    }
    assert_eq!(
        owners,
        vec![(crate::engine::fight::rules::DEFENDER_SIDE_UID, Some(-1))]
    );
}

#[test]
fn moxie_readiness_survives_another_owners_skill_rewrite() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(1),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ex_point: Some(5),
                    ex_skill: Some(900),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    model_id: Some(2),
                    team_type: Some(1),
                    current_hp: Some(100),
                    skill_group1: vec![101],
                    skill_group2: vec![102],
                    ..Default::default()
                },
            ],
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
                    skill_id: Some(900),
                    ..Default::default()
                },
                CardInfo {
                    uid: Some(11),
                    skill_id: Some(101),
                    ..Default::default()
                },
            ],
            draw_pile: Vec::new(),
            deck_num: 1,
        }))
        .unwrap();
    let origin = CommandOrigin {
        domain: RuleDomain::Lifecycle,
        key: DefinitionKey::new(0, "UltimateAvailabilityTest"),
    };

    let rewrite = run(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Command(BattleCommand::Card(
            CardCommand::ReplaceOwnerSkills(CardReplaceOwnerSkills {
                origin,
                owner_uid: 11,
                base_group1: vec![101],
                base_group2: vec![102],
                replacement_group1: vec![201],
                replacement_group2: vec![202],
            }),
        ))],
    )
    .unwrap();

    let cards = rewrite
        .outcomes
        .iter()
        .find_map(|outcome| match outcome {
            RuleOutcome::Card(changes) => Some(&changes.after),
            _ => None,
        })
        .unwrap();
    assert!(
        cards
            .iter()
            .any(|card| card.uid == Some(10) && card.skill_id == Some(900))
    );
    assert!(
        cards
            .iter()
            .any(|card| card.uid == Some(11) && card.skill_id == Some(201))
    );

    run(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Command(BattleCommand::ExPoint(
            ExPointCommand::Change(ExPointChange {
                origin,
                source_uid: 10,
                target_uid: 10,
                delta: -1,
                config_effect: 0,
                effect_type: 0,
            }),
        ))],
    )
    .unwrap();

    assert!(
        managers
            .card
            .hand()
            .iter()
            .all(|card| card.uid != Some(10) || card.skill_id != Some(900))
    );
    assert!(
        managers
            .card
            .refilled()
            .iter()
            .all(|card| card.uid != Some(10) || card.skill_id != Some(900))
    );
}
