use super::*;

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
                shield_absorbed: 0,
                career_restraint: false,
                damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
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
            entitys: vec![FightEntityInfo {
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
        BattleEvent::EntityDied(crate::engine::event::payload::EntityDiedEvent {
            source_uid: 10,
            target_uid: -1,
        }),
    )
    .unwrap();

    assert_eq!(managers.buff.buff_id_amount(10, 5072), 1);
    assert_eq!(managers.buff.buff_id_amount(-1, 5072), 0);
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

#[test]
fn bendith_buff_drain_projects_add_remove_and_recast_in_capture_order() {
    crate::test_support::init_config();
    let base_group1 = vec![31460114, 31460115, 31460116];
    let base_group2 = vec![31460127, 31460128, 31460129];
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3146),
                team_type: Some(1),
                current_hp: Some(100),
                skill_group1: base_group1,
                skill_group2: base_group2,
                ..Default::default()
            }],
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
                    hero_id: Some(3146),
                    skill_id: Some(31460114),
                    ..Default::default()
                },
                CardInfo {
                    uid: Some(10),
                    hero_id: Some(3146),
                    skill_id: Some(31460127),
                    ..Default::default()
                },
            ],
            draw_pile: Vec::new(),
            deck_num: 2,
        }))
        .unwrap();
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60001, "AddBuff"),
    };
    let grant = || {
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: 10,
            target_uid: 10,
            buff_id: 31460137,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        })))
    };
    let remove = || {
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
            origin,
            target_uid: 10,
            selector: BuffRemoveSelector::ExactId(31460137),
        })))
    };
    let projected_types = |result: &crate::engine::runtime::drain::DrainResult| {
        fn collect(effect: &sonettobuf::ActEffect, types: &mut Vec<i32>) {
            let relevant = [
                sonettobuf::effect_type_enum::EffectType::Cardaconvertcardb as i32,
                sonettobuf::effect_type_enum::EffectType::Heroupgrade as i32,
                sonettobuf::effect_type_enum::EffectType::Buffadd as i32,
                sonettobuf::effect_type_enum::EffectType::Buffdel as i32,
            ];
            if effect
                .effect_type
                .is_some_and(|kind| relevant.contains(&kind))
            {
                types.push(effect.effect_type.unwrap());
            }
            if let Some(step) = &effect.fight_step {
                for nested in &step.act_effect {
                    collect(nested, types);
                }
            }
        }

        let mut types = Vec::new();
        for effect in crate::engine::packet::timeline::project(&result.frames)
            .unwrap()
            .iter()
            .flat_map(|step| &step.act_effect)
        {
            collect(effect, &mut types);
        }
        types
    };
    let converted = sonettobuf::effect_type_enum::EffectType::Cardaconvertcardb as i32;
    let upgraded = sonettobuf::effect_type_enum::EffectType::Heroupgrade as i32;
    let added = sonettobuf::effect_type_enum::EffectType::Buffadd as i32;
    let deleted = sonettobuf::effect_type_enum::EffectType::Buffdel as i32;

    let add = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [grant()],
    )
    .unwrap();
    assert_eq!(
        projected_types(&add),
        vec![converted, converted, upgraded, added]
    );

    let recast = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [remove(), grant()],
    )
    .unwrap();
    assert_eq!(
        projected_types(&recast),
        vec![
            converted, converted, upgraded, deleted, converted, converted, upgraded, added,
        ]
    );

    let remove = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [remove()],
    )
    .unwrap();
    assert_eq!(
        projected_types(&remove),
        vec![converted, converted, upgraded, deleted]
    );
}

#[test]
fn rapport_debuff_projects_add_remove_and_recast_in_capture_order() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3146),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(31460003),
                    layer: Some(3),
                    from_uid: Some(10),
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
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(-1),
                    buff_id: Some(31460003),
                    layer: Some(8),
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
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "AddBuff"),
    };
    let grant = || {
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: 10,
            target_uid: -1,
            buff_id: 31460212,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        })))
    };
    let remove = || {
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
            origin,
            target_uid: -1,
            selector: BuffRemoveSelector::ExactId(31460212),
        })))
    };
    let projected_types = |result: &crate::engine::runtime::drain::DrainResult| {
        fn collect(effect: &sonettobuf::ActEffect, types: &mut Vec<i32>) {
            let relevant = [
                sonettobuf::effect_type_enum::EffectType::Buffadd as i32,
                sonettobuf::effect_type_enum::EffectType::Buffdel as i32,
                sonettobuf::effect_type_enum::EffectType::Attr as i32,
            ];
            if effect
                .effect_type
                .is_some_and(|kind| relevant.contains(&kind))
            {
                types.push(effect.effect_type.unwrap());
            }
            if let Some(step) = &effect.fight_step {
                for nested in &step.act_effect {
                    collect(nested, types);
                }
            }
        }

        let mut types = Vec::new();
        for effect in crate::engine::packet::timeline::project(&result.frames)
            .unwrap()
            .iter()
            .flat_map(|step| &step.act_effect)
        {
            collect(effect, &mut types);
        }
        types
    };
    let added = sonettobuf::effect_type_enum::EffectType::Buffadd as i32;
    let deleted = sonettobuf::effect_type_enum::EffectType::Buffdel as i32;
    let attr = sonettobuf::effect_type_enum::EffectType::Attr as i32;

    let add = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [grant()],
    )
    .unwrap();
    assert_eq!(projected_types(&add), vec![added, attr]);
    assert_eq!(
        managers.persistent_attribute_delta(-1, AttrId::CriticalDef),
        -240
    );

    let recast = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [remove(), grant()],
    )
    .unwrap();
    assert_eq!(projected_types(&recast), vec![deleted, added, attr]);
    assert_eq!(
        managers.persistent_attribute_delta(-1, AttrId::CriticalDef),
        -240
    );
}

