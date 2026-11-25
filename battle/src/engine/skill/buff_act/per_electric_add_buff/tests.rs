use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::{
    manager::{
        buff::BuffSelector,
        field::{FieldCommand, FieldDefinition, FieldOperation, FieldThreshold},
    },
    mechanic::field_transfer::FieldTransferCommand,
    runtime::{determinism::RoundDeterminism, drain},
    skill::{
        effect::SkillEffectCatalog,
        rule::{CommandOrigin, DefinitionKey, RuleDomain},
        target::TargetPool,
    },
};

#[test]
fn transfer_reward_keeps_remainder_after_field_redeployment() {
    crate::test_support::init_config();
    let entity = |uid, buff_id, layer| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(1),
        current_hp: Some(100),
        buffs: vec![BuffInfo {
            uid: Some(uid + 100),
            buff_id: Some(buff_id),
            from_uid: Some(uid),
            layer,
            ..Default::default()
        }],
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 31350211, None), entity(11, 90071, Some(40))],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    let catalog = SkillEffectCatalog::default();
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60195, "ElectricTransform"),
    };
    let field = |operation| {
        BattleCommand::Field(FieldCommand {
            origin,
            team: 1,
            operation,
        })
    };
    let deploy = || {
        field(FieldOperation::DeployIfAbsent {
            definition: FieldDefinition {
                field_id: 30001,
                duration: 3,
            },
            create_uid: 10,
            initial_level: 1,
            thresholds: vec![FieldThreshold {
                level: 2,
                progress: 90,
                definition: FieldDefinition {
                    field_id: 30002,
                    duration: 3,
                },
            }],
        })
    };
    let transfer = |limit| {
        BattleCommand::FieldTransfer(FieldTransferCommand {
            origin,
            target_uid: 11,
            buff: BuffSelector::IdOrType(90071),
            limit,
            team: 1,
        })
    };
    let mut run = |managers: &mut BattleManagers, command| {
        drain::run_command_group(
            managers,
            &pool,
            &catalog,
            &mut determinism,
            Default::default(),
            [RuleOp::Command(command)],
        )
        .unwrap();
    };

    run(&mut managers, deploy());
    run(&mut managers, transfer(15));
    assert_eq!(managers.buff.buff_id_or_type_amount(10, 31350131), 0);
    assert_eq!(managers.buff.act_value(110, 1076), 15);

    managers.begin_round();
    run(&mut managers, field(FieldOperation::Remove));
    run(&mut managers, deploy());
    run(&mut managers, transfer(25));
    assert_eq!(managers.buff.buff_id_or_type_amount(10, 31350131), 10);
    assert_eq!(managers.buff.buff_id_or_type_amount(11, 31350131), 0);
    assert_eq!(managers.buff.act_value(110, 1076), 0);
}
