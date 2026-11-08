use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroSpAttribute};

use super::*;
use crate::engine::{
    manager::buff::{BuffCommand, BuffGrant},
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};

fn managers() -> BattleManagers {
    crate::test_support::init_config();
    let mut managers = BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: [-4, -1, -2, -3]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    current_hp: Some(100),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    });
    managers.attribute.override_sp(
        -4,
        &HeroSpAttribute {
            device_skill_rate: Some(550),
            ..Default::default()
        },
    );
    managers
}

fn grant(managers: &mut BattleManagers) -> crate::engine::manager::buff::BuffChanges {
    managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(1162, "EachChangeAttrOneWayAfterAdd"),
            },
            source_uid: -4,
            target_uid: -4,
            buff_id: 31560152,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap()
}

fn captured_state(info: &[BuffActInfo], value: i32) -> bool {
    let expected = format!("205#{value}");
    info.iter().any(|info| {
        info.act_id == Some(1162) && info.str_param.as_deref() == Some(expected.as_str())
    })
}

#[test]
fn snapshots_the_after_add_attribute_into_each_halo_copy() {
    let mut managers = managers();
    let changes = grant(&mut managers);
    let added = changes.change.added.as_ref().expect("field carrier");

    assert!(captured_state(&added.buff.act_info, 750));
    let copies = &changes.fanout[0].added;
    assert_eq!(copies.len(), 3);
    assert!(
        copies
            .iter()
            .all(|copy| captured_state(&copy.buff.act_info, 750))
    );
    for uid in [-4, -1, -2, -3] {
        assert_eq!(
            managers.buff.fixed_attribute_delta(uid, AttrId::DmgBonus),
            750
        );
    }
}

#[test]
fn replacement_keeps_the_captured_value() {
    let mut managers = managers();
    let initial = grant(&mut managers);
    let old_copy_uids = initial.fanout[0]
        .added
        .iter()
        .filter_map(|copy| copy.buff.uid)
        .collect::<Vec<_>>();
    managers.attribute.override_sp(
        -4,
        &HeroSpAttribute {
            device_skill_rate: Some(400),
            ..Default::default()
        },
    );
    let changes = grant(&mut managers);
    let added = changes.change.added.as_ref().expect("replacement carrier");

    assert!(captured_state(&added.buff.act_info, 600));
    let copies = &changes.fanout[0].added;
    assert_eq!(copies.len(), 3);
    assert!(
        copies
            .iter()
            .all(|copy| captured_state(&copy.buff.act_info, 600))
    );
    assert!(old_copy_uids.iter().all(|uid| {
        changes
            .change
            .removed
            .iter()
            .any(|removed| removed.buff.uid == Some(*uid))
    }));
    assert!(
        copies
            .iter()
            .filter_map(|copy| copy.buff.uid)
            .all(|uid| !old_copy_uids.contains(&uid))
    );
    for uid in [-4, -1, -2, -3] {
        assert_eq!(
            managers.buff.fixed_attribute_delta(uid, AttrId::DmgBonus),
            600
        );
    }
}

#[test]
fn supports_only_the_proven_conversion_shape() {
    assert!(supports(&[231, 205, 1_000, 750]));
    assert!(!supports(&[231, 205, 1_000]));
    assert!(!supports(&[205, 231, 1_000, 750]));
    assert!(!supports(&[231, 205, -1, 750]));
    assert!(!supports(&[231, 205, 1_000, -1]));
}