#[test]
fn moxie_gain_waits_for_the_normal_card_refill() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ex_point: Some(4),
                ex_skill: Some(900),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![CardInfo {
                uid: Some(10),
                skill_id: Some(101),
                ..Default::default()
            }],
            draw_pile: Vec::new(),
            deck_num: 1,
        }))
        .unwrap();

    run(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Command(BattleCommand::ExPoint(
            ExPointCommand::Change(ExPointChange {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "UltimateRefillTest"),
                },
                source_uid: 10,
                target_uid: 10,
                delta: 1,
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
            .all(|card| card.skill_id != Some(900))
    );
}

#[test]
fn assist_boss_attack_passive_resolves_from_the_derived_skill_cast_event() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            assist_boss: Some(FightEntityInfo {
                uid: Some(-1),
                team_type: Some(1),
                current_hp: Some(999_999),
                attr: Some(HeroAttribute {
                    hp: Some(999_999),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                passive_skill: vec![12_720_012],
                ..Default::default()
            }),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: [-2, -3]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    team_type: Some(2),
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let event = BattleEvent::SkillAction(crate::engine::skill::action::SkillActionEvent {
        source_uid: -1,
        skill_id: 370_001_002,
        target_uid: -2,
        target_uids: vec![-2, -3],
        attacked_target_uids: vec![-2, -3],
        phase: crate::engine::skill::action::SkillPhase::HitPassives,
        skill_slot: -1,
        is_attack: true,
        rank: 1,
        skill_type: 0,
        effect_tag: 2,
        assassinate: false,
        ignore_riposte: false,
        damage_amount: 1,
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
    });

    run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        event,
    )
    .unwrap();

    assert!(managers.hp.current(-2) < 10_000);
    assert!(managers.hp.current(-3) < 10_000);
}

#[test]
fn random_additional_target_passive_expands_the_configured_attack() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            assist_boss: Some(FightEntityInfo {
                uid: Some(-1),
                team_type: Some(1),
                current_hp: Some(999_999),
                attr: Some(HeroAttribute {
                    hp: Some(999_999),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                passive_skill: vec![370_002_190],
                ..Default::default()
            }),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: [-2, -3]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    team_type: Some(2),
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_condition_random_choices(vec![
        crate::engine::runtime::determinism::ConditionRandomChoice {
            skill_id: 370_001_002,
            opcode: 552_203,
            roll: 499,
        },
    ]);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: -1,
        skill_id: 370_001_002,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-2);

    let result = run(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut determinism,
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    assert!(managers.hp.current(-2) < 10_000);
    assert!(
        managers.hp.current(-3) < 10_000,
        "marker={} frames={:#?}",
        managers.buff.has_buff_id(-1, 370_002_190),
        result.frames
    );
    assert!(!managers.buff.has_buff_id(-1, 370_002_190));
}

#[test]
fn active_skill_publishes_exact_phase_to_its_psychube_passive() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3107),
                    team_type: Some(1),
                    career: Some(6),
                    current_hp: Some(100),
                    skill_group1: vec![31070111],
                    passive_skill: vec![435011],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    model_id: Some(3074),
                    team_type: Some(1),
                    current_hp: Some(100),
                    passive_skill: vec![2270001],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                career: Some(6),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 31070111,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    invocation.card_index = 1;

    let result = run(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    assert!(managers.buff.has_buff_id(10, 31070111));
    assert!(managers.buff.has_buff_id(10, 435011));
    assert!(!managers.buff.has_buff_id(11, 90071));
    assert_eq!(
        managers
            .buff
            .active_for(10)
            .find(|buff| buff.buff_id == Some(31070111))
            .and_then(|buff| buff.act_common_params.as_deref()),
        Some("1003#0")
    );

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let root = steps
        .iter()
        .find(|step| step.act_id == Some(31070111))
        .expect("the active skill owns its packet frame");
    assert_eq!(root.to_id, Some(-1));
    assert_eq!(
        root.act_effect
            .iter()
            .take(3)
            .map(|effect| effect.effect_type)
            .collect::<Vec<_>>(),
        vec![
            Some(sonettobuf::effect_type_enum::EffectType::Buffadd as i32),
            Some(sonettobuf::effect_type_enum::EffectType::None as i32),
            Some(sonettobuf::effect_type_enum::EffectType::Fightstep as i32),
        ]
    );
    let passive = root.act_effect[2]
        .fight_step
        .as_ref()
        .expect("the psychube reaction stays nested under the active skill");
    assert_eq!(passive.act_id, Some(435011));
    assert_eq!(passive.from_id, Some(10));
    assert_eq!(passive.to_id, Some(-1));
}

#[test]
fn reactive_skill_frame_targets_the_other_team_of_a_hit() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-2),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-3),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let event = BattleEvent::Hit(crate::engine::event::payload::HitEvent {
        origin: CommandOrigin {
            domain: RuleDomain::Skill,
            key: DefinitionKey::new(100, "SkillDamage"),
        },
        source_uid: 10,
        target_uid: -2,
        skill_id: 100,
        amount: 50,
        shield_absorbed: 0,
        career_restraint: false,
        damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
        assassinate: false,
        ignore_riposte: false,
    });

    assert_eq!(reaction_counterparty(&pool, &event, -2), Some(10));
    assert_eq!(reaction_counterparty(&pool, &event, -3), Some(10));
    assert_eq!(reaction_counterparty(&pool, &event, 10), Some(-2));
    assert_eq!(
        reaction_skill_target(
            &pool,
            &event,
            -3,
            crate::engine::skill::condition::registry::ReactionFrameTarget::Counterparty,
            None,
        ),
        Some(10)
    );
    assert_eq!(
        reaction_skill_target(
            &pool,
            &event,
            -3,
            crate::engine::skill::condition::registry::ReactionFrameTarget::CausingFrame,
            Some(-2),
        ),
        Some(-2)
    );
    assert_eq!(
        reaction_skill_target(
            &pool,
            &event,
            -3,
            crate::engine::skill::condition::registry::ReactionFrameTarget::CausingFrame,
            None,
        ),
        Some(-2)
    );
}

