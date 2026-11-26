use super::*;

#[test]
fn interval_consumes_mend_and_restarts_only_its_device() {
    init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(3145),
        team_type: Some(1),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(100),
            ..Default::default()
        }),
        passive_skill: vec![31450141],
        buffs: vec![BuffInfo {
            uid: Some(uid + 100),
            buff_id: Some(31450142),
            from_uid: Some(uid),
            layer: Some(4),
            ..Default::default()
        }],
        ..Default::default()
    };

    for (skill_id, expected_amount, expected_restarts) in
        [(31450111, 0, vec![10]), (31450121, 4, Vec::new())]
    {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![entity(10), entity(11)],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    model_id: Some(1001),
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
        let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
        let mut invocation = crate::engine::skill::action::SkillInvocation::from(
            crate::engine::skill::action::SkillRequest {
                source_uid: 10,
                skill_id,
            },
        );
        invocation.mode = SkillExecutionMode::Device;
        invocation.target = crate::engine::skill::action::SkillTarget::Explicit(-1);

        let result = crate::engine::runtime::drain::run_action(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            [],
            invocation,
        )
        .unwrap();

        assert_eq!(managers.buff.buff_id_amount(10, 31450142), expected_amount);
        assert_eq!(managers.buff.buff_id_amount(11, 31450142), 4);
        let restarts = result
            .outcomes
            .iter()
            .filter_map(|outcome| match outcome {
                RuleOutcome::Conduit(
                    crate::engine::manager::conduit::ConduitChange::DeviceRestarted {
                        source_uid,
                        ..
                    },
                ) => Some(*source_uid),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(restarts, expected_restarts);
        if skill_id == 31450111 {
            let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
            let skill = steps
                .iter()
                .find_map(|step| find_step(step, skill_id))
                .unwrap();
            let reactions = skill
                .act_effect
                .iter()
                .filter_map(|effect| effect.fight_step.as_ref())
                .filter(|step| step.act_id == Some(31450141))
                .collect::<Vec<_>>();
            assert_eq!(reactions.len(), 1);
            assert!(reactions[0].act_effect.iter().any(|effect| {
                effect.effect_type
                    == Some(sonettobuf::effect_type_enum::EffectType::Devicestop as i32)
                    && effect.target_id == Some(10)
                    && effect.effect_num == Some(0)
            }));
            assert!(skill.act_effect.iter().any(|effect| {
                effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Buffdel as i32)
                    && effect.target_id == Some(10)
                    && effect.buff.as_ref().and_then(|buff| buff.buff_id) == Some(31450142)
            }));
        }
    }
}

#[test]
fn free_skill_repeats_then_continues_in_the_selected_group() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3145),
                team_type: Some(1),
                current_hp: Some(100_000),
                attr: Some(HeroAttribute {
                    hp: Some(100_000),
                    attack: Some(100),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(110),
                    buff_id: Some(31450121),
                    from_uid: Some(10),
                    layer: Some(2),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(1001),
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
    managers
        .conduit
        .execute(
            crate::engine::manager::conduit::ConduitCommand::SelectGroup {
                source_uid: 10,
                group: 2,
            },
        )
        .unwrap();
    let mut catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    let result = run_conduit_phase(
        managers.catalog(),
        crate::engine::runtime::schedule::ConduitPhase {
            fight: &fight,
            operations: &[sonettobuf::FightDeviceOper {
                uid: Some(10),
                index: Some(1),
            }],
        },
        &mut managers,
        &pool,
        &mut catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
    )
    .unwrap();

    let finished = result
        .outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            RuleOutcome::Conduit(
                crate::engine::manager::conduit::ConduitChange::SkillFinished {
                    source_uid: 10,
                    skill_id,
                    ..
                },
            ) => Some(*skill_id),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(finished, vec![31450121, 31450121, 31450111]);
    assert_eq!(managers.conduit.uses(10), 3);
    assert_eq!(managers.ex_point.get(10), 25);
    assert!(!managers.buff.has_buff_id(10, 31450121));
    assert_eq!(managers.conduit.selected_group(10), Some(1));
    assert_eq!(
        result
            .frames
            .iter()
            .filter(|frame| matches!(
                frame.owner,
                crate::engine::runtime::record::FrameOwner::ConduitStopped { source_uid: 10, .. }
            ))
            .count(),
        1
    );
}

#[test]
fn mend_restart_reopens_a_device_after_its_group_changes() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3145),
                team_type: Some(1),
                current_hp: Some(100_000),
                attr: Some(HeroAttribute {
                    hp: Some(100_000),
                    attack: Some(100),
                    ..Default::default()
                }),
                passive_skill: vec![31450141],
                buffs: vec![
                    BuffInfo {
                        uid: Some(110),
                        buff_id: Some(31450121),
                        from_uid: Some(10),
                        layer: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(111),
                        buff_id: Some(31450142),
                        from_uid: Some(10),
                        layer: Some(4),
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
                model_id: Some(1001),
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
    managers
        .conduit
        .execute(
            crate::engine::manager::conduit::ConduitCommand::SelectGroup {
                source_uid: 10,
                group: 2,
            },
        )
        .unwrap();
    let mut catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    let result = run_conduit_phase(
        managers.catalog(),
        crate::engine::runtime::schedule::ConduitPhase {
            fight: &fight,
            operations: &[sonettobuf::FightDeviceOper {
                uid: Some(10),
                index: Some(1),
            }],
        },
        &mut managers,
        &pool,
        &mut catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
    )
    .unwrap();

    let finished = result
        .outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            RuleOutcome::Conduit(
                crate::engine::manager::conduit::ConduitChange::SkillFinished {
                    source_uid: 10,
                    skill_id,
                    ..
                },
            ) => Some(*skill_id),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(finished, vec![31450121, 31450111, 31450111]);
    assert_eq!(managers.ex_point.get(10), 20);
    assert!(!managers.buff.has_buff_id(10, 31450142));
}
