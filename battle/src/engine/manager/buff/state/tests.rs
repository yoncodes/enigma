use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute};

use crate::engine::{
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrant, CommandOrigin},
    },
    skill::rule::{DefinitionKey, RuleDomain},
};

#[test]
fn poison_keeps_each_buff_uid_attack_snapshot_independent() {
    crate::test_support::init_config();
    let attacker = |uid, attack| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(1),
        current_hp: Some(100),
        attr: Some(HeroAttribute {
            attack: Some(attack),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![attacker(10, 1_000), attacker(20, 2_000)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(10_000),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let grant = |source_uid| {
        BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid,
            target_uid: -1,
            buff_id: 30560101,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };
    let first_uid = managers
        .execute_buff(grant(10))
        .unwrap()
        .change
        .added
        .unwrap()
        .buff
        .uid
        .unwrap();
    let second_uid = managers
        .execute_buff(grant(20))
        .unwrap()
        .change
        .added
        .unwrap()
        .buff
        .uid
        .unwrap();

    assert_eq!(
        managers.buff.dot_damage_for_owner(-1, 803, first_uid),
        Some(300)
    );
    assert_eq!(
        managers.buff.dot_damage_for_owner(-1, 803, second_uid),
        Some(600)
    );
}

#[test]
fn poison_snapshot_includes_active_flat_attack_attributes() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(1_000),
                    buffs: vec![sonettobuf::BuffInfo {
                        uid: Some(1),
                        buff_id: Some(30800141),
                        from_uid: Some(10),
                        act_common_params: Some("770#100#200".to_owned()),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    team_type: Some(1),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        attack: Some(1_000),
                        ..Default::default()
                    }),
                    buffs: vec![sonettobuf::BuffInfo {
                        uid: Some(2),
                        buff_id: Some(30800111),
                        from_uid: Some(10),
                        act_common_params: Some("767#200".to_owned()),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(10_000),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let added = managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid: 11,
            target_uid: -1,
            buff_id: 30560101,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap()
        .change
        .added
        .unwrap();

    assert_eq!(
        managers
            .buff
            .dot_damage_for_owner(-1, 803, added.buff.uid.expect("poison has a uid")),
        Some(360)
    );
}

#[test]
fn source_max_hp_dot_keeps_one_snapshot_when_layers_refresh() {
    crate::test_support::init_config();
    let source = |uid, hp| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(1),
        current_hp: Some(hp),
        attr: Some(HeroAttribute {
            hp: Some(hp),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![source(10, 11_400), source(20, 10_280)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(100_000),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let grant = |source_uid, layer| {
        BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid,
            target_uid: -1,
            buff_id: 105300115,
            amount: Some(layer),
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };
    let buff_uid = managers
        .execute_buff(grant(10, 18))
        .unwrap()
        .change
        .added
        .unwrap()
        .buff
        .uid
        .unwrap();
    managers.execute_buff(grant(20, 14)).unwrap();

    assert_eq!(
        managers.buff.dot_damage_for_owner(-1, 202, buff_uid),
        Some(570)
    );
}

#[test]
fn entity_scale_grant_starts_with_neutral_offset() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let added = managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid: 10,
            target_uid: 10,
            buff_id: 1033760020,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap()
        .change
        .added
        .unwrap();

    let buff_uid = added.buff.uid.unwrap();
    let stored = managers.buff.snapshot(10, buff_uid).unwrap();
    assert_eq!(
        stored.act_info,
        [sonettobuf::BuffActInfo {
            act_id: Some(1144),
            param: vec![0],
            str_param: Some(String::new()),
        }]
    );
    assert_eq!(
        added.pre_markers,
        [crate::engine::manager::buff::BuffActInfoMarkerResult {
            target_uid: 10,
            buff_uid,
            act_id: 1144,
            params: vec![0],
            str_param: Some(String::new()),
            team_type: 0,
        }]
    );
}
