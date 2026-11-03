use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::skill::rule::{DefinitionKey, RuleDomain};

const ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::Behavior,
    key: DefinitionKey::new(60270, "UpdateWangQiMagicCircle"),
};

#[test]
fn replaces_linked_buffs_and_field_as_one_transaction() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(7),
                team_type: Some(1),
                ..Default::default()
            }],
            sub_entitys: vec![FightEntityInfo {
                uid: Some(emitter::UID),
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
            origin: ORIGIN,
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

    let changes = execute(
        &mut managers,
        MagicCircleLevelCommand {
            origin: ORIGIN,
            source_uid: 7,
            team: 1,
            expected_field_id: 42003,
            replacement: FieldDefinition {
                field_id: 42004,
                duration: 4,
            },
            level: 0,
            removed_buffs: vec![31410601, 31412602, 31410603],
            added_buffs: vec![31410601, 31412602, 31410603, 31410604],
        },
    )
    .unwrap();

    assert_eq!(changes.removed_buffs.len(), 3);
    assert_eq!(changes.added_buffs.len(), 4);
    assert_eq!(
        changes.removed_field.before.unwrap().definition.field_id,
        42003
    );
    let field = changes.added_field.after.unwrap();
    assert_eq!(field.definition.field_id, 42004);
    assert_eq!(field.owner_uid, 7);
    assert_eq!(field.create_uid, -1);
    assert_eq!(field.level, 0);
    for buff_id in [31410601, 31412602, 31410603, 31410604] {
        assert!(managers.buff.has_buff_id(emitter::UID, buff_id));
    }
}
