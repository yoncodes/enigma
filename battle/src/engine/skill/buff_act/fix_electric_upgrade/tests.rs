use super::*;

#[test]
fn allied_modifier_replaces_the_configured_transition_destination() {
    let feature = ActiveBuffFeature {
        owner_uid: 20,
        source_uid: 20,
        buff_uid: 30,
        buff_id: 31280117,
        amount: 1,
        team_type: 1,
        owner_alive: true,
        act_type: "FixElectricUpgrade".to_owned(),
        effect_time: 0,
        effect_condition: 0,
        raw: "1032#2#40#3".to_owned(),
        values: vec![1032, 2, 40, 3],
    };
    let thresholds = resolve_thresholds(
        1,
        &[
            FieldThreshold {
                level: 2,
                progress: 50,
                definition: crate::engine::manager::field::FieldDefinition {
                    field_id: 30002,
                    duration: 3,
                },
            },
            FieldThreshold {
                level: 3,
                progress: 120,
                definition: crate::engine::manager::field::FieldDefinition {
                    field_id: 30003,
                    duration: 2,
                },
            },
        ],
        &[feature],
    );

    assert_eq!(
        thresholds,
        [
            FieldThreshold {
                level: 3,
                progress: 90,
                definition: crate::engine::manager::field::FieldDefinition {
                    field_id: 30003,
                    duration: 2,
                },
            },
            FieldThreshold {
                level: 3,
                progress: 120,
                definition: crate::engine::manager::field::FieldDefinition {
                    field_id: 30003,
                    duration: 2,
                },
            }
        ]
    );
}
