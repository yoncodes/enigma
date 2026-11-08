use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute, HeroExAttribute};

use super::*;
use crate::engine::skill::rule::{DefinitionKey, RuleDomain};

fn command() -> ShieldCommand {
    ShieldCommand {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(60259, "SupplyShield2"),
        },
        source_uid: 1,
        target_uid: 1,
        buff_id: 31170002,
        amount_attr: crate::engine::entity::attr::AttrId::Attack,
        amount_rate: 1_500,
        multiplier_bonus: Some((crate::engine::entity::attr::AttrId::CriticalRate, 900)),
        max_attr: crate::engine::entity::attr::AttrId::Attack,
        max_rate: 6_500,
        scope: ShieldScope::Entity,
        carrier_uid: ShieldCarrierUid::Definition,
    }
}

#[test]
fn shield_terms_are_rounded_after_they_are_combined() {
    assert_eq!(shield_amount(1_657, 1_500, Some((323, 400))), 2_699);
    assert_eq!(shield_amount(1_657, 2_250, Some((323, 600))), 4_049);
    assert_eq!(shield_amount(1_737, 1_500, Some((443, 400))), 2_913);
}

#[test]
fn origin_attribute_includes_snapshotted_stateful_buff_values() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                buffs: vec![sonettobuf::BuffInfo {
                    uid: Some(2),
                    buff_id: Some(31340007),
                    from_uid: Some(1),
                    act_info: vec![sonettobuf::BuffActInfo {
                        act_id: Some(1053),
                        param: vec![18],
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers.attribute.override_ex(
        1,
        &HeroExAttribute {
            cri: Some(100),
            ..Default::default()
        },
    );

    assert_eq!(
        managers.origin_attribute(1, crate::engine::entity::attr::AttrId::CriticalRate),
        118
    );
}

#[test]
fn shield_buff_is_attached_once_and_repeats_stack_to_the_cap() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers.attribute.override_ex(
        1,
        &HeroExAttribute {
            cri: Some(100),
            ..Default::default()
        },
    );

    let first = execute(&mut managers, command()).unwrap();
    let added = first
        .buff
        .as_ref()
        .and_then(|changes| changes.change.added.as_ref())
        .unwrap();
    assert_eq!(added.buff.uid, Some(2));
    assert_eq!(managers.hp.shield(1), 1_590);

    let second = execute(&mut managers, command()).unwrap();
    assert!(second.buff.as_ref().unwrap().change.refreshed.is_empty());
    assert_eq!(second.hp.unwrap().shield_granted.unwrap().added, 1_590);
    assert_eq!(managers.hp.shield(1), 3_180);
}

#[test]
fn cumulative_carrier_initializes_from_its_tier_then_refreshes_by_the_increment() {
    crate::test_support::init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-2),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let cumulative = ShieldCommand {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(60183, "SupplyShield2"),
        },
        source_uid: -2,
        target_uid: -2,
        buff_id: 116385674,
        amount_attr: crate::engine::entity::attr::AttrId::Hp,
        amount_rate: 200,
        multiplier_bonus: None,
        max_attr: crate::engine::entity::attr::AttrId::Hp,
        max_rate: 1_000,
        scope: ShieldScope::Entity,
        carrier_uid: ShieldCarrierUid::Child,
    };

    let first = execute(&mut managers, cumulative).unwrap();
    assert_eq!(first.hp.unwrap().shield_granted.unwrap().added, 400);
    assert_eq!(managers.hp.shield(-2), 400);

    let second = execute(&mut managers, cumulative).unwrap();
    assert_eq!(second.hp.unwrap().shield_granted.unwrap().added, 200);
    assert_eq!(managers.hp.shield(-2), 600);
}

#[test]
fn fifth_cumulative_carrier_initializes_to_full_max_hp() {
    crate::test_support::init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-2),
                current_hp: Some(191_000),
                attr: Some(HeroAttribute {
                    hp: Some(191_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);

    let changes = execute(
        &mut managers,
        ShieldCommand {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60183, "SupplyShield2"),
            },
            source_uid: -2,
            target_uid: -2,
            buff_id: 116385677,
            amount_attr: crate::engine::entity::attr::AttrId::Hp,
            amount_rate: 200,
            multiplier_bonus: None,
            max_attr: crate::engine::entity::attr::AttrId::Hp,
            max_rate: 1_000,
            scope: ShieldScope::Entity,
            carrier_uid: ShieldCarrierUid::Child,
        },
    )
    .unwrap();

    assert_eq!(changes.hp.unwrap().shield_granted.unwrap().added, 191_000);
    assert_eq!(managers.hp.shield(-2), 191_000);
}

