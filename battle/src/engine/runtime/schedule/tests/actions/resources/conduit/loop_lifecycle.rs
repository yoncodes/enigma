use super::*;

#[test]
fn conduit_attacks_complete_loop_chain_and_grant_moxie() {
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
            entitys: vec![
                entity(10, 3149),
                FightEntityInfo {
                    ex_point: Some(4),
                    ex_skill: Some(31430131),
                    passive_skill: vec![31430141, 31430151],
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
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "AddBuff"),
    };
    managers
        .execute_buff(crate::engine::manager::buff::BuffCommand::Grant(
            crate::engine::manager::buff::BuffGrant {
                origin,
                source_uid: 30,
                target_uid: 30,
                buff_id: 31430151,
                amount: Some(3),
                occurrences: 1,
                child_uid_reservations: 0,
            },
        ))
        .unwrap();
    managers
        .conduit
        .execute(
            crate::engine::manager::conduit::ConduitCommand::ChangePower(
                crate::engine::manager::conduit::ConduitPowerChange {
                    origin,
                    source_uid: 10,
                    team: 1,
                    power_id: 1,
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
            index: Some(1),
        }],
    )
    .unwrap();

    for skill_id in [31490111, 31490121] {
        assert!(result.events.iter().any(|event| matches!(
            event,
            BattleEvent::AllyAction(action)
                if action.skill_id == skill_id && action.mode == SkillExecutionMode::Device
        )));
    }
    assert_eq!(managers.buff.buff_id_amount(30, 31430151), 1);
    assert_eq!(managers.ex_point.get(30), 5);

    fn find_step(step: &sonettobuf::FightStep, act_id: i32) -> Option<&sonettobuf::FightStep> {
        (step.act_id == Some(act_id)).then_some(step).or_else(|| {
            step.act_effect
                .iter()
                .filter_map(|effect| effect.fight_step.as_ref())
                .find_map(|nested| find_step(nested, act_id))
        })
    }
    fn collect_loop_chain_layers(step: &sonettobuf::FightStep, layers: &mut Vec<i32>) {
        for effect in &step.act_effect {
            if let Some(buff) = effect
                .buff
                .as_ref()
                .filter(|buff| buff.buff_id == Some(31430151))
            {
                layers.push(buff.layer.unwrap_or_default());
            }
            if let Some(nested) = effect.fight_step.as_ref() {
                collect_loop_chain_layers(nested, layers);
            }
        }
    }

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let mut layers = Vec::new();
    for step in &steps {
        collect_loop_chain_layers(step, &mut layers);
    }
    assert!(layers.windows(2).any(|layers| layers == [3, 4]));
    assert!(layers.windows(2).any(|layers| layers == [4, 5]));
    let threshold = steps
        .iter()
        .find_map(|step| find_step(step, 31430141))
        .unwrap();
    assert!(threshold.act_effect.iter().any(|effect| {
        effect
            .fight_step
            .as_ref()
            .is_some_and(|step| step.act_id == Some(31430181))
    }));
}

#[test]
fn conduit_attacks_keep_their_lifecycle_without_becoming_active_incantations() {
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
            entitys: vec![entity(20, 3134), entity(10, 3149)],
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
    managers
        .execute_buff(crate::engine::manager::buff::BuffCommand::Grant(
            crate::engine::manager::buff::BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 20,
                target_uid: 20,
                buff_id: 31340006,
                amount: Some(1),
                occurrences: 1,
                child_uid_reservations: 0,
            },
        ))
        .unwrap();
    assert!(managers.emanation.select(20, 10));
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
            index: Some(1),
        }],
    )
    .unwrap();
    assert!(result.outcomes.iter().any(|outcome| matches!(
        outcome,
        RuleOutcome::SkillLifecycle(SkillLifecycle::ActionCompleted(action))
            if action.mode == SkillExecutionMode::Device && action.card_index == 0
    )));
    assert!(result.events.iter().any(|event| matches!(
        event,
        BattleEvent::SkillAction(action)
            if action.mode == SkillExecutionMode::Device
                && action.is_attack
    )));
    assert_eq!(managers.buff.buff_id_amount(-1, 31340001), 0);
}

#[test]
fn conduit_repeats_a_paid_skill_until_its_energy_is_spent() {
    init_config();
    let entity = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 3149)],
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
    let origin = crate::engine::skill::rule::CommandOrigin {
        domain: crate::engine::skill::rule::RuleDomain::Behavior,
        key: crate::engine::skill::rule::DefinitionKey::new(60291, "AddDevicePower"),
    };
    managers
        .conduit
        .execute(
            crate::engine::manager::conduit::ConduitCommand::ChangePower(
                crate::engine::manager::conduit::ConduitPowerChange {
                    origin,
                    source_uid: 10,
                    team: 1,
                    power_id: 1,
                    delta: 2,
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
            index: Some(1),
        }],
    )
    .unwrap();

    assert_eq!(managers.conduit.power(1, 1), 0);
    assert_eq!(managers.conduit.consumed(1, 1), 6);
    assert_eq!(managers.conduit.uses(10), 3);
    let running = crate::engine::packet::timeline::project(&result.frames)
        .unwrap()
        .into_iter()
        .filter_map(|step| {
            step.act_effect
                .iter()
                .find(|effect| {
                    effect.effect_type
                        == Some(sonettobuf::effect_type_enum::EffectType::Devicerunning as i32)
                })
                .map(|effect| {
                    (
                        step.from_id,
                        step.card_index,
                        step.support_hero_id,
                        effect.effect_num,
                    )
                })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        running,
        [
            (Some(10), Some(1), Some(1), Some(1)),
            (Some(10), Some(1), Some(2), Some(1)),
            (Some(10), Some(1), Some(1), Some(0)),
            (Some(10), Some(1), Some(2), Some(1)),
            (Some(10), Some(1), Some(1), Some(0)),
            (Some(10), Some(1), Some(2), Some(0)),
        ]
    );
}
