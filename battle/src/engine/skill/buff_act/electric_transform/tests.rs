use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::manager::BattleManagers;

#[test]
fn configured_extra_value_uses_the_carrier_layer() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    buff_id: Some(31430141),
                    uid: Some(2),
                    layer: Some(6),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);

    assert_eq!(
        extra_transfers(&managers.buff, &managers.hp, 10),
        vec![ExtraTransfer {
            buff_uid: 2,
            consumed_layers: 6,
            progress: 6,
        }]
    );
}

#[test]
fn recorded_team_damage_bonus_keeps_its_separate_multiplier_lane() {
    use crate::engine::{
        manager::buff::{BuffAccumulateActValue, BuffCommand, BuffGrant},
        skill::rule::{DefinitionKey, RuleDomain},
    };

    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        buff_id: Some(31430145),
                        uid: Some(2),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_buff(BuffCommand::AccumulateActValue(BuffAccumulateActValue {
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(1128, "RecordTeamExElectricTransConsumeValue"),
            },
            target_uid: 10,
            buff_uid: 2,
            act_id: 1128,
            delta: 30,
        }))
        .unwrap();
    managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid: 10,
            target_uid: 10,
            buff_id: 31430131,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();

    assert_eq!(
        team_damage_multiplier_delta(&managers.buff, &managers.hp, 10),
        600
    );
    assert_eq!(
        team_damage_multiplier_delta(&managers.buff, &managers.hp, 11),
        600
    );
}