#[test]
fn assist_boss_hp_can_drive_its_configured_shield() {
    crate::test_support::init_config();
    let mut fight = Fight {
        attacker: Some(FightTeam {
            assist_boss: Some(FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(999_999),
                attr: Some(HeroAttribute {
                    hp: Some(999_999),
                    attack: Some(7_001),
                    defense: Some(7_002),
                    mdefense: Some(7_003),
                    technic: Some(7_004),
                    ..Default::default()
                }),
                ex_point: Some(7),
                ex_point_type: Some(3),
                buffs: vec![sonettobuf::BuffInfo {
                    uid: Some(900),
                    buff_id: Some(999_999_999),
                    from_uid: Some(-1),
                    ..Default::default()
                }],
                team_type: Some(1),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    execute(
        &mut managers,
        ShieldCommand {
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(501, "Shield"),
            },
            source_uid: -1,
            target_uid: -1,
            buff_id: 4_700_101,
            amount_attr: crate::engine::entity::attr::AttrId::Hp,
            amount_rate: 600,
            multiplier_bonus: None,
            max_attr: crate::engine::entity::attr::AttrId::Hp,
            max_rate: 600,
            scope: ShieldScope::Entity,
            carrier_uid: ShieldCarrierUid::Definition,
        },
    )
    .unwrap();

    managers.sync_entities(&mut fight);

    assert_eq!(managers.hp.shield(-1), 599_999);
    let assist = fight.attacker.unwrap().assist_boss.unwrap();
    assert_eq!(assist.shield_value, Some(599_999));
    assert_eq!(assist.ex_point, Some(7));
    assert_eq!(assist.ex_point_type, Some(3));
    assert_eq!(
        assist.attr,
        Some(HeroAttribute {
            hp: Some(999_999),
            attack: Some(7_001),
            defense: Some(7_002),
            mdefense: Some(7_003),
            technic: Some(7_004),
            ..Default::default()
        })
    );
    assert!(assist.buffs.iter().any(|buff| buff.uid == Some(900)));
    assert_eq!(managers.buff.team_type(-1), Some(1));
    assert!(!managers.buff.alive_team_uids(1, &managers.hp).contains(&-1));
}

#[test]
fn shield_variants_share_the_configured_type_carrier() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);

    let first = execute(&mut managers, command()).unwrap();
    let carrier_uid = first
        .buff
        .as_ref()
        .and_then(|changes| changes.change.added.as_ref())
        .and_then(|added| added.buff.uid)
        .unwrap();

    let mut stronger = command();
    stronger.buff_id = 31170009;
    stronger.amount_rate = 2_700;
    stronger.multiplier_bonus = None;
    let second = execute(&mut managers, stronger).unwrap();

    assert!(second.buff.is_none());
    assert_eq!(
        managers.buff.buff_family_carrier_uid(1, stronger.buff_id),
        Some(carrier_uid)
    );
    assert_eq!(second.hp.unwrap().shield_granted.unwrap().added, 2_700);
    assert_eq!(managers.hp.shield(1), 4_200);
    assert_eq!(
        managers
            .buff
            .active_for(1)
            .filter_map(|buff| buff.buff_id)
            .collect::<Vec<_>>(),
        vec![31170002]
    );
}

#[test]
fn timed_shield_does_not_replace_or_expire_a_permanent_carrier() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let mut permanent = command();
    permanent.buff_id = 610161;

    let first = execute(&mut managers, permanent).unwrap();
    let carrier = first
        .buff
        .as_ref()
        .and_then(|changes| changes.change.added.as_ref())
        .unwrap();
    assert_eq!(carrier.buff.duration, Some(0));

    let timed = execute(&mut managers, command()).unwrap();
    assert!(timed.buff.is_none());
    let active = managers.buff.active_for(1).collect::<Vec<_>>();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].buff_id, Some(610161));
    assert_eq!(active[0].duration, Some(0));
}

#[test]
fn shared_family_shield_carrier_uses_its_configured_uid_lane() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin: command().origin,
            source_uid: 1,
            target_uid: 1,
            buff_id: 5_230_012,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();
    let mut shield = command();
    shield.buff_id = 222_001_232;
    shield.amount_attr = crate::engine::entity::attr::AttrId::Hp;
    shield.amount_rate = 300;
    shield.max_attr = crate::engine::entity::attr::AttrId::Hp;
    shield.max_rate = 300;
    shield.multiplier_bonus = None;

    let changes = execute(&mut managers, shield).unwrap();

    assert_eq!(
        changes.buff.unwrap().change.added.unwrap().buff.uid,
        Some(1004)
    );
}

#[test]
fn child_carrier_commands_allocate_consecutive_uids_after_observed_state() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: (1..=4)
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        attack: Some(1_000),
                        ..Default::default()
                    }),
                    buffs: (uid == 1)
                        .then(|| sonettobuf::BuffInfo {
                            buff_id: Some(434735),
                            uid: Some(1081),
                            from_uid: Some(1),
                            ..Default::default()
                        })
                        .into_iter()
                        .collect(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);

    let actual = (1..=4)
        .map(|target_uid| {
            let mut shield = command();
            shield.target_uid = target_uid;
            shield.carrier_uid = ShieldCarrierUid::Child;
            execute(&mut managers, shield)
                .unwrap()
                .buff
                .unwrap()
                .change
                .added
                .unwrap()
                .buff
                .uid
                .unwrap()
        })
        .collect::<Vec<_>>();

    assert_eq!(actual, vec![1082, 1083, 1084, 1085]);
}

