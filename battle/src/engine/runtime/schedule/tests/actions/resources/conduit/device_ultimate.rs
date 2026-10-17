use super::*;

fn fight(ex_point: i32) -> Fight {
    let entity = |uid, model_id, ex_point, ex_point_type| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(100_000),
        ex_point: Some(ex_point),
        ex_point_type: Some(ex_point_type),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut twins = entity(
        10,
        3149,
        ex_point,
        crate::engine::manager::ex_point::ExPointKind::DevicePower.as_wire(),
    );
    twins.ex_skill_level = Some(5);
    twins.ex_skill = Some(31495151);
    Fight {
        attacker: Some(FightTeam {
            entitys: vec![twins],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 1001, 0, 0)],
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn selected_unique_device(fight: &Fight) -> (TargetPool, BattleManagers, SkillEffectCatalog) {
    let pool = TargetPool::from_fight(fight);
    let mut managers = BattleManagers::seeded(fight);
    managers
        .conduit
        .execute(
            crate::engine::manager::conduit::ConduitCommand::SelectGroup {
                source_uid: 10,
                group: 3,
            },
        )
        .unwrap();
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), fight);
    (pool, managers, catalog)
}

fn operation() -> [sonettobuf::FightDeviceOper; 1] {
    [sonettobuf::FightDeviceOper {
        uid: Some(10),
        index: Some(3),
    }]
}