#[test]
fn attack_consumption_keeps_first_hit_entity_order() {
    let hit = |source_uid, target_uid| {
        BattleEvent::Hit(crate::engine::event::payload::HitEvent {
            origin: CommandOrigin {
                domain: RuleDomain::Skill,
                key: DefinitionKey::new(100, "SkillDamage"),
            },
            source_uid,
            target_uid,
            skill_id: 100,
            amount: 50,
            shield_absorbed: 0,
            career_restraint: false,
            damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
            assassinate: false,
            ignore_riposte: false,
        })
    };
    let events = [hit(10, -2), hit(10, -1), hit(11, -2)];

    assert_eq!(
        ordered_hit_entities(&events, |hit| hit.source_uid),
        [10, 11]
    );
    assert_eq!(
        ordered_hit_entities(&events, |hit| hit.target_uid),
        [-2, -1]
    );
}

#[test]
fn damage_based_rebound_routes_once_and_removes_the_counter() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(1_069),
                    buff_id: Some(117200101),
                    from_uid: Some(-1),
                    count: Some(1),
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
    let hit = BattleEvent::Hit(crate::engine::event::payload::HitEvent {
        origin: CommandOrigin {
            domain: RuleDomain::Skill,
            key: DefinitionKey::new(1, "Damage"),
        },
        source_uid: 10,
        target_uid: -1,
        skill_id: 1,
        amount: 1_000,
        shield_absorbed: 0,
        career_restraint: false,
        damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
        assassinate: false,
        ignore_riposte: false,
    });
    let events = [hit.clone(), hit];
    let mut frames = Vec::new();
    let root = push_root(&mut frames, FrameOwner::Command, FrameTrigger::Active);
    let dispatched = dispatch_event_batch(
        &pool.runtime_view(&managers),
        &managers,
        &catalog,
        &mut RoundDeterminism::default(),
        &events,
        &root,
        &root,
        Some(&root),
        Some((10, 1, Some(-1))),
        false,
        true,
        crate::engine::event::subscription::PublicationPhase::AfterPublish,
        None,
    )
    .unwrap();
    let mut queue = std::collections::VecDeque::from(dispatched.into_ordered());

    assert_eq!(queue.len(), 3);
    let result = drain_queue_with_frames(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &mut queue,
        frames,
    )
    .unwrap();

    assert_eq!(managers.hp.current(10), 700);
    assert!(managers.buff.snapshot(-1, 1_069).is_none());
    assert!(result.events.iter().any(|event| matches!(
        event,
        BattleEvent::BuffRemoved(change)
            if change.target_uid == -1 && change.buff_uid == 1_069
    )));
    fn contains_rebound_marker(step: &sonettobuf::FightStep) -> bool {
        step.act_effect.iter().any(|effect| {
            (effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Rebound as i32)
                && effect.buff_act_id == Some(743))
                || effect
                    .fight_step
                    .as_ref()
                    .is_some_and(contains_rebound_marker)
        })
    }
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    assert!(steps.iter().any(contains_rebound_marker));
}

#[test]
fn allied_action_observer_keeps_the_triggering_action_target() {
    let event = BattleEvent::AllyAction(ActionEvent {
        source_uid: 10,
        target_uid: -2,
        skill_id: 100,
        skill_slot: 1,
        is_attack: true,
        rank: 1,

        skill_type: 0,
        effect_tag: 1,
        additional_moxie: 0,
        extra_skill_kind: 0,
        assassinate: false,
        ..Default::default()
    });

    assert_eq!(
        reaction_counterparty(&TargetPool::default(), &event, 99),
        Some(-2)
    );
}

#[test]
fn active_ally_reaction_without_parent_keeps_the_action_target() {
    crate::test_support::init_config();
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
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    let result = run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::AllyAction(ActionEvent {
            source_uid: 10,
            skill_id: 31490111,
            target_uid: 30,
            target_uids: vec![30],
            skill_slot: 1,
            is_attack: true,
            rank: 1,
            mode: SkillExecutionMode::Active,
            ..Default::default()
        }),
    )
    .unwrap();

    assert_eq!(managers.buff.buff_id_amount(30, 31430151), 1);

    fn find_step(step: &sonettobuf::FightStep, act_id: i32) -> Option<&sonettobuf::FightStep> {
        (step.act_id == Some(act_id)).then_some(step).or_else(|| {
            step.act_effect
                .iter()
                .filter_map(|effect| effect.fight_step.as_ref())
                .find_map(|nested| find_step(nested, act_id))
        })
    }

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let reaction = steps
        .iter()
        .find_map(|step| find_step(step, 31430151))
        .unwrap();
    assert_eq!(reaction.to_id, Some(30));
    assert!(reaction.act_effect.iter().any(|effect| {
        effect.target_id == Some(30)
            && effect
                .buff
                .as_ref()
                .is_some_and(|buff| buff.buff_id == Some(31430151))
    }));
}

