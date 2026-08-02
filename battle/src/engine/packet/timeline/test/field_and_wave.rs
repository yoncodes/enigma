use super::*;

#[test]
fn field_deploy_projects_the_complete_committed_snapshot() {
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(50019, "AddMagicCircle"),
    };
    let mut fields = FieldManager::default();
    let change = fields
        .execute_command(FieldCommand {
            origin,
            team: 1,
            operation: FieldOperation::DeployIfAbsent {
                definition: FieldDefinition {
                    field_id: 30001,
                    duration: 3,
                },
                create_uid: 10,
                initial_level: 1,
                thresholds: vec![
                    FieldThreshold {
                        level: 1,
                        progress: 0,
                        definition: FieldDefinition {
                            field_id: 30001,
                            duration: 3,
                        },
                    },
                    FieldThreshold {
                        level: 2,
                        progress: 90,
                        definition: FieldDefinition {
                            field_id: 30002,
                            duration: 3,
                        },
                    },
                ],
            },
        })
        .unwrap();

    let effects = project_change_for_test(&BattleChange::Field(change)).unwrap();

    let info = effects[0].magic_circle.unwrap();
    assert_eq!(effects[0].target_id, Some(10));
    assert_eq!(info.magic_circle_id, Some(30001));
    assert_eq!(info.create_uid, Some(10));
    assert_eq!(info.electric_level, Some(1));
    assert_eq!(info.electric_progress, Some(0));
    assert_eq!(info.max_electric_progress, Some(90));

    let duration = fields
        .execute_command(FieldCommand {
            origin,
            team: 1,
            operation: FieldOperation::ChangeDuration { delta: -1 },
        })
        .unwrap();
    let effects = project_change_for_test(&BattleChange::Field(duration)).unwrap();
    assert_eq!(
        effects[0].effect_type,
        Some(EffectType::Magiccircleupdate as i32)
    );
    assert_eq!(effects[0].reserve_str.as_deref(), Some("-1"));
    assert_eq!(effects[0].magic_circle.as_ref().unwrap().round, Some(2));

    let progress = fields
        .execute_command(FieldCommand {
            origin,
            team: 1,
            operation: FieldOperation::ChangeProgress { delta: 120 },
        })
        .unwrap();
    let effects = project_change_for_test(&BattleChange::Field(progress)).unwrap();
    assert_eq!(
        effects[0].effect_type,
        Some(EffectType::Magiccircleupdate as i32)
    );
    let level = fields
        .execute_command(FieldCommand {
            origin,
            team: 1,
            operation: FieldOperation::ResolveLevel {
                thresholds: vec![FieldThreshold {
                    level: 3,
                    progress: 120,
                    definition: FieldDefinition {
                        field_id: 30003,
                        duration: 2,
                    },
                }],
            },
        })
        .unwrap();
    let effects = project_change_for_test(&BattleChange::Field(level)).unwrap();
    let info = effects[0].magic_circle.as_ref().unwrap();
    assert_eq!(
        effects[0].effect_type,
        Some(EffectType::Magiccircleupgrade as i32)
    );
    assert_eq!(info.magic_circle_id, Some(30003));
    assert_eq!(info.electric_level, Some(3));
    assert_eq!(info.electric_progress, Some(120));
    assert_eq!(info.max_electric_progress, Some(120));
}

#[test]
fn wave_projection_uses_the_owned_fight_snapshot() {
    let change = crate::engine::manager::wave::WaveAdvanced {
        wave: 2,
        entering_uids: vec![-3, -4],
        fight: sonettobuf::Fight {
            cur_round: Some(2),
            cur_wave: Some(2),
            ..Default::default()
        },
    };

    let effects = project_change_for_test(&BattleChange::WaveAdvanced(Box::new(change))).unwrap();

    assert_eq!(
        effects[0].effect_type,
        Some(EffectType::Newchangewave as i32)
    );
    assert_eq!(effects[0].fight.as_ref().unwrap().cur_round, Some(2));
    assert_eq!(effects[0].fight.as_ref().unwrap().cur_wave, Some(2));
}
