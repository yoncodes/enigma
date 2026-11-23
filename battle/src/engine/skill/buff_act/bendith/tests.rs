use super::*;
use crate::engine::event::payload::BuffChangeEvent;
use sonettobuf::{Fight, FightEntityInfo, FightTeam};

#[test]
fn parses_the_structured_group_mapping() {
    assert_eq!(
        parse_skill_group_mapping("1138#1:31460211,31460212,31460213#2:31460221,31460222,31460223"),
        Some(SkillGroupMapping {
            group1: vec![31460211, 31460212, 31460213],
            group2: vec![31460221, 31460222, 31460223],
        })
    );
}

#[test]
fn rejects_flattened_or_unsupported_mappings() {
    crate::test_support::init_config();
    for raw in [
        "1138#1#31460211,31460212,31460213#2:31460221,31460222,31460223",
        "1138#1:31460211,31460212,31460213",
        "1138#1:31460211,31460212#2:31460221,31460222,31460223",
        "1138#1:31460211,0,31460213#2:31460221,31460222,31460223",
        "1138#1:31460211,31460212,31460213#3:31460221,31460222,31460223",
    ] {
        assert!(
            !supports_replace_entity_skill_group(config::try_get(), raw),
            "{raw}"
        );
    }
}

#[test]
fn structurally_valid_mapping_without_unique_inverse_is_unsupported() {
    crate::test_support::init_config();
    assert!(!supports_replace_entity_skill_group(
        config::try_get(),
        "1138#1:30120111,30120112,30120113#2:30120121,30120122,30120123"
    ));
}

#[test]
fn retained_mapping_adds_and_removes_as_exact_inverse_commands() {
    crate::test_support::init_config();
    let managers = BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3146),
                team_type: Some(1),
                current_hp: Some(100),
                skill_group1: vec![31460114, 31460115, 31460116],
                skill_group2: vec![31460127, 31460128, 31460129],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let change = BuffChangeEvent {
        source_uid: 10,
        target_uid: 10,
        buff_uid: 20,
        buff_id: 31460137,
        before_amount: 0,
        after_amount: 1,
        act_id: 0,
        act_value: 0,
        replaced_fixed_max_hp: None,
        replaced_buff_id: None,
        expiry: crate::engine::event::payload::BuffExpiry::None,
    };
    let command = |event| {
        let operations = replace_entity_skill_group_transaction(&managers, &event);
        let [(_, RuleOp::Command(BattleCommand::Card(CardCommand::ReplaceOwnerSkills(command))))] =
            operations.as_slice()
        else {
            panic!("expected one exact replacement command")
        };
        command.clone()
    };

    let added = command(BattleEvent::BuffAdded(change));
    assert_eq!(added.owner_uid, 10);
    assert_eq!(added.base_group1, vec![31460114, 31460115, 31460116]);
    assert_eq!(added.replacement_group1, vec![31460214, 31460215, 31460216]);
    assert_eq!(added.base_group2, vec![31460127, 31460128, 31460129]);
    assert_eq!(added.replacement_group2, vec![31460227, 31460228, 31460229]);

    let removed = command(BattleEvent::BuffRemoved(BuffChangeEvent {
        before_amount: 1,
        after_amount: 0,
        ..change
    }));
    assert_eq!(removed.base_group1, added.replacement_group1);
    assert_eq!(removed.base_group2, added.replacement_group2);
    assert_eq!(removed.replacement_group1, added.base_group1);
    assert_eq!(removed.replacement_group2, added.base_group2);
}