#[test]
fn rejected_seal_triggers_the_configured_moxie_fallback() {
    crate::test_support::init_config();
    let fight = |immune: bool| Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    attack: Some(1),
                    hp: Some(10_000),
                    ..Default::default()
                }),
                ex_skill: Some(30200135),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(100_000),
                ex_point: Some(5),
                ex_point_max: Some(5),
                attr: Some(HeroAttribute {
                    defense: Some(1_000),
                    hp: Some(100_000),
                    ..Default::default()
                }),
                buffs: immune
                    .then_some(BuffInfo {
                        buff_id: Some(5140006),
                        uid: Some(20),
                        from_uid: Some(-1),
                        ..Default::default()
                    })
                    .into_iter()
                    .collect(),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let execute = |fight: &Fight| {
        let pool = TargetPool::from_fight(fight);
        let mut managers = BattleManagers::seeded(fight);
        let mut invocation: SkillInvocation = SkillRequest {
            source_uid: 10,
            skill_id: 30200135,
        }
        .into();
        invocation.target = SkillTarget::Explicit(-1);
        invocation.mode = SkillExecutionMode::Active;
        let result = run(
            &mut managers,
            &pool,
            crate::engine::skill::effect::catalog::global(),
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            [RuleOp::Skill(invocation)],
        )
        .unwrap();
        (managers, result)
    };

    let (immune_managers, immune_result) = execute(&fight(true));
    assert!(!immune_managers.buff.has_buff_id_or_type(-1, 4007));
    assert_eq!(immune_managers.ex_point.get(-1), 3);
    assert!(immune_result.events.iter().any(|event| matches!(
        event,
        BattleEvent::BuffRejected(rejected)
            if rejected.target_uid == -1
                && rejected.buff_id == 720202
                && rejected.type_id == 4007
    )));
    let immune_steps = crate::engine::packet::timeline::project(&immune_result.frames).unwrap();
    assert!(
        immune_steps
            .iter()
            .any(|step| step.act_effect.iter().any(|effect| {
                effect.effect_type
                    == Some(sonettobuf::effect_type_enum::EffectType::Buffreject as i32)
                    && effect.target_id == Some(-1)
            }))
    );

    let (normal_managers, _) = execute(&fight(false));
    assert!(normal_managers.buff.has_buff_id_or_type(-1, 4007));
    assert_eq!(normal_managers.ex_point.get(-1), 5);
}

#[test]
fn eureka_threshold_reaction_observes_the_gain_from_the_same_action() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(10_000),
                passive_skill: vec![30660143, 30660193],
                power_infos: vec![PowerInfo {
                    power_id: Some(EUREKA_RESOURCE_ID),
                    num: Some(4),
                    max: Some(5),
                }],
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
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
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
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    let result = run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::AllyAction(ActionEvent {
            source_uid: 10,
            skill_id: 30660111,
            target_uid: -1,
            target_uids: vec![-1],
            skill_slot: 1,
            is_attack: true,
            rank: 1,
            ..Default::default()
        }),
    )
    .unwrap();

    let deltas = result
        .events
        .iter()
        .filter_map(|event| match event {
            BattleEvent::EurekaChanged(change) if change.target_uid == 10 => {
                Some(change.applied_delta)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(deltas, [1, -5]);
    assert_eq!(managers.eureka.get(10, EUREKA_RESOURCE_ID).current, 0);
    assert!(managers.hp.current(-1) < 10_000);
}

#[test]
fn after_hit_passive_uses_the_active_skills_successful_buff_additions() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                passive_skill: vec![200],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: [-1, -2, -3]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    current_hp: Some(100),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut passive_slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(
            BehaviorSpec::new(60059, "AddBurnBySkillAddBurnCount"),
            vec![4150001],
            Vec::new(),
        ),
        TargetRequest::self_only(),
    );
    passive_slot.conditions = vec![ParsedCondition {
        opcode: 210,
        type_name: "None".to_owned(),
        kind: ParsedConditionKind::None(NoneMode::SkillActionAfterHit),
        raw_args: Vec::new(),
    }];
    passive_slot.compiled_route = ConditionRoute::compile(&passive_slot.conditions);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::from_spec(BehaviorSpec::new(1, "AddBuff"), vec![4150001], Vec::new()),
            TargetRequest {
                code: 202,
                raw: Vec::new(),
            },
        )],
    });
    catalog.insert(ParsedSkillEffect {
        skill_id: 200,
        slots: vec![passive_slot],
    });

    run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(
            SkillRequest {
                source_uid: 10,
                skill_id: 100,
            }
            .into(),
        )],
    )
    .unwrap();

    assert_eq!(managers.buff.max_id_or_type_layer(10, 4150001), 3);
    assert!(managers.buff.has_buff_id(-1, 4150001));
    assert!(managers.buff.has_buff_id(-2, 4150001));
    assert!(managers.buff.has_buff_id(-3, 4150001));
}

#[test]
fn eureka_reaction_frame_stays_owned_by_the_subscriber() {
    let event = BattleEvent::EurekaChanged(crate::engine::event::payload::EurekaChangeEvent {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(1, "SpendEureka"),
        },
        source_uid: 10,
        target_uid: 20,
        power_id: EUREKA_RESOURCE_ID,
        before: 3,
        requested_delta: -2,
        applied_delta: -2,
        after: 1,
        overflow: 0,
    });

    assert_eq!(
        reaction_skill_target(
            &TargetPool::default(),
            &event,
            99,
            crate::engine::skill::condition::registry::ReactionFrameTarget::Owner,
            None,
        ),
        Some(99)
    );
}

#[test]
fn event_emitted_skill_starts_a_fresh_cast_with_its_explicit_target() {
    crate::test_support::init_config();
    let entity = |uid, model_id, position, career, current_hp, max_hp| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        position: Some(position),
        career: Some(career),
        current_hp: Some(current_hp),
        attr: Some(HeroAttribute {
            hp: Some(max_hp),
            ..Default::default()
        }),
        ..Default::default()
    };
    let facade = |uid| BuffInfo {
        uid: Some(uid),
        buff_id: Some(530000111),
        layer: Some(2),
        count: Some(1),
        ..Default::default()
    };
    let mut ally = entity(20, 3114, 2, 1, 100, 100);
    ally.skill_group1 = vec![31140111, 31140112, 31140113];
    ally.skill_group2 = vec![31140121, 31140122, 31140123];
    let mut pickles = entity(30, 3063, 3, 1, 100, 100);
    pickles.passive_skill = vec![30630151];
    pickles.skill_group1 = vec![30630111, 30630112, 30630113];
    pickles.skill_group2 = vec![30630121, 30630122, 30630123];
    let mut first = entity(-1, 30110801, 1, 4, 50, 100);
    first.buffs = vec![facade(101)];
    let mut second = entity(-2, 30110802, 2, 4, 100, 100);
    second.buffs = vec![facade(102)];
    let mut third = entity(-3, 30110803, 3, 4, 100, 100);
    third.buffs = vec![facade(103)];
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![ally, pickles],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![first, second, third],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);

    run_event(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::AllyAction(ActionEvent {
            source_uid: 20,
            skill_id: 31140131,
            target_uid: -1,
            skill_slot: 3,
            is_attack: false,
            rank: 1,
            skill_type: 0,
            effect_tag: 4,
            additional_moxie: 0,
            extra_skill_kind: 0,
            assassinate: false,
            ..Default::default()
        }),
    )
    .unwrap();

    assert!(managers.buff.has_buff_id(-1, 530000111));
    assert!(!managers.buff.has_buff_id(-2, 530000111));
    assert!(!managers.buff.has_buff_id(-3, 530000111));
}

