use super::*;

#[test]
fn conduit_skill_finishes_after_its_hit_passive_reaction() {
    init_config();
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
            entitys: vec![FightEntityInfo {
                ex_skill_level: Some(2),
                passive_skill: vec![422811],
                ..entity(10, 3144)
            }],
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
    managers
        .conduit
        .execute(
            crate::engine::manager::conduit::ConduitCommand::SelectGroup {
                source_uid: 10,
                group: 2,
            },
        )
        .unwrap();

    let result = crate::engine::runtime::drain::run_conduit_action(
        BattleAccess::new(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
        ),
        10,
        2,
        crate::engine::runtime::drain::ConduitSlot {
            device_index: 1,
            skill_position: 2,
        },
        31441121,
        None,
        None,
    )
    .unwrap();

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let skill = steps
        .iter()
        .find_map(|step| find_step(step, 31441121))
        .unwrap();
    let reaction = skill
        .act_effect
        .iter()
        .position(|effect| effect.fight_step.as_ref().and_then(|step| step.act_id) == Some(422811))
        .unwrap();
    let completion = skill
        .act_effect
        .iter()
        .position(|effect| {
            effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Counterchange as i32)
                && effect.effect_num == Some(63)
        })
        .unwrap();

    assert!(reaction < completion);
}

#[test]
fn conduit_activation_reactions_settle_before_ally_action_reactions() {
    init_config();
    let entity = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        team_type: Some(1),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    ex_skill_level: Some(2),
                    destiny_stone: Some(302502),
                    destiny_rank: Some(4),
                    ..entity(10, 3025)
                },
                FightEntityInfo {
                    passive_skill: vec![31450141, 437311],
                    equips: vec![sonettobuf::EquipRecord {
                        equip_id: Some(1573),
                        refine_lv: Some(1),
                        ..Default::default()
                    }],
                    buffs: vec![sonettobuf::BuffInfo {
                        uid: Some(111),
                        buff_id: Some(31450142),
                        from_uid: Some(11),
                        layer: Some(3),
                        ..Default::default()
                    }],
                    ..entity(11, 3145)
                },
                FightEntityInfo {
                    passive_skill: vec![31440141],
                    ..entity(12, 3144)
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                team_type: Some(2),
                ..entity(-1, 1001)
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    managers
        .conduit
        .execute(
            crate::engine::manager::conduit::ConduitCommand::ChangePower(
                crate::engine::manager::conduit::ConduitPowerChange {
                    origin: crate::engine::skill::rule::CommandOrigin {
                        domain: crate::engine::skill::rule::RuleDomain::Lifecycle,
                        key: crate::engine::skill::rule::DefinitionKey::new(0, "TestPower"),
                    },
                    source_uid: 10,
                    team: 1,
                    power_id: 2,
                    delta: 1,
                    kind: crate::engine::manager::conduit::ConduitPowerChangeKind::Standard,
                },
            ),
        )
        .unwrap();
    managers
        .conduit
        .execute(crate::engine::manager::conduit::ConduitCommand::StopSkill {
            origin: crate::engine::skill::rule::CommandOrigin {
                domain: crate::engine::skill::rule::RuleDomain::Behavior,
                key: crate::engine::skill::rule::DefinitionKey::new(60294, "StopDeviceSkill"),
            },
            source_uid: 11,
            team: 1,
            skill_id: 31450111,
        })
        .unwrap();

    let result = crate::engine::runtime::drain::run_conduit_action(
        BattleAccess::new(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
        ),
        10,
        1,
        crate::engine::runtime::drain::ConduitSlot {
            device_index: 1,
            skill_position: 1,
        },
        302524112,
        None,
        None,
    )
    .unwrap();

    let activated = result
        .outcomes
        .iter()
        .position(|outcome| {
            matches!(
                outcome,
                RuleOutcome::Conduit(
                    crate::engine::manager::conduit::ConduitChange::ActivationCompleted(_)
                )
            )
        })
        .unwrap();
    let action_completed = result
        .outcomes
        .iter()
        .position(|outcome| {
            matches!(
                outcome,
                RuleOutcome::SkillLifecycle(SkillLifecycle::ActionCompleted(_))
            )
        })
        .unwrap();

    assert!(activated < action_completed);
    assert!(!managers.buff.has_buff_id(11, 31450142));

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let skill = steps
        .iter()
        .find_map(|step| find_step(step, 302524112))
        .unwrap();
    let reactions = skill
        .act_effect
        .iter()
        .filter_map(|effect| effect.fight_step.as_ref())
        .filter(|step| step.act_id == Some(31450141))
        .collect::<Vec<_>>();
    assert_eq!(reactions.len(), 1);
    assert!(reactions[0].act_effect.iter().any(|effect| {
        effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Devicestop as i32)
            && effect.target_id == Some(11)
            && effect.effect_num == Some(0)
    }));
    assert!(skill.act_effect.iter().any(|effect| {
        effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Buffdel as i32)
            && effect.target_id == Some(11)
            && effect.buff.as_ref().and_then(|buff| buff.buff_id) == Some(31450142)
    }));
    let mend_removed = skill
        .act_effect
        .iter()
        .position(|effect| {
            effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Buffdel as i32)
                && effect.target_id == Some(11)
                && effect.buff.as_ref().and_then(|buff| buff.buff_id) == Some(31450142)
        })
        .unwrap();
    for hero_reaction in [31440141, 31450141] {
        let position = skill
            .act_effect
            .iter()
            .position(|effect| {
                effect.fight_step.as_ref().and_then(|step| step.act_id) == Some(hero_reaction)
            })
            .unwrap();
        assert!(position < mend_removed);
    }
    let psychube_reaction = skill
        .act_effect
        .iter()
        .position(|effect| effect.fight_step.as_ref().and_then(|step| step.act_id) == Some(437311))
        .unwrap();

    assert!(mend_removed < psychube_reaction);
}
