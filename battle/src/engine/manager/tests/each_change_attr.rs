use sonettobuf::HeroExAttribute;

use super::*;
use crate::engine::entity::attr::AttrId;

fn origin() -> CommandOrigin {
    CommandOrigin {
        domain: RuleDomain::BuffAct,
        key: DefinitionKey::new(834, "EachChangeAttr"),
    }
}

fn buff(uid: i64, buff_id: i32, from_uid: i64) -> BuffInfo {
    BuffInfo {
        uid: Some(uid),
        buff_id: Some(buff_id),
        from_uid: Some(from_uid),
        ..Default::default()
    }
}

fn entity(uid: i64, attack: i32, defense: i32, buffs: Vec<BuffInfo>) -> FightEntityInfo {
    FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(1_000),
        attr: Some(HeroAttribute {
            hp: Some(1_000),
            attack: Some(attack),
            defense: Some(defense),
            mdefense: Some(defense),
            ..Default::default()
        }),
        buffs,
        ..Default::default()
    }
}

fn fight(source_buffs: Vec<BuffInfo>, target_buffs: Vec<BuffInfo>) -> Fight {
    Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(1, 1_862, 1_000, source_buffs),
                entity(2, 1_871, 500, target_buffs),
            ],
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn grant(managers: &mut BattleManagers, buff_id: i32) -> crate::engine::manager::buff::BuffChanges {
    managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin: origin(),
            source_uid: 1,
            target_uid: 2,
            buff_id,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap()
}

#[test]
fn snapshots_source_attack_without_wire_state() {
    crate::test_support::init_config();
    let mut managers = BattleManagers::seeded(&fight(vec![buff(3, 101, 1)], Vec::new()));

    let changes = grant(&mut managers, 31_410_407);

    assert!(changes.change.added.unwrap().buff.act_info.is_empty());
    assert_eq!(managers.origin_attribute(2, AttrId::Attack), 2_164);
    managers
        .execute_buff(BuffCommand::Remove(BuffRemove {
            origin: origin(),
            target_uid: 1,
            selector: BuffRemoveSelector::Uid(3),
        }))
        .unwrap();
    assert_eq!(managers.origin_attribute(2, AttrId::Attack), 2_164);
}

#[test]
fn restores_private_snapshot_state() {
    crate::test_support::init_config();
    let managers =
        BattleManagers::seeded(&fight(vec![buff(3, 101, 1)], vec![buff(4, 31_410_407, 1)]));

    assert_eq!(managers.origin_attribute(2, AttrId::Attack), 2_164);
}

#[test]
fn keeps_multiple_attribute_snapshots() {
    crate::test_support::init_config();
    let mut managers = BattleManagers::seeded(&fight(Vec::new(), Vec::new()));
    managers.attribute.override_ex(
        1,
        &HeroExAttribute {
            cri_dmg: Some(2_000),
            ..Default::default()
        },
    );
    managers.attribute.override_ex(
        2,
        &HeroExAttribute {
            cri_dmg: Some(1_000),
            ..Default::default()
        },
    );
    let critical_before = managers.origin_attribute(2, AttrId::CriticalDmg);
    let defense_before = managers.origin_attribute(2, AttrId::RealityDef);

    grant(&mut managers, 31_000_181);

    assert_eq!(
        managers.origin_attribute(2, AttrId::CriticalDmg),
        critical_before + 160
    );
    assert_eq!(
        managers.origin_attribute(2, AttrId::RealityDef),
        defense_before + 80
    );
}

#[test]
fn applies_reciprocal_critical_rate_snapshots() {
    crate::test_support::init_config();
    let mut managers = BattleManagers::seeded(&fight(Vec::new(), Vec::new()));
    managers.attribute.override_ex(
        1,
        &HeroExAttribute {
            cri: Some(500),
            ..Default::default()
        },
    );
    managers.attribute.override_ex(
        2,
        &HeroExAttribute {
            cri: Some(100),
            ..Default::default()
        },
    );

    grant(&mut managers, 31_000_191);

    assert_eq!(managers.origin_attribute(1, AttrId::CriticalRate), 508);
    assert_eq!(managers.origin_attribute(2, AttrId::CriticalRate), 140);
    assert_eq!(managers.origin_attribute(1, AttrId::MentalDef), 1_040);
    assert_eq!(managers.origin_attribute(2, AttrId::MentalDef), 580);
}

#[test]
fn snapshots_both_sides_of_the_link() {
    crate::test_support::init_config();
    let linked = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(1, 2_000, 0, Vec::new()),
                entity(2, 1_000, 0, Vec::new()),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&linked);
    let buff_uid = grant(&mut managers, 72_200_003)
        .change
        .added
        .unwrap()
        .buff
        .uid
        .unwrap();

    assert_eq!(managers.origin_attribute(1, AttrId::Attack), 2_100);
    assert_eq!(managers.origin_attribute(2, AttrId::Attack), 1_200);
    managers
        .execute_buff(BuffCommand::Remove(BuffRemove {
            origin: origin(),
            target_uid: 2,
            selector: BuffRemoveSelector::Uid(buff_uid),
        }))
        .unwrap();
    assert_eq!(managers.origin_attribute(1, AttrId::Attack), 2_000);
    assert_eq!(managers.origin_attribute(2, AttrId::Attack), 1_000);
}
