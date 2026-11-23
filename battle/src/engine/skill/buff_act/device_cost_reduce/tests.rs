use super::*;
use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

#[test]
fn conduit_uses_only_its_carriers_calibration_stack() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3134),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(31440113),
                        count: Some(2),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    model_id: Some(3149),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        uid: Some(21),
                        buff_id: Some(31440113),
                        count: Some(2),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(1001),
                current_hp: Some(100_000),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
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
    let (reduction, consume) = modifier(&managers, 11, 31490141).unwrap();
    assert_eq!(reduction, 1);
    let RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(consume_command))) = consume
    else {
        panic!("expected one exact buff consumption")
    };
    assert!(matches!(
        consume_command,
        BuffConsume {
            target_uid: 11,
            selector: BuffSelector::Uid(21),
            amount: 1,
            ..
        }
    ));
    managers
        .conduit
        .execute(
            crate::engine::manager::conduit::ConduitCommand::ChangePower(
                crate::engine::manager::conduit::ConduitPowerChange {
                    origin: consume_command.origin,
                    source_uid: 11,
                    team: 1,
                    power_id: 2,
                    delta: 1,
                    kind: crate::engine::manager::conduit::ConduitPowerChangeKind::Standard,
                },
            ),
        )
        .unwrap();
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut catalog = crate::engine::skill::effect::SkillEffectCatalog::from_fight(
        config::configs::get(),
        &fight,
    );
    crate::engine::runtime::schedule::run_conduit_phase(
        managers.catalog(),
        &fight,
        &mut managers,
        &pool,
        &mut catalog,
        &mut crate::engine::runtime::determinism::RoundDeterminism::default(),
        crate::engine::skill::target::TargetContext::default(),
        &[sonettobuf::FightDeviceOper {
            uid: Some(11),
            index: Some(2),
        }],
    )
    .unwrap();

    assert_eq!(managers.conduit.power(1, 2), 0);
    assert_eq!(
        managers
            .buff
            .buff_act_amount(11, BuffActKind::DeviceCostReduce),
        0
    );
    assert_eq!(
        managers
            .buff
            .buff_act_amount(10, BuffActKind::DeviceCostReduce),
        2
    );
}