#[test]
fn target_attacked_passive_and_be_attacked_buff_act_share_one_hit_payload() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                passive_skill: vec![530000151],
                buffs: vec![
                    BuffInfo {
                        uid: Some(20),
                        buff_id: Some(530000111),
                        layer: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(21),
                        buff_id: Some(30620111),
                        layer: Some(1),
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
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let event = BattleEvent::Hit(crate::engine::event::payload::HitEvent {
        origin: CommandOrigin {
            domain: RuleDomain::Skill,
            key: DefinitionKey::new(100, "SkillDamage"),
        },
        source_uid: 10,
        target_uid: -1,
        skill_id: 100,
        amount: 50,
        shield_absorbed: 0,
        career_restraint: false,
        damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
        assassinate: false,
        ignore_riposte: false,
    });

    let result = run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        event,
    )
    .unwrap();

    assert!(!managers.buff.has_buff_id(-1, 530000111));
    assert_eq!(managers.buff.snapshot(-1, 21).unwrap().layer, Some(0));
    assert_eq!(managers.ex_point.get(10), 1);
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    assert!(steps.iter().any(|step| step.act_id == Some(530000151)));
    assert!(steps.iter().any(|step| step.act_id == Some(30620111)));
}

#[test]
fn entity_defeat_passive_executes_each_configured_sibling_slot() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(10_000),
                passive_skill: vec![30865186],
                attr: Some(HeroAttribute {
                    attack: Some(1_000),
                    hp: Some(10_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(30860113),
                    layer: Some(2),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(0),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        defense: Some(500),
                        mdefense: Some(500),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        defense: Some(500),
                        mdefense: Some(500),
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
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let event = BattleEvent::EntityDied(crate::engine::event::payload::EntityDiedEvent {
        source_uid: 10,
        target_uid: -1,
    });

    let dispatched = dispatcher::dispatch_event(
        &pool.runtime_view(&managers),
        &managers,
        &catalog,
        &mut RoundDeterminism::default(),
        &event,
    )
    .unwrap();
    let mut slots = dispatched
        .skills
        .iter()
        .filter(|(subscriber, _)| subscriber.skill_id == 30865186)
        .filter_map(|(subscriber, _)| subscriber.slot_index)
        .collect::<Vec<_>>();
    slots.sort_unstable();
    assert_eq!(slots, vec![4, 5]);

    run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        event,
    )
    .unwrap();

    assert_eq!(managers.buff.snapshot(10, 20).unwrap().layer, Some(4));
}

#[test]
fn lucy_entity_defeat_follow_up_respects_its_configured_round_limit() {
    crate::test_support::init_config();

    let fires = |passive_skill, round_limit| {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(10_000),
                    passive_skill: vec![passive_skill],
                    attr: Some(HeroAttribute {
                        attack: Some(100),
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
                        team_type: Some(2),
                        current_hp: Some(0),
                        attr: Some(HeroAttribute {
                            hp: Some(10_000),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(-2),
                        team_type: Some(2),
                        current_hp: Some(1_000_000),
                        attr: Some(HeroAttribute {
                            hp: Some(1_000_000),
                            defense: Some(100),
                            mdefense: Some(100),
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
        let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
        let mut managers = BattleManagers::seeded(&fight);
        let event = BattleEvent::EntityDied(crate::engine::event::payload::EntityDiedEvent {
            source_uid: 10,
            target_uid: -1,
        });
        let dispatched = dispatcher::dispatch_event(
            &pool.runtime_view(&managers),
            &managers,
            &catalog,
            &mut RoundDeterminism::default(),
            &event,
        )
        .unwrap();
        let effect = catalog.get(passive_skill).unwrap();
        let subscriber = dispatched
            .skills
            .iter()
            .map(|(subscriber, _)| subscriber)
            .find(|subscriber| {
                subscriber.skill_id == passive_skill
                    && subscriber
                        .slot_index
                        .is_some_and(|slot| effect.slots[slot].round_limit == round_limit)
            })
            .unwrap();
        let slot_index = subscriber.slot_index.unwrap();
        let slot = &effect.slots[slot_index];
        assert_eq!(slot.round_limit, round_limit);
        let can_fire = |managers: &BattleManagers| {
            managers.can_fire_rule(
                10,
                passive_skill,
                slot_index,
                subscriber.key.definition,
                slot.limit,
                slot.round_limit,
            )
        };

        for _ in 0..round_limit {
            assert!(can_fire(&managers));
            run_event(
                &mut managers,
                &pool,
                &catalog,
                &mut RoundDeterminism::default(),
                TargetContext::default(),
                event.clone(),
            )
            .unwrap();
        }

        assert!(!can_fire(&managers));
        for _ in 0..2 {
            run_event(
                &mut managers,
                &pool,
                &catalog,
                &mut RoundDeterminism::default(),
                TargetContext::default(),
                event.clone(),
            )
            .unwrap();
        }
        assert!(!can_fire(&managers));
        managers.begin_round();
        assert!(can_fire(&managers));
        run_event(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            event,
        )
        .unwrap();
        assert_eq!(can_fire(&managers), round_limit > 1);
    };

    fires(30865171, 2);
    fires(30865175, 4);
    fires(30865186, 4);
}

#[test]
fn restrained_ultimate_consumes_bedrock_before_damage_once_per_round() {
    crate::test_support::init_config();
    let fight = || Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                career: Some(3),
                current_hp: Some(10_000),
                ex_point: Some(5),
                ex_skill: Some(30060133),
                skill_group1: vec![30060111],
                attr: Some(HeroAttribute {
                    attack: Some(1),
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
                career: Some(8),
                weak_careers: vec![3],
                current_hp: Some(1_000_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000_000),
                    ..Default::default()
                }),
                passive_skill: vec![109360013],
                buffs: vec![BuffInfo {
                    uid: Some(100),
                    buff_id: Some(109360005),
                    from_uid: Some(-1),
                    layer: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let action = |skill_id| {
        let mut invocation: SkillInvocation = SkillRequest {
            source_uid: 10,
            skill_id,
        }
        .into();
        invocation.target = SkillTarget::Explicit(-1);
        invocation.mode = SkillExecutionMode::Active;
        invocation
    };
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let context = TargetContext {
        current_round: 1,
        ..Default::default()
    };

    let normal_fight = fight();
    let normal_pool = TargetPool::from_fight(&normal_fight);
    let mut normal_managers = BattleManagers::seeded(&normal_fight);
    run_action(
        &mut normal_managers,
        &normal_pool,
        &catalog,
        &mut RoundDeterminism::default(),
        context,
        [],
        action(30060111),
    )
    .unwrap();
    assert_eq!(normal_managers.buff.buff_id_amount(-1, 109360005), 7);

    let ultimate_fight = fight();
    let ultimate_pool = TargetPool::from_fight(&ultimate_fight);
    let mut ultimate_managers = BattleManagers::seeded(&ultimate_fight);
    let ultimate_result = run_action_with_cost(
        &mut ultimate_managers,
        &ultimate_pool,
        &catalog,
        &mut RoundDeterminism::default(),
        context,
        [],
        Some(ExPointCommand::Spend(ExPointChange {
            origin: CARD_PLAY_ORIGIN,
            source_uid: 10,
            target_uid: 10,
            delta: -5,
            config_effect: 0,
            effect_type: sonettobuf::effect_type_enum::EffectType::Expointchange as i32,
        })),
        action(30060133),
    )
    .unwrap();
    assert_eq!(ultimate_managers.buff.buff_id_amount(-1, 109360005), 0);

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
    for frame in &ultimate_result.frames {
        ordered_changes(frame, &mut changes);
    }
    let bedrock_removal = changes
        .iter()
        .position(|change| {
            matches!(
                change,
                BattleChange::Buff(change)
                    if change.change.removed.iter().any(|removed| {
                        removed.buff.buff_id == Some(109360005)
                    })
            )
        })
        .unwrap();
    let incoming_damage = changes
        .iter()
        .position(|change| {
            matches!(
                change,
                BattleChange::Hp(change)
                    if change.target_uid == -1 && change.damage.is_some()
            )
        })
        .unwrap();
    assert!(bedrock_removal < incoming_damage);

    run(
        &mut ultimate_managers,
        &ultimate_pool,
        &catalog,
        &mut RoundDeterminism::default(),
        context,
        [RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
            BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: -1,
                target_uid: -1,
                buff_id: 109360005,
                amount: Some(10),
                occurrences: 1,
                child_uid_reservations: 0,
            },
        )))],
    )
    .unwrap();
    run_action(
        &mut ultimate_managers,
        &ultimate_pool,
        &catalog,
        &mut RoundDeterminism::default(),
        context,
        [],
        action(30060133),
    )
    .unwrap();
    assert_eq!(ultimate_managers.buff.buff_id_amount(-1, 109360005), 10);
}

#[test]
fn gorgon_death_kills_tentacles_and_exposes_the_core() {
    crate::test_support::init_config();
    let entity = |uid, model_id, position, hp, passive_skill| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        position: Some(position),
        team_type: Some(2),
        current_hp: Some(hp),
        passive_skill,
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            ..Default::default()
        }),
        ..Default::default()
    };
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
            entitys: vec![
                entity(-1, 150401, 1, 10_000, vec![114200141]),
                entity(-2, 150402, 2, 10_000, vec![]),
                entity(-3, 150403, 3, 10_000, vec![]),
            ],
            sp_entitys: vec![entity(-4, 150404, 4, 10_000, vec![114200143])],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    managers.hp.lose(-1, 10_000, 10);

    run_event(
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

    assert_eq!(managers.hp.current(-2), 0);
    assert_eq!(managers.hp.current(-3), 0);
    assert_eq!(managers.hp.current(-1), 5_000);
    assert!(managers.buff.has_buff_id(-4, 11410082));

    run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::EntityDied(crate::engine::event::payload::EntityDiedEvent {
            source_uid: 10,
            target_uid: -2,
        }),
    )
    .unwrap();

    assert_eq!(managers.hp.current(-1), 5_000);
}

#[test]
fn active_skill_publishes_hits_between_after_damage_and_after_hit_rows() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    attack: Some(1_000),
                    hp: Some(10_000),
                    ..Default::default()
                }),
                buffs: vec![
                    BuffInfo {
                        uid: Some(30),
                        buff_id: Some(31280113),
                        layer: Some(50),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(33),
                        buff_id: Some(4150002),
                        count: Some(1),
                        layer: Some(1),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    hp: Some(10_000),
                    ..Default::default()
                }),
                passive_skill: vec![530000151],
                buffs: vec![
                    BuffInfo {
                        uid: Some(31),
                        buff_id: Some(31280111),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(32),
                        buff_id: Some(530000111),
                        layer: Some(2),
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
    let mut managers = BattleManagers::seeded(&fight);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 31280111,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);

    let result = run(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let effects = &steps
        .iter()
        .find(|step| step.act_id == Some(31280111))
        .unwrap()
        .act_effect;
    let fear_act_info = effects
        .iter()
        .position(|effect| {
            effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Buffactinfoupdate as i32)
                && effect.target_id == Some(-1)
        })
        .unwrap();
    let fear_delete = effects
        .iter()
        .position(|effect| {
            effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Buffdel as i32)
                && effect.buff.as_ref().and_then(|buff| buff.buff_id) == Some(31280111)
        })
        .unwrap();
    let fear = effects
        .iter()
        .rposition(|effect| effect.buff.as_ref().and_then(|buff| buff.buff_id) == Some(31280111))
        .unwrap();
    let attacked = effects
        .iter()
        .position(|effect| {
            effect.fight_step.as_ref().and_then(|step| step.act_id) == Some(530000151)
        })
        .unwrap();
    let combustion_cleanup = effects
        .iter()
        .position(|effect| effect.fight_step.as_ref().and_then(|step| step.act_id) == Some(4150002))
        .unwrap();
    let shock_wave = effects
        .iter()
        .position(|effect| effect.buff.as_ref().and_then(|buff| buff.buff_id) == Some(31280113))
        .unwrap();

    assert!(fear_act_info < fear_delete);
    assert!(fear < attacked && attacked < combustion_cleanup && combustion_cleanup < shock_wave);
}

