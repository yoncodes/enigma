use super::*;
use crate::engine::{
    manager::hp::{DamageEffectKind, HpCommand, HpDamage, HurtDamageFromType, HurtInfoData},
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};
use sonettobuf::{BuffInfo, FightTeam};

#[test]
fn damage_crosses_segments_and_reports_wire_deltas() {
    let mut manager = ToughnessManager::default();
    manager.states.insert(
        -1,
        ToughnessState {
            value: 100,
            point: 3,
            segment_value: 100,
            max_point: 3,
            team_type: 2,
            broken: false,
        },
    );

    let first = manager.reduce(-1, 80, true).unwrap();
    assert_eq!(
        (first.value_delta, first.point_delta, first.broke),
        (80, 0, false)
    );

    let crossed = manager.reduce(-1, 30, true).unwrap();
    assert_eq!(
        (crossed.value_delta, crossed.point_delta, crossed.broke),
        (-70, 1, false)
    );

    let broken = manager.reduce(-1, 190, true).unwrap();
    assert_eq!(
        (broken.value_delta, broken.point_delta, broken.broke),
        (90, 2, true)
    );
    assert_eq!(manager.get(-1).unwrap().point, 0);
}

#[test]
fn ordinary_afflatus_reduces_one_fifth_of_damage() {
    let mut manager = ToughnessManager::default();
    manager.states.insert(
        -1,
        ToughnessState {
            value: 100,
            point: 1,
            segment_value: 100,
            max_point: 1,
            team_type: 2,
            broken: false,
        },
    );

    let change = manager.reduce(-1, 99, false).unwrap();
    assert_eq!(change.value_delta, 19);
}

#[test]
fn configured_percent_builds_each_guard_segment_from_max_hp() {
    crate::test_support::init_config();
    let mut manager = ToughnessManager::default();
    manager.register_with_game_data(
        crate::test_support::game_data(),
        &FightEntityInfo {
            uid: Some(-1),
            model_id: Some(109_350_003),
            toughness_value: Some(101_500),
            toughness_point: Some(4),
            attr: Some(sonettobuf::HeroAttribute {
                hp: Some(1_015_000),
                ..Default::default()
            }),
            ..Default::default()
        },
    );

    assert_eq!(manager.get(-1).unwrap().segment_value, 101_500);
    assert_eq!(manager.get(-1).unwrap().max_point, 4);
}

#[test]
fn recovery_only_resets_a_broken_guard() {
    let mut manager = ToughnessManager::default();
    manager.states.insert(
        -1,
        ToughnessState {
            value: 0,
            point: 0,
            segment_value: 100,
            max_point: 3,
            team_type: 2,
            broken: true,
        },
    );
    let recovery = manager
        .recover(ToughnessRecover {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60287, "ToughnessRecover"),
            },
            target_uid: -1,
            config_effect: 60287,
        })
        .unwrap();

    assert_eq!(
        (
            recovery.point,
            recovery.value,
            recovery.config_effect,
            recovery.team_type,
        ),
        (3, 100, 60287, 2)
    );
    assert_eq!(
        manager.get(-1),
        Some(ToughnessState {
            value: 100,
            point: 3,
            segment_value: 100,
            max_point: 3,
            team_type: 2,
            broken: false,
        })
    );
}

#[test]
fn recorded_break_damage_reduces_the_recovered_segment() {
    let mut manager = ToughnessManager::default();
    manager.states.insert(
        -1,
        ToughnessState {
            value: 0,
            point: 0,
            segment_value: 101_500,
            max_point: 3,
            team_type: 2,
            broken: true,
        },
    );
    manager.record_broken_damage(ToughnessRecord {
        target_uid: -1,
        damage: 203_000,
        rate_permille: STANDARD_DAMAGE_RATE_PERMILLE,
    });

    let recovery = manager
        .recover(ToughnessRecover {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60_287, "ToughnessRecover"),
            },
            target_uid: -1,
            config_effect: 60_287,
        })
        .unwrap();

    assert_eq!((recovery.point, recovery.value), (3, 60_900));
    assert_eq!(manager.get(-1).unwrap().value, 60_900);
}

