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
