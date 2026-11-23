use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;

fn managers_and_pool() -> (BattleManagers, TargetPool) {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(99_998),
                current_hp: Some(1),
                buffs: vec![
                    BuffInfo {
                        uid: Some(10),
                        buff_id: Some(31_130_139),
                        from_uid: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(11),
                        buff_id: Some(31_130_134),
                        from_uid: Some(1),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(1),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(31_130_122),
                        layer: Some(3),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    current_hp: Some(1),
                    buffs: vec![
                        BuffInfo {
                            uid: Some(21),
                            buff_id: Some(31_130_122),
                            layer: Some(2),
                            ..Default::default()
                        },
                        BuffInfo {
                            uid: Some(22),
                            buff_id: Some(31_130_122),
                            layer: Some(4),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-3),
                    current_hp: Some(1),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    (
        BattleManagers::seeded(&fight),
        TargetPool::from_fight(&fight),
    )
}

#[test]
fn final_attack_targets_only_interpretation_holders_at_full_added_rate() {
    let (managers, pool) = managers_and_pool();

    assert!(resolve(&managers, &pool, 99_998, 10, 11).is_none());
    let plan = resolve(&managers, &pool, 99_998, 11, 11).unwrap();

    assert_eq!(plan.targets(), vec![-1, -2]);
    assert_eq!(plan.rate_term(-1).unwrap().rate, 7_200);
    assert_eq!(plan.rate_term(-2).unwrap().rate, 14_400);
    assert_eq!(plan.rate_term(-3), None);
}

#[test]
fn cleanup_uses_configured_carrier_then_removes_marks_and_rend_carrier() {
    let (managers, pool) = managers_and_pool();
    let plan = resolve(&managers, &pool, 99_998, 11, 11).unwrap();

    assert!(matches!(
        plan.before_damage_rule_ops().as_slice(),
        [RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(
            BuffRemove {
                target_uid: 99_998,
                selector: BuffRemoveSelector::ExactId(31_130_134),
                ..
            }
        )))]
    ));
    assert!(matches!(
        plan.after_damage_rule_ops().as_slice(),
        [
            RuleOp::Command(BattleCommand::BuffBatch(removals)),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
                target_uid: 99_998,
                selector: BuffRemoveSelector::Uid(10),
                ..
            })))
        ] if matches!(removals.as_slice(), [
            BuffCommand::Remove(BuffRemove {
                target_uid: -1,
                selector: BuffRemoveSelector::ExactId(31_130_122),
                ..
            }),
            BuffCommand::Remove(BuffRemove {
                target_uid: -2,
                selector: BuffRemoveSelector::ExactId(31_130_122),
                ..
            })
        ])
    ));
}
