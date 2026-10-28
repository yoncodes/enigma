use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, effect_type_enum::EffectType};

use super::*;
use crate::engine::{
    manager::{
        buff::CommandOrigin,
        field::{FieldChangeKind, FieldCommand, FieldDefinition, FieldOperation},
    },
    mechanic::magic_circle_level::MagicCircleLevelCommand,
    skill::rule::{DefinitionKey, RuleDomain},
};

#[test]
fn wangqi_level_transaction_preserves_captured_change_event_and_packet_order() {
    crate::test_support::init_config();
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60270, "UpdateWangQiMagicCircle"),
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(7),
                team_type: Some(1),
                ..Default::default()
            }],
            sub_entitys: vec![FightEntityInfo {
                uid: Some(crate::engine::manager::emitter::UID),
                team_type: Some(1),
                buffs: [31410601, 31412602, 31410603]
                    .into_iter()
                    .enumerate()
                    .map(|(index, buff_id)| BuffInfo {
                        buff_id: Some(buff_id),
                        uid: Some(index as i64 + 1),
                        from_uid: Some(7),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_field(FieldCommand {
            origin,
            team: 1,
            operation: FieldOperation::DeployIfAbsent {
                definition: FieldDefinition {
                    field_id: 42003,
                    duration: -1,
                },
                create_uid: 7,
                initial_level: 3,
                thresholds: Vec::new(),
            },
        })
        .unwrap();
    let mut events = EventBus::default();

    let outcome = execute_rule_op(
        &mut managers,
        &mut events,
        RuleOp::Command(BattleCommand::MagicCircleLevel(MagicCircleLevelCommand {
            origin,
            source_uid: 7,
            team: 1,
            expected_field_id: 42003,
            replacement: FieldDefinition {
                field_id: 42004,
                duration: 4,
            },
            level: 4,
            removed_buffs: vec![31410601, 31412602, 31410603],
            added_buffs: vec![31410601, 31412602, 31410603, 31410604],
        })),
    )
    .unwrap();
    let changes = outcome.changes();

    assert!(matches!(changes.as_slice(), [
        BattleChange::Buff(_),
        BattleChange::Buff(_),
        BattleChange::Buff(_),
        BattleChange::Field(removed),
        BattleChange::Buff(_),
        BattleChange::Buff(_),
        BattleChange::Buff(_),
        BattleChange::Buff(_),
        BattleChange::Field(added),
    ] if removed.kind == FieldChangeKind::Removed && added.kind == FieldChangeKind::Deployed));

    let event_kinds = std::iter::from_fn(|| events.pop())
        .map(|event| event.kind())
        .filter(|kind| {
            matches!(
                kind,
                EventKind::BuffRemoved | EventKind::BuffAdded | EventKind::FieldChanged
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        event_kinds,
        [
            vec![EventKind::BuffRemoved; 3],
            vec![EventKind::FieldChanged],
            vec![EventKind::BuffAdded; 4],
            vec![EventKind::FieldChanged],
        ]
        .concat()
    );

    let effects = changes
        .iter()
        .flat_map(|change| {
            crate::engine::packet::timeline::project_change_for_test(change).unwrap()
        })
        .filter(|effect| {
            matches!(
                effect.effect_type,
                Some(value)
                    if value == EffectType::Buffdel as i32
                        || value == EffectType::Magiccircledelete as i32
                        || value == EffectType::Buffadd as i32
                        || value == EffectType::Magiccircleadd as i32
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        effects
            .iter()
            .filter_map(|effect| effect.effect_type)
            .collect::<Vec<_>>(),
        [
            vec![EffectType::Buffdel as i32; 3],
            vec![EffectType::Magiccircledelete as i32],
            vec![EffectType::Buffadd as i32; 4],
            vec![EffectType::Magiccircleadd as i32],
        ]
        .concat()
    );
    let field = effects.last().unwrap();
    assert_eq!(field.target_id, Some(7));
    assert_eq!(field.magic_circle.as_ref().unwrap().create_uid, Some(-1));
}