#[test]
fn rhiannon_ultimate_snapshots_ally_attributes_before_each_buff_grant() {
    crate::test_support::init_config();
    let ally = |uid| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(1),
        current_hp: Some(2_000),
        attr: Some(HeroAttribute {
            hp: Some(2_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3146),
                    team_type: Some(1),
                    current_hp: Some(12_763),
                    attr: Some(HeroAttribute {
                        hp: Some(12_763),
                        attack: Some(2_140),
                        ..Default::default()
                    }),
                    ex_skill: Some(31460131),
                    ..Default::default()
                },
                ally(11),
                ally(12),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
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
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 31460131,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);

    let result = run(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let effects = &steps
        .iter()
        .find(|step| step.act_id == Some(31460131))
        .unwrap()
        .act_effect;
    for target_uid in [11, 12] {
        let add = effects
            .iter()
            .position(|effect| {
                effect.target_id == Some(target_uid)
                    && effect.buff.as_ref().and_then(|buff| buff.buff_id) == Some(31460131)
            })
            .unwrap();
        let buff_uid = effects[add].buff.as_ref().unwrap().uid.unwrap();
        let markers = effects[..add]
            .iter()
            .filter(|effect| {
                effect.effect_type
                    == Some(sonettobuf::effect_type_enum::EffectType::Buffactinfoupdate as i32)
                    && effect.reserve_id == Some(buff_uid)
            })
            .collect::<Vec<_>>();
        assert_eq!(markers.len(), 2);
        assert!(markers.iter().all(|effect| {
            effect.target_id == Some(0)
                && effect.reserve_id == Some(buff_uid)
                && effect.buff_act_info.as_ref().and_then(|info| info.act_id) == Some(1131)
        }));
        assert_eq!(
            markers
                .iter()
                .filter_map(|effect| effect.buff_act_info.as_ref()?.str_param.as_deref())
                .collect::<Vec<_>>(),
            ["102#171", "101#1021"]
        );
        assert_eq!(
            effects[add]
                .buff
                .as_ref()
                .unwrap()
                .act_info
                .iter()
                .filter_map(|info| info.str_param.as_deref())
                .collect::<Vec<_>>(),
            ["102#171", "101#1021"]
        );
        assert_eq!(managers.origin_attribute(target_uid, AttrId::Attack), 1_171);
        assert_eq!(managers.hp.max(target_uid), 3_021);
    }
}

#[test]
fn actual_contract_bound_death_clears_buffs_and_cards_through_the_drain() {
    crate::test_support::init_config();

    for passive_skill in [31000141, 31000142] {
        let card = |uid, skill_id| CardInfo {
            uid: Some(uid),
            skill_id: Some(skill_id),
            ..Default::default()
        };
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
                        attr: Some(HeroAttribute {
                            hp: Some(100),
                            attack: Some(100),
                            ..Default::default()
                        }),
                        passive_skill: vec![passive_skill],
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(20),
                        model_id: Some(3086),
                        position: Some(2),
                        team_type: Some(1),
                        career: Some(1),
                        current_hp: Some(100),
                        attr: Some(HeroAttribute {
                            hp: Some(100),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(30),
                        model_id: Some(3001),
                        position: Some(3),
                        team_type: Some(1),
                        current_hp: Some(100),
                        attr: Some(HeroAttribute {
                            hp: Some(100),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    position: Some(1),
                    team_type: Some(2),
                    current_hp: Some(100),
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
        let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
        let effect = catalog
            .get(passive_skill)
            .unwrap_or_else(|| panic!("missing actual passive effect {passive_skill}"));
        let dead_key = DefinitionKey::new(8, "Dead");
        let none_key = DefinitionKey::new(52, "None");
        let lanes = catalog
            .compiled_subscription_lanes(passive_skill)
            .unwrap_or_else(|error| panic!("passive {passive_skill} route failed: {error:?}"));
        assert!(
            lanes.iter().any(|(slot_index, subscription)| {
                *slot_index == 3
                    && subscription.event == crate::engine::event::kind::EventKind::EntityDied
                    && subscription.definition == dead_key
            }),
            "passive {passive_skill} did not discover slot 3 Dead subscription: {lanes:?}"
        );
        assert!(
            !lanes
                .iter()
                .any(|(_, subscription)| { subscription.definition == none_key })
        );

        let end_slot = effect
            .slots
            .get(3)
            .unwrap_or_else(|| panic!("passive {passive_skill} has no slot 3"));
        assert_eq!(end_slot.behavior.spec.key.opcode, 60093);
        assert_eq!(end_slot.behavior.spec.key.type_name, "ContractEndClearBuff");
        let owner_buff_ids = end_slot
            .behavior
            .arg_list(0)
            .expect("actual contract cleanup has owner buff arguments");
        let bound_buff_ids = end_slot
            .behavior
            .arg_list(1)
            .expect("actual contract cleanup has bound buff arguments");

        let contract_origin = CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(60092, "NotifyHeroContract"),
        };
        managers
            .contract
            .execute(crate::engine::manager::contract::ContractCommand::Offer {
                origin: contract_origin,
                owner_uid: 10,
                candidates: vec![20],
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
        assert_eq!(managers.contract.bound_uid(10), Some(20));

        let seed_buff = |managers: &mut BattleManagers, target_uid, buff_id| {
            managers
                .execute_buff(BuffCommand::Grant(BuffGrant {
                    origin: CommandOrigin {
                        domain: RuleDomain::Behavior,
                        key: DefinitionKey::new(60093, "ContractEndClearBuff"),
                    },
                    source_uid: target_uid,
                    target_uid,
                    buff_id,
                    amount: Some(1),
                    occurrences: 1,
                    child_uid_reservations: 0,
                }))
                .unwrap();
        };
        for buff_id in owner_buff_ids.iter().copied() {
            seed_buff(&mut managers, 10, buff_id);
        }
        for buff_id in bound_buff_ids.iter().copied() {
            seed_buff(&mut managers, 20, buff_id);
        }
        seed_buff(&mut managers, 10, 31000141);
        seed_buff(&mut managers, 20, 31000142);

        managers
            .execute_card(CardCommand::Setup(CardSetup {
                hand: vec![card(20, 200), card(10, 300), card(30, 400)],
                draw_pile: vec![card(20, 201), card(10, 301), card(30, 401)],
                deck_num: 6,
            }))
            .unwrap();

        for buff_id in owner_buff_ids.iter().copied() {
            assert!(managers.buff.has_buff_id(10, buff_id));
        }
        for buff_id in bound_buff_ids.iter().copied() {
            assert!(managers.buff.has_buff_id(20, buff_id));
        }
        assert!(managers.buff.has_buff_id(10, 31000141));
        assert!(managers.buff.has_buff_id(20, 31000142));
        assert!(managers.card.hand().iter().any(|card| card.uid == Some(20)));
        assert!(
            managers
                .card
                .draw_pile()
                .iter()
                .any(|card| card.uid == Some(20))
        );

        let death = BattleEvent::EntityDied(crate::engine::event::payload::EntityDiedEvent {
            source_uid: 10,
            target_uid: 20,
        });
        let dispatched = crate::engine::event::dispatcher::dispatch_event(
            &pool.runtime_view(&managers),
            &managers,
            &catalog,
            &mut RoundDeterminism::default(),
            &death,
        )
        .unwrap();
        assert!(dispatched.skills.iter().any(|(subscriber, _)| {
            subscriber.owner_uid == 10
                && subscriber.skill_id == passive_skill
                && subscriber.slot_index == Some(3)
                && subscriber.key.definition == dead_key
        }));
        assert!(
            !dispatched
                .skills
                .iter()
                .any(|(subscriber, _)| subscriber.key.definition == none_key)
        );

        let result = run_command_group(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext {
                current_round: 1,
                ..Default::default()
            },
            [RuleOp::Command(BattleCommand::Hp(
                crate::engine::manager::hp::HpCommand::Damage(
                    crate::engine::manager::hp::HpDamage {
                        origin: CommandOrigin {
                            domain: RuleDomain::Behavior,
                            key: DefinitionKey::new(1, "TestDamage"),
                        },
                        source_uid: 10,
                        target_uid: 20,
                        amount: 100,
                        config_effect: 1,
                        effect_kind: crate::engine::manager::hp::DamageEffectKind::Normal,
                        assassinate: false,
                        ignore_riposte: false,
                        hurt: crate::engine::manager::hp::HurtInfoData {
                            from_uid: 10,
                            is_crit: false,
                            career_restraint: false,
                            reduce_hp: 0,
                            effect_id: 1,
                            skill_id: 1,
                            damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
                            buff_act_id: 0,
                            buff_uid: 0,
                            hurt_effect_type: 0,
                            display_amount: None,
                        },
                    },
                ),
            ))],
        )
        .unwrap();

        assert_eq!(managers.hp.current(20), 0);
        assert!(result.events.iter().any(|event| {
            matches!(
                event,
                BattleEvent::EntityDied(death)
                    if death.source_uid == 10 && death.target_uid == 20
            )
        }));
        assert!(result.outcomes.iter().any(|outcome| {
            matches!(
                outcome,
                RuleOutcome::Contract(crate::engine::manager::contract::ContractChange::Cleared {
                    owner_uid: 10,
                    bound_uid: 20,
                })
            )
        }));
        assert_eq!(managers.contract.bound_uid(10), None);

        for buff_id in owner_buff_ids.iter().copied() {
            assert!(!managers.buff.has_buff_id(10, buff_id));
        }
        for buff_id in bound_buff_ids.iter().copied() {
            assert!(!managers.buff.has_buff_id(20, buff_id));
        }
        assert!(!managers.buff.has_buff_id(20, 31000151));
        assert!(!managers.buff.has_buff_id(10, 31000141));
        assert!(managers.buff.has_buff_id(20, 31000142));

        let remaining_cards = managers
            .card
            .hand()
            .iter()
            .chain(managers.card.draw_pile())
            .collect::<Vec<_>>();
        assert!(remaining_cards.iter().all(|card| card.uid != Some(20)));
        assert!(
            remaining_cards
                .iter()
                .any(|card| card.uid == Some(10) && card.skill_id == Some(300))
        );
        assert!(
            remaining_cards
                .iter()
                .any(|card| card.uid == Some(30) && card.skill_id == Some(400))
        );
    }
}
