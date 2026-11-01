use super::*;

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
        card_index: 0,
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