#[test]
fn skill_damage_commits_guard_break_with_the_hp_transaction() {
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1_000),
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                toughness_value: Some(100),
                toughness_point: Some(1),
                is_broken: Some(false),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = super::super::BattleManagers::seeded(&fight);
    let changes = managers.execute_hp(guard_hit(100)).unwrap();

    assert!(changes.toughness.unwrap().broke);
    assert!(changes.events().iter().any(|event| matches!(
        event,
        crate::engine::event::payload::BattleEvent::ToughnessBroken {
            source_uid: 1,
            target_uid: -1,
            skill_id: 1,
            target_defeated: false,
        }
    )));
    assert!(managers.toughness.get(-1).unwrap().broken);
}

fn guard_hit(amount: i32) -> HpCommand {
    HpCommand::Damage(HpDamage {
        origin: CommandOrigin {
            domain: RuleDomain::Skill,
            key: DefinitionKey::new(1, "SkillDamage"),
        },
        source_uid: 1,
        target_uid: -1,
        amount,
        config_effect: 0,
        effect_kind: DamageEffectKind::Normal,
        assassinate: false,
        ignore_riposte: false,
        hurt: HurtInfoData {
            from_uid: 1,
            is_crit: false,
            career_restraint: true,
            reduce_hp: 0,
            effect_id: 0,
            skill_id: 1,
            damage_from: HurtDamageFromType::Skill,
            buff_act_id: 0,
            buff_uid: 0,
            hurt_effect_type: 0,
            display_amount: None,
        },
    })
}

fn overflow_managers(has_record: bool) -> super::super::BattleManagers {
    crate::test_support::init_config();
    super::super::BattleManagers::seeded(&Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(21_746_250),
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(21_746_250),
                    ..Default::default()
                }),
                toughness_value: Some(1_739_700),
                toughness_point: Some(3),
                is_broken: Some(false),
                team_type: Some(2),
                buffs: has_record
                    .then_some(BuffInfo {
                        buff_id: Some(116_362_200),
                        uid: Some(20),
                        from_uid: Some(-1),
                        ..Default::default()
                    })
                    .into_iter()
                    .collect(),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    })
}

fn recover_with_break_loss(manager: &mut ToughnessManager) -> (i32, i32) {
    manager.record_broken_damage(ToughnessRecord {
        target_uid: -1,
        damage: 4_349_250,
        rate_permille: STANDARD_DAMAGE_RATE_PERMILLE,
    });
    let recovery = manager
        .recover(ToughnessRecover {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60287, "ToughnessRecover"),
            },
            target_uid: -1,
            config_effect: 60287,
        })
        .unwrap();
    (recovery.point, recovery.value)
}

#[test]
fn recovery_spends_break_loss_against_recorded_segment_overflow() {
    let mut managers = overflow_managers(true);
    let mut recover_after_hits = |hits: &[i32]| {
        for &amount in hits {
            managers.execute_hp(guard_hit(amount)).unwrap();
        }
        recover_with_break_loss(&mut managers.toughness)
    };

    assert_eq!(
        recover_after_hits(&[22_184, 3_013_646, 192_817, 354_035, 1_551_984, 1_393_470]),
        (2, 1_210_138)
    );
    assert_eq!(
        recover_after_hits(&[601_647, 805_206, 956_768, 1_359_495]),
        (3, 143_573)
    );
}

#[test]
fn segment_overflow_cannot_disable_recovery_for_a_surviving_target() {
    for (has_record, expected) in [(true, (1, 1)), (false, (3, 869_850))] {
        let mut managers = overflow_managers(has_record);
        managers.execute_hp(guard_hit(4_953_210)).unwrap();
        managers.execute_hp(guard_hit(5_993_306)).unwrap();
        assert_eq!(recover_with_break_loss(&mut managers.toughness), expected);
        let response = managers.execute_hp(guard_hit(1)).unwrap();
        assert_eq!(response.toughness.unwrap().broke, has_record);
        if has_record {
            assert!(managers.hp.current(-1) > 0);
            assert!(recover_with_break_loss(&mut managers.toughness).0 > 0);
        }
    }
}
