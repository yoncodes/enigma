use super::*;
use crate::engine::skill::rule::{DefinitionKey, RuleDomain};
use sonettobuf::{BuffActInfo, BuffInfo, Fight, FightEntityInfo, FightTeam};

fn origin() -> CommandOrigin {
    CommandOrigin {
        domain: RuleDomain::BuffAct,
        key: DefinitionKey::new(1158, "HedoneHurtCounter"),
    }
}

fn fight_with_carriers(buff_uids: &[i64], target_hp: i32) -> Fight {
    Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-4),
                current_hp: Some(1_000_000),
                buffs: buff_uids
                    .iter()
                    .map(|uid| BuffInfo {
                        uid: Some(*uid),
                        buff_id: Some(31560102),
                        from_uid: Some(-4),
                        act_info: vec![BuffActInfo {
                            act_id: Some(1158),
                            param: vec![0],
                            str_param: Some(String::new()),
                        }],
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                current_hp: Some(target_hp),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn record(amount: i32) -> HedoneCounterCommand {
    HedoneCounterCommand::Record(HedoneCounterRecord {
        origin: origin(),
        buff_uid: 10,
        visible: 0,
        amount,
    })
}

fn settle() -> HedoneCounterSettle {
    HedoneCounterSettle {
        origin: origin(),
        buff_uid: 10,
        visible: 0,
        retained_rounds: 4,
    }
}

#[test]
fn retains_only_the_configured_round_window() {
    let mut manager = HedoneCounterManager::default();
    for amount in [10, 20, 30, 40] {
        manager.execute(record(amount)).unwrap();
        manager
            .execute(HedoneCounterCommand::Settle(settle()))
            .unwrap();
    }
    assert_eq!(manager.value(10, 0), Ok(100));

    manager.execute(record(50)).unwrap();
    manager
        .execute(HedoneCounterCommand::Settle(settle()))
        .unwrap();
    assert_eq!(manager.value(10, 0), Ok(140));
}

#[test]
fn rejects_a_visible_counter_without_exact_round_history() {
    let mut manager = HedoneCounterManager::default();
    let command = HedoneCounterCommand::Record(HedoneCounterRecord {
        origin: origin(),
        buff_uid: 10,
        visible: 300,
        amount: 20,
    });
    assert_eq!(
        manager.execute(command),
        Err(HedoneCounterError::MissingHistory {
            buff_uid: 10,
            visible: 300
        })
    );
    assert_eq!(
        manager.settled_value(HedoneCounterSettle {
            visible: 300,
            ..settle()
        }),
        Err(HedoneCounterError::MissingHistory {
            buff_uid: 10,
            visible: 300
        })
    );

    manager
        .execute(HedoneCounterCommand::Clear {
            origin: origin(),
            buff_uid: 10,
        })
        .unwrap();
    assert_eq!(manager.value(10, 0), Ok(0));
}

#[test]
fn stored_damage_uses_the_configured_rate_and_origin_provenance() {
    crate::test_support::init_config();
    let fight = fight_with_carriers(&[1070], 1_000_000);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .hedone_counter
        .execute(HedoneCounterCommand::Record(HedoneCounterRecord {
            origin: origin(),
            buff_uid: 1070,
            visible: 0,
            amount: 306_698,
        }))
        .unwrap();

    let execution = damage(
        &mut managers,
        HedoneDamageCommand {
            origin: origin(),
            source_uid: -4,
            target_uid: 1,
            rate: 1000,
            config_effect: 60332,
            skill_id: 31560133,
        },
    )
    .unwrap()
    .unwrap();
    let hp = execution.changes.hp.unwrap();
    assert_eq!(hp.delta, -306_698);
    assert_eq!(hp.config_effect, 60332);
    assert!(matches!(
        hp.hurt,
        Some(super::super::hp::HurtInfoData {
            hurt_effect_type,
            is_crit: false,
            ..
        }) if hurt_effect_type == sonettobuf::effect_type_enum::EffectType::Origindamage as i32
    ));

    let secondary = damage(
        &mut managers,
        HedoneDamageCommand {
            origin: origin(),
            source_uid: -4,
            target_uid: 1,
            rate: 500,
            config_effect: 60332,
            skill_id: 31560133,
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(secondary.changes.hp.unwrap().delta, -153_349);
}

#[test]
fn stored_damage_rejects_missing_carrier() {
    let mut managers = BattleManagers::default();
    assert!(matches!(
        damage(
            &mut managers,
            HedoneDamageCommand {
                origin: origin(),
                source_uid: -4,
                target_uid: 1,
                rate: 500,
                config_effect: 60332,
                skill_id: 31560133,
            }
        ),
        Err(HedoneDamageError::MissingCarrier)
    ));
}

#[test]
fn stored_damage_rejects_multiple_carriers() {
    crate::test_support::init_config();
    let mut managers = BattleManagers::seeded(&fight_with_carriers(&[1070, 1071], 1_000_000));
    assert!(matches!(
        damage(
            &mut managers,
            HedoneDamageCommand {
                origin: origin(),
                source_uid: -4,
                target_uid: 1,
                rate: 1000,
                config_effect: 60332,
                skill_id: 31560133,
            }
        ),
        Err(HedoneDamageError::MultipleCarriers)
    ));
}

#[test]
fn stored_damage_projects_the_committed_origin_damage() {
    crate::test_support::init_config();
    let mut managers = BattleManagers::seeded(&fight_with_carriers(&[1070], 100_000));
    managers
        .hedone_counter
        .execute(HedoneCounterCommand::Record(HedoneCounterRecord {
            origin: origin(),
            buff_uid: 1070,
            visible: 0,
            amount: 306_698,
        }))
        .unwrap();

    let execution = damage(
        &mut managers,
        HedoneDamageCommand {
            origin: origin(),
            source_uid: -4,
            target_uid: 1,
            rate: 1000,
            config_effect: 60332,
            skill_id: 31560133,
        },
    )
    .unwrap()
    .unwrap();
    let effect = crate::engine::packet::timeline::project_change_for_test(
        &crate::engine::runtime::change::BattleChange::Hp(Box::new(execution.changes)),
    )
    .unwrap()
    .into_iter()
    .find(|effect| {
        effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Origindamage as i32)
    })
    .expect("stored damage projects an ORIGINDAMAGE effect");
    let hurt = effect.hurt_info.unwrap();

    assert_eq!(
        effect.effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Origindamage as i32)
    );
    assert_eq!(effect.effect_num, Some(306_698));
    assert_eq!(effect.config_effect, Some(60332));
    assert_eq!(hurt.reduce_hp, Some(-100_000));
    assert_eq!(hurt.effect_id, Some(31560133));
    assert_eq!(hurt.skill_id, Some(31560133));
    assert_eq!(hurt.from_uid, Some(-4));
}
