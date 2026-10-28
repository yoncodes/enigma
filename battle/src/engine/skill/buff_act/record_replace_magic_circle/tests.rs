use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::manager::field::{FieldDefinition, FieldOperation};

fn managers_with_circle(circle_id: i32, duration: i32) -> BattleManagers {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(31410409),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .field
        .execute_command(FieldCommand {
            origin: ORIGIN,
            team: 1,
            operation: FieldOperation::DeployIfAbsent {
                definition: FieldDefinition {
                    field_id: circle_id,
                    duration,
                },
                create_uid: 10,
                initial_level: 3,
                thresholds: Vec::new(),
            },
        })
        .unwrap();
    managers.field.begin_round();
    managers
}

fn subscriber(managers: &BattleManagers) -> BuffActSubscriber {
    let feature = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find(|feature| feature.buff_id == 31410409)
        .unwrap();
    super::super::subscriber_from_feature(feature, EventKind::RoundEndAfterFinalSettlement).unwrap()
}

#[test]
fn round_end_counts_down_then_restores_the_configured_return_circle() {
    let mut managers = managers_with_circle(42004, 2);
    let carrier = subscriber(&managers);
    let countdown_ops = round_end_rule_ops(&managers, &carrier).unwrap();
    let [
        BuffActRuleOp {
            op: RuleOp::Command(BattleCommand::Field(countdown)),
            frame_owner: BuffActFrameOwner::Command,
            ..
        },
    ] = countdown_ops.as_slice()
    else {
        panic!("expected one duration countdown");
    };
    managers.field.execute_command(countdown.clone()).unwrap();

    let restore = round_end_rule_ops(&managers, &carrier).unwrap();
    assert!(matches!(
        restore.last(),
        Some(BuffActRuleOp {
            op: RuleOp::Command(BattleCommand::Field(FieldCommand {
                operation: FieldOperation::DeployReplacement {
                    definition: FieldDefinition {
                        field_id: 42003,
                        duration: -1,
                    },
                    level: 0,
                    ..
                },
                ..
            })),
            scope: BuffActFrameScope::CausingFrame,
            ..
        })
    ));
}

#[test]
fn newly_deployed_circle_waits_until_the_next_round() {
    let mut managers = managers_with_circle(42004, 4);
    managers
        .field
        .execute_command(FieldCommand {
            origin: ORIGIN,
            team: 1,
            operation: FieldOperation::Remove,
        })
        .unwrap();
    managers
        .field
        .execute_command(FieldCommand {
            origin: ORIGIN,
            team: 1,
            operation: FieldOperation::DeployReplacement {
                definition: FieldDefinition {
                    field_id: 42004,
                    duration: 4,
                },
                owner_uid: 10,
                create_uid: -1,
                level: 0,
            },
        })
        .unwrap();

    let carrier = subscriber(&managers);
    assert!(round_end_rule_ops(&managers, &carrier).unwrap().is_empty());
}

#[test]
fn missing_return_mapping_fails_before_countdown() {
    let mut managers = managers_with_circle(999999, 2);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let event =
        crate::engine::event::payload::BattleEvent::Kind(EventKind::RoundEndAfterFinalSettlement);

    assert!(matches!(
        crate::engine::runtime::drain::run_group_event(
            &mut managers,
            &pool,
            &crate::engine::skill::effect::SkillEffectCatalog::default(),
            &mut determinism,
            crate::engine::skill::target::TargetContext::default(),
            event,
            crate::engine::runtime::drain::ReactionLane::BuffActs,
            None,
        ),
        Err(crate::engine::runtime::drain::DrainError::BuffActRule(
            super::super::BuffActRuleError::RecordReplaceMagicCircle(
                RecordReplaceMagicCircleError::MissingCircle(999999),
            )
        ))
    ));
    assert_eq!(managers.field.get(1).unwrap().definition.duration, 2);
}