#[test]
fn team_shared_shield_stacks_on_its_buff_act_state() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let command = ShieldCommand {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(60290, "SupplyTeamShareShield"),
        },
        source_uid: 1,
        target_uid: 1,
        buff_id: 31430144,
        amount_attr: crate::engine::entity::attr::AttrId::Attack,
        amount_rate: 2_800,
        multiplier_bonus: None,
        max_attr: crate::engine::entity::attr::AttrId::Attack,
        max_rate: 12_500,
        scope: ShieldScope::TeamShared,
        carrier_uid: ShieldCarrierUid::Definition,
    };

    let first = execute(&mut managers, command).unwrap();
    let added = first
        .buff
        .as_ref()
        .and_then(|changes| changes.change.added.as_ref())
        .unwrap();
    let buff_uid = added.buff.uid.unwrap();
    assert_eq!(
        added
            .buff
            .act_info
            .iter()
            .map(|info| (info.act_id.unwrap(), info.param.clone()))
            .collect::<Vec<_>>(),
        vec![(1125, vec![2_800]), (1126, vec![4])]
    );
    assert_eq!(added.pre_markers[0].act_id, 1126);
    assert_eq!(added.pre_markers[0].params, vec![4]);
    assert_eq!(
        added.markers[0],
        BuffMarkerResult {
            target_uid: 1,
            effect_type: sonettobuf::effect_type_enum::EffectType::None as i32,
            effect_num: 2_800,
            buff_act_id: 0,
        }
    );
    let second = execute(&mut managers, command).unwrap();
    assert!(second.buff.is_none());
    assert_eq!(second.team_shared.unwrap().after, 5_600);
    assert_eq!(
        managers
            .buff
            .snapshot(1, buff_uid)
            .unwrap()
            .act_info
            .iter()
            .find(|info| info.act_id == Some(1125))
            .unwrap()
            .param,
        vec![5_600]
    );

    managers
        .execute_buff(BuffCommand::Remove(
            crate::engine::manager::buff::BuffRemove {
                origin: command.origin,
                target_uid: 1,
                selector: crate::engine::manager::buff::BuffRemoveSelector::ExactId(
                    command.buff_id,
                ),
            },
        ))
        .unwrap();
    assert!(managers.buff.snapshot(1, buff_uid).is_none());
}

#[test]
fn team_shared_shield_stacks_on_an_existing_shared_type_variant() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    attack: Some(1_948),
                    ..Default::default()
                }),
                buffs: vec![sonettobuf::BuffInfo {
                    buff_id: Some(31430144),
                    uid: Some(1073),
                    from_uid: Some(1),
                    act_info: vec![sonettobuf::BuffActInfo {
                        act_id: Some(1125),
                        param: vec![5_454],
                        str_param: Some(String::new()),
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let command = ShieldCommand {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(60290, "SupplyTeamShareShield"),
        },
        source_uid: 1,
        target_uid: 1,
        buff_id: 31430121,
        amount_attr: crate::engine::entity::attr::AttrId::Attack,
        amount_rate: 2_800,
        multiplier_bonus: None,
        max_attr: crate::engine::entity::attr::AttrId::Attack,
        max_rate: 12_500,
        scope: ShieldScope::TeamShared,
        carrier_uid: ShieldCarrierUid::Definition,
    };

    let changes = execute(&mut managers, command).unwrap();

    assert!(changes.buff.is_none());
    assert_eq!(changes.team_shared.unwrap().after, 10_908);
    let carrier = managers.buff.snapshot(1, 1073).unwrap();
    assert_eq!(carrier.buff_id, Some(31430144));
    assert_eq!(
        carrier
            .act_info
            .iter()
            .find(|info| info.act_id == Some(1125))
            .unwrap()
            .param,
        vec![10_908]
    );
}

#[test]
fn removing_the_last_shield_buff_clears_its_shield_value() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    execute(&mut managers, command()).unwrap();

    managers
        .execute_buff(BuffCommand::Remove(
            crate::engine::manager::buff::BuffRemove {
                origin: command().origin,
                target_uid: 1,
                selector: crate::engine::manager::buff::BuffRemoveSelector::ExactId(
                    command().buff_id,
                ),
            },
        ))
        .unwrap();

    assert_eq!(managers.hp.shield(1), 0);
}

#[test]
fn failed_shield_planning_does_not_attach_a_carrier_or_advance_its_uid_lane() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let mut invalid = command();
    invalid.amount_rate = 0;

    assert_eq!(
        execute(&mut managers, invalid),
        Err(ShieldCommandError::Hp(HpCommandError::InvalidCommand))
    );
    assert!(!managers.buff.has_buff_id(1, invalid.buff_id));
    assert_eq!(managers.hp.shield(1), 0);

    let valid = execute(&mut managers, command()).unwrap();
    assert_eq!(valid.buff.unwrap().change.added.unwrap().buff.uid, Some(2));
}
