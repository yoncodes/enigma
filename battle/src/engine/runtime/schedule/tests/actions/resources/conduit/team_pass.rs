use super::*;

#[test]
fn conduit_devices_share_power_in_team_turns() {
    init_config();
    let entity = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(100),
            ..Default::default()
        }),
        ex_skill_level: Some(2),
        destiny_stone: Some(302502),
        destiny_rank: Some(4),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 3025), entity(11, 3025)],
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
        .conduit
        .execute(
            crate::engine::manager::conduit::ConduitCommand::ChangePower(
                crate::engine::manager::conduit::ConduitPowerChange {
                    origin: CommandOrigin {
                        domain: RuleDomain::Behavior,
                        key: DefinitionKey::new(60291, "AddDevicePower"),
                    },
                    source_uid: 10,
                    team: 1,
                    power_id: 2,
                    delta: 4,
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
        &[
            sonettobuf::FightDeviceOper {
                uid: Some(10),
                index: Some(1),
            },
            sonettobuf::FightDeviceOper {
                uid: Some(11),
                index: Some(1),
            },
        ],
    )
    .unwrap();

    let sources = result
        .frames
        .iter()
        .filter_map(|frame| match frame.owner {
            crate::engine::runtime::record::FrameOwner::ConduitAction { source_uid, .. } => {
                Some(source_uid)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(sources, [10, 11, 10, 11]);
    assert_eq!(managers.conduit.uses(10), 2);
    assert_eq!(managers.conduit.uses(11), 2);
}

#[test]
fn a_later_device_can_enable_an_earlier_device_on_the_next_team_pass() {
    init_config();
    let entity = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
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
            entitys: vec![entity(10, 3149), entity(11, 3149)],
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
        key: DefinitionKey::new(60294, "StopDeviceSkill"),
    };
    for (source_uid, skill_id) in [(10, 31490111), (11, 31490121)] {
        managers
            .conduit
            .execute(crate::engine::manager::conduit::ConduitCommand::StopSkill {
                origin,
                source_uid,
                team: 1,
                skill_id,
            })
            .unwrap();
    }
    let mut catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    let result = run_conduit_phase(
        managers.catalog(),
        &fight,
        &mut managers,
        &pool,
        &mut catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[
            sonettobuf::FightDeviceOper {
                uid: Some(10),
                index: Some(1),
            },
            sonettobuf::FightDeviceOper {
                uid: Some(11),
                index: Some(1),
            },
        ],
    )
    .unwrap();

    let sources = result
        .frames
        .iter()
        .filter_map(|frame| match frame.owner {
            crate::engine::runtime::record::FrameOwner::ConduitAction { source_uid, .. } => {
                Some(source_uid)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(sources, [11, 10]);
    assert_eq!(managers.conduit.uses(10), 1);
    assert_eq!(managers.conduit.uses(11), 1);
}

#[test]
fn conduit_finishes_non_attack_devices_after_battle_ends() {
    init_config();
    let entity = |uid, model_id, hp| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(hp),
        attr: Some(HeroAttribute {
            hp: Some(hp),
            attack: Some(10_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(10, 3149, 100_000),
                entity(11, 3145, 100_000),
                entity(12, 3149, 100_000),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 1001, 1)],
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
                source_uid: 11,
                group: 2,
            },
        )
        .unwrap();
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
                    delta: 6,
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
        &[
            sonettobuf::FightDeviceOper {
                uid: Some(10),
                index: Some(1),
            },
            sonettobuf::FightDeviceOper {
                uid: Some(11),
                index: Some(1),
            },
            sonettobuf::FightDeviceOper {
                uid: Some(12),
                index: Some(1),
            },
        ],
    )
    .unwrap();

    assert_eq!(managers.hp.current(-1), 0);
    assert_eq!(managers.conduit.uses(11), 1);
    assert_eq!(managers.ex_point.get(11), 10);
    assert_eq!(managers.conduit.uses(12), 1);
    assert_eq!(
        result
            .frames
            .iter()
            .filter(|frame| matches!(
                frame.owner,
                crate::engine::runtime::record::FrameOwner::ConduitStopped { source_uid: 12, .. }
            ))
            .count(),
        2
    );
    assert!(!result.outcomes.iter().any(|outcome| matches!(
        outcome,
        RuleOutcome::Conduit(
            crate::engine::manager::conduit::ConduitChange::SkillFinished {
                source_uid: 12,
                skill_id: 31490121,
                ..
            }
        )
    )));
}