#[test]
fn device_overflow_is_banked_before_applied_harmonization_is_recorded() {
    init_config();
    let mut twins = FightEntityInfo {
        uid: Some(10),
        model_id: Some(3149),
        current_hp: Some(100_000),
        ex_point: Some(96),
        ex_point_max: Some(100),
        ex_point_type: Some(crate::engine::manager::ex_point::ExPointKind::DevicePower.as_wire()),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        buffs: vec![BuffInfo {
            uid: Some(20),
            buff_id: Some(31450173),
            from_uid: Some(11),
            act_info: vec![sonettobuf::BuffActInfo {
                act_id: Some(1149),
                param: vec![0],
                str_param: Some(String::new()),
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    twins.ex_skill = Some(31490151);
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![twins],
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
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    let result = crate::engine::runtime::drain::run_conduit_action(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        10,
        2,
        1,
        2,
        31490141,
        None,
        None,
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(10), 100);
    assert_eq!(
        managers.buff.snapshot(10, 20).unwrap().act_info[0].param,
        [1]
    );
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let skill = steps
        .iter()
        .find_map(|step| find_step(step, 31490141))
        .unwrap();
    fn collect_effects(step: &sonettobuf::FightStep, effects: &mut Vec<(i32, i32, i64, i32)>) {
        for effect in &step.act_effect {
            effects.push((
                effect.effect_type.unwrap_or_default(),
                effect
                    .buff_act_info
                    .as_ref()
                    .and_then(|info| info.act_id)
                    .unwrap_or_default(),
                effect.target_id.unwrap_or_default(),
                effect.effect_num.unwrap_or_default(),
            ));
            if let Some(child) = effect.fight_step.as_ref() {
                collect_effects(child, effects);
            }
        }
    }
    let mut effects = Vec::new();
    collect_effects(skill, &mut effects);
    let banked = effects
        .iter()
        .position(|(_, buff_act_id, _, _)| *buff_act_id == 1149)
        .unwrap();
    let applied = effects
        .iter()
        .position(|(effect_type, _, target_id, effect_num)| {
            *effect_type == sonettobuf::effect_type_enum::EffectType::Expointchange as i32
                && *target_id == 10
                && *effect_num == 4
        })
        .unwrap();
    assert!(banked < applied);
}

#[test]
fn conduit_unique_does_not_require_full_personal_device_power() {
    init_config();
    let fight = fight(10);
    let (pool, mut managers, mut catalog) = selected_unique_device(&fight);

    run_conduit_phase(
        managers.catalog(),
        &fight,
        &mut managers,
        &pool,
        &mut catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &operation(),
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(10), 10);
}

#[test]
fn conduit_unique_does_not_spend_personal_device_power_before_casting() {
    init_config();
    let fight = fight(100);
    let (pool, mut managers, mut catalog) = selected_unique_device(&fight);

    let result = run_conduit_phase(
        managers.catalog(),
        &fight,
        &mut managers,
        &pool,
        &mut catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &operation(),
    )
    .unwrap();

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let action = steps
        .iter()
        .find(|step| {
            step.act_effect.iter().any(|effect| {
                effect
                    .fight_step
                    .as_ref()
                    .is_some_and(|skill| skill.act_id == Some(31495151))
            })
        })
        .unwrap();
    let spend = action.act_effect.iter().position(|effect| {
        effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Expointchange as i32)
            && effect.target_id == Some(10)
            && effect.effect_num == Some(-100)
    });
    let skill = action
        .act_effect
        .iter()
        .position(|effect| {
            effect
                .fight_step
                .as_ref()
                .is_some_and(|skill| skill.act_id == Some(31495151))
        })
        .unwrap();
    assert!(spend.is_none());
    assert_eq!(managers.ex_point.get(10), 100);
    assert_eq!(action.card_index, Some(1));
    assert_eq!(action.support_hero_id, Some(1));
    assert_eq!(
        action.act_effect[skill]
            .fight_step
            .as_ref()
            .unwrap()
            .card_index,
        Some(1)
    );
}

#[test]
fn consumed_energy_reactions_follow_the_twins_unique_skill() {
    init_config();
    let entity = |uid, model_id, hp, team_type| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        team_type: Some(team_type),
        current_hp: Some(hp),
        attr: Some(HeroAttribute {
            hp: Some(hp),
            attack: Some(100_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut twins = entity(10, 3149, 100_000, 1);
    twins.ex_skill = Some(31490151);
    twins.ex_point = Some(10);
    twins.ex_point_type =
        Some(crate::engine::manager::ex_point::ExPointKind::DevicePower.as_wire());
    twins.passive_skill = vec![31490191];
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![twins],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 1001, 1, 2), entity(-2, 1001, 100_000_000, 2)],
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
                group: 3,
            },
        )
        .unwrap();
    managers
        .conduit
        .execute(
            crate::engine::manager::conduit::ConduitCommand::ChangePower(
                crate::engine::manager::conduit::ConduitPowerChange {
                    origin: crate::engine::skill::rule::CommandOrigin {
                        domain: crate::engine::skill::rule::RuleDomain::Behavior,
                        key: crate::engine::skill::rule::DefinitionKey::new(
                            60291,
                            "AddDevicePower",
                        ),
                    },
                    source_uid: 10,
                    team: 1,
                    power_id: 2,
                    delta: 3,
                    kind: crate::engine::manager::conduit::ConduitPowerChangeKind::Standard,
                },
            ),
        )
        .unwrap();
    let mut catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    let result = run_conduit_phase(
        managers.catalog(),
        &fight,
        &mut managers,
        &pool,
        &mut catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[sonettobuf::FightDeviceOper {
            uid: Some(10),
            index: Some(3),
        }],
    )
    .unwrap();
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let effects = steps
        .iter()
        .flat_map(|step| &step.act_effect)
        .find_map(|effect| {
            effect
                .fight_step
                .as_ref()
                .filter(|skill| skill.act_id == Some(31490151))
                .map(|skill| skill.act_effect.as_slice())
        })
        .expect("the unique conduit skill projects a nested step");
    let power = effects
        .iter()
        .position(|effect| effect.reserve_str.as_deref() == Some("2#-3"))
        .expect("the unique skill consumes the configured energy pool");
    let marker = effects
        .iter()
        .position(|effect| effect.config_effect == Some(100031))
        .expect("the unique skill projects its consumption marker");
    let death = effects
        .iter()
        .position(|effect| {
            effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Dead as i32)
        })
        .expect("the unique skill defeats the target");
    let gains = effects
        .iter()
        .enumerate()
        .filter_map(|(index, effect)| {
            (effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Expointchange as i32)
                && effect.target_id == Some(10)
                && effect.effect_num == Some(1))
            .then_some(index)
        })
        .collect::<Vec<_>>();

    assert_eq!(gains.len(), 3);
    assert!(power < marker);
    assert!(marker < death);
    assert!(gains.iter().all(|gain| death < *gain));
}
