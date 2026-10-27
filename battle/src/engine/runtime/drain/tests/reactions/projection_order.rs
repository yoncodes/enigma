use super::*;

#[test]
fn synchronization_action_publishes_card_state_after_the_skill() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3123),
                team_type: Some(1),
                current_hp: Some(10_000),
                ex_point: Some(99),
                ex_point_type: Some(2),
                attr: Some(HeroAttribute {
                    hp: Some(10_000),
                    attack: Some(1_000),
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
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(100000, "EzioProps"),
    };
    managers
        .execute_ex_point(
            crate::engine::manager::ex_point::ExPointCommand::ConfigureSynchronization(
                crate::engine::manager::ex_point::ExPointConfigureSynchronization {
                    origin,
                    target_uid: 10,
                    definition: crate::engine::manager::ex_point::SynchronizationDefinition::new(
                        [312301323, 312301333, 312301343],
                        4,
                        100,
                    )
                    .unwrap(),
                },
            ),
        )
        .unwrap();
    managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: 10,
            target_uid: 10,
            buff_id: 229100,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();
    let catalog = SkillEffectCatalog::from_roots(
        config::configs::get(),
        [312301323, 312301333, 312301343],
        [],
    );
    let hand = vec![CardInfo {
        uid: Some(10),
        hero_id: Some(3123),
        skill_id: Some(31230111),
        ..Default::default()
    }];
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: hand.clone(),
            draw_pile: Vec::new(),
            deck_num: 27,
        }))
        .unwrap();
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 312301333,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    invocation.mode = SkillExecutionMode::Synchronization;

    let result = run_skill(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        invocation,
        crate::engine::skill::action::SkillModifiers::default(),
    )
    .unwrap();

    assert!(matches!(
        result.frames.last().map(|frame| &frame.owner),
        Some(FrameOwner::SynchronizationCardState { source_uid: 10 })
    ));
    let steps = crate::engine::packet::timeline::project_for_version(&result.frames, 7).unwrap();
    let published = steps.last().unwrap();
    assert_eq!(
        published
            .act_effect
            .iter()
            .filter_map(|effect| effect.effect_type)
            .collect::<Vec<_>>(),
        vec![
            sonettobuf::effect_type_enum::EffectType::Cardspush as i32,
            sonettobuf::effect_type_enum::EffectType::Carddecknum as i32,
        ]
    );
    assert_eq!(managers.card.hand(), hand);
    assert_eq!(managers.card.deck_num(), 27);
}

#[test]
fn each_change_attr_one_way_hp_replacement_releases_before_buff_deletion() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3146),
                    team_type: Some(1),
                    current_hp: Some(20_000),
                    attr: Some(HeroAttribute {
                        hp: Some(20_000),
                        attack: Some(4_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    team_type: Some(1),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        attack: Some(1_000),
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
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "AddBuff"),
    };
    let grant = || {
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: 10,
            target_uid: 11,
            buff_id: 31460131,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        })))
    };
    run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [grant()],
    )
    .unwrap();

    let replaced = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [grant()],
    )
    .unwrap();
    let relevant = crate::engine::packet::timeline::project_for_version(&replaced.frames, 7)
        .unwrap()
        .into_iter()
        .flat_map(|step| step.act_effect)
        .filter(|effect| {
            effect.target_id == Some(11)
                && (effect.effect_type
                    == Some(sonettobuf::effect_type_enum::EffectType::Buffdel as i32)
                    || effect.effect_type
                        == Some(sonettobuf::effect_type_enum::EffectType::Maxhpchange as i32)
                        && effect.buff_act_id == Some(1131))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        relevant
            .iter()
            .filter_map(|effect| effect.effect_type)
            .collect::<Vec<_>>(),
        vec![
            sonettobuf::effect_type_enum::EffectType::Maxhpchange as i32,
            sonettobuf::effect_type_enum::EffectType::Maxhpchange as i32,
            sonettobuf::effect_type_enum::EffectType::Buffdel as i32,
        ]
    );
    assert!(
        relevant[..2]
            .iter()
            .all(|effect| { effect.effect_num == Some(1_000) && effect.buff_act_id == Some(1131) })
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
