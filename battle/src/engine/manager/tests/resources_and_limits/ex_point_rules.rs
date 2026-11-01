use super::*;

#[test]
fn ex_point_types_use_their_own_default_caps() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity_with_ex_point_type(1, 0),
                entity_with_ex_point_type(2, 1),
                entity_with_ex_point_type(3, 2),
                entity_with_ex_point_type(4, 3),
                entity_with_ex_point_type(5, 999),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };

    let mut managers = BattleManagers::seeded(&fight);

    assert_eq!(managers.ex_point.add(1, 1, 10, 0).after, 5);
    assert_eq!(managers.ex_point.add(2, 2, 10, 0).after, 10);
    assert_eq!(managers.ex_point.add(3, 3, 10, 0).after, 10);
    assert_eq!(managers.ex_point.add(4, 4, 10, 0).after, 10);
    assert_eq!(managers.ex_point.add(5, 5, 10, 0).after, 10);
}

#[test]
fn ex_point_cant_add_blocks_gains_but_allows_spending() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                ex_point: Some(3),
                current_hp: Some(100),
                attr: Some(HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let origin = CommandOrigin {
        domain: RuleDomain::BuffAct,
        key: DefinitionKey::new(603, "ExPointCantAdd"),
    };
    managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: 10,
            target_uid: 10,
            buff_id: 31050132,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();

    let change = |delta| {
        ExPointCommand::Change(ExPointChange {
            origin,
            source_uid: 10,
            target_uid: 10,
            delta,
            config_effect: 0,
            effect_type: 0,
        })
    };
    let blocked = managers.execute_ex_point(change(2)).unwrap();
    assert!(matches!(
        blocked,
        ExPointChanges::Value { change, .. }
            if change.requested_delta == 2
                && change.applied_delta == 0
                && change.before == 3
                && change.after == 3
    ));
    let spent = managers.execute_ex_point(change(-2)).unwrap();
    assert!(matches!(
        spent,
        ExPointChanges::Value { change, .. }
            if change.applied_delta == -2 && change.after == 1
    ));
}

#[test]
fn transfer_add_ex_point_redirects_only_gains_to_the_buff_source() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    ex_point: Some(0),
                    current_hp: Some(100),
                    attr: Some(HeroAttribute {
                        hp: Some(100),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    ex_point: Some(1),
                    current_hp: Some(100),
                    attr: Some(HeroAttribute {
                        hp: Some(100),
                        ..Default::default()
                    }),
                    buffs: vec![BuffInfo {
                        uid: Some(1),
                        buff_id: Some(31_000_171),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let origin = CommandOrigin {
        domain: RuleDomain::BuffAct,
        key: DefinitionKey::new(833, "TransferAddExPoint"),
    };

    let gained = managers
        .execute_ex_point(ExPointCommand::Change(ExPointChange {
            origin,
            source_uid: 20,
            target_uid: 20,
            delta: 2,
            config_effect: 0,
            effect_type: 0,
        }))
        .unwrap();
    assert!(matches!(
        gained,
        ExPointChanges::Value { change, .. }
            if change.source_uid == 20
                && change.target_uid == 10
                && change.applied_delta == 2
    ));
    assert_eq!(managers.ex_point.get(10), 2);
    assert_eq!(managers.ex_point.get(20), 1);

    managers
        .execute_ex_point(ExPointCommand::Spend(ExPointChange {
            origin,
            source_uid: 20,
            target_uid: 20,
            delta: -1,
            config_effect: 0,
            effect_type: 0,
        }))
        .unwrap();
    assert_eq!(managers.ex_point.get(10), 2);
    assert_eq!(managers.ex_point.get(20), 0);
}

#[test]
fn skill_damage_reduces_guard_at_the_active_action_rate() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                team_type: Some(1),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(10_000),
                toughness_value: Some(5_000),
                toughness_point: Some(3),
                is_broken: Some(false),
                attr: Some(HeroAttribute {
                    hp: Some(10_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let damage = || {
        HpCommand::Damage(HpDamage {
            origin: CommandOrigin {
                domain: RuleDomain::Skill,
                key: DefinitionKey::new(1, "Damage"),
            },
            source_uid: 10,
            target_uid: -1,
            amount: 1_000,
            config_effect: -1,
            effect_kind: DamageEffectKind::Normal,
            assassinate: false,
            ignore_riposte: false,
            hurt: HurtInfoData {
                from_uid: 10,
                is_crit: false,
                career_restraint: false,
                reduce_hp: -1_000,
                effect_id: 1,
                skill_id: 1,
                damage_from: HurtDamageFromType::Skill,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: 0,
                display_amount: Some(1_000),
            },
        })
    };

    let normal = managers.execute_hp(damage()).unwrap();
    assert_eq!(normal.toughness.unwrap().value_delta, 200);

    managers
        .conduit
        .execute(ConduitCommand::SetRunning {
            source_uid: 10,
            running: true,
        })
        .unwrap();
    let conduit = managers.execute_hp(damage()).unwrap();
    assert_eq!(conduit.toughness.unwrap().value_delta, 1_000);
}

#[test]
fn moxie_reduction_immunity_blocks_reduction_but_allows_spending() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                ex_point: Some(3),
                current_hp: Some(100),
                attr: Some(HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let guard_origin = CommandOrigin {
        domain: RuleDomain::BuffAct,
        key: DefinitionKey::new(509, "ImmunityExpointChange"),
    };
    managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin: guard_origin,
            source_uid: 10,
            target_uid: 10,
            buff_id: 5081,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();

    let reduction = |delta| {
        ExPointCommand::Change(ExPointChange {
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(605, "ExPointDel"),
            },
            source_uid: 10,
            target_uid: 10,
            delta,
            config_effect: 0,
            effect_type: 0,
        })
    };
    let blocked = managers.execute_ex_point(reduction(-2)).unwrap();
    assert!(matches!(
        blocked,
        ExPointChanges::Value { change, .. }
            if change.requested_delta == -2
                && change.applied_delta == 0
                && change.before == 3
                && change.after == 3
    ));
    let spent = managers
        .execute_ex_point(ExPointCommand::Spend(ExPointChange {
            origin: crate::engine::manager::card::CARD_PLAY_ORIGIN,
            source_uid: 10,
            target_uid: 10,
            delta: -2,
            config_effect: 0,
            effect_type: 0,
        }))
        .unwrap();
    assert!(matches!(
        spent,
        ExPointChanges::Value { change, .. }
            if change.applied_delta == -2 && change.after == 1
    ));
}

#[test]
fn moxie_transfer_respects_reduction_immunity() {
    crate::test_support::init_config();
    let entity = |uid, ex_point| FightEntityInfo {
        uid: Some(uid),
        ex_point: Some(ex_point),
        current_hp: Some(100),
        attr: Some(HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 0)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let immunity_uid = managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(509, "ImmunityExpointChange"),
            },
            source_uid: -1,
            target_uid: -1,
            buff_id: 5081,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap()
        .change
        .added
        .unwrap()
        .buff
        .uid
        .unwrap();

    let transfer = ExPointCommand::Transfer(ExPointTransfer {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(30011, "AbsorbExPoint"),
        },
        source_uid: 10,
        from_uid: -1,
        to_uid: 10,
        amount: 1,
        config_effect: 30011,
        effect_type: 0,
    });
    let changes = managers.execute_ex_point(transfer).unwrap();

    assert!(matches!(
        changes,
        ExPointChanges::Transfer { loss, gain, .. }
            if loss.applied_delta == 0 && gain.applied_delta == 0
    ));
    assert_eq!(managers.ex_point.get(-1), 2);
    assert_eq!(managers.ex_point.get(10), 0);

    managers
        .execute_buff(BuffCommand::Remove(BuffRemove {
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(509, "ImmunityExpointChange"),
            },
            target_uid: -1,
            selector: BuffRemoveSelector::Uid(immunity_uid),
        }))
        .unwrap();
    let changes = managers.execute_ex_point(transfer).unwrap();
    assert!(matches!(
        changes,
        ExPointChanges::Transfer { loss, gain, .. }
            if loss.applied_delta == -1 && gain.applied_delta == 1
    ));
    assert_eq!(managers.ex_point.get(-1), 1);
    assert_eq!(managers.ex_point.get(10), 1);

    managers.ex_point.add(10, 10, 4, 0);
    let capped = managers.execute_ex_point(transfer).unwrap();
    assert!(matches!(
        capped,
        ExPointChanges::Transfer { loss, gain, .. }
            if loss.applied_delta == -1 && gain.applied_delta == 0 && gain.overflow == 1
    ));
    assert!(matches!(
        capped.events().as_slice(),
        [
            crate::engine::event::payload::BattleEvent::ExPointChanged(loss),
            crate::engine::event::payload::BattleEvent::ExPointOverflow(gain),
        ] if loss.target_uid == -1 && gain.target_uid == 10 && gain.overflow == 1
    ));
}

#[test]
fn moxie_reduction_immunity_precedes_resistance() {
    crate::test_support::init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(103370111),
                ex_point: Some(3),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let granted = managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(509, "ImmunityExpointChange"),
            },
            source_uid: -1,
            target_uid: -1,
            buff_id: 5081,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();
    let buff_uid = granted.change.added.unwrap().buff.uid.unwrap();
    let reduction = || {
        ExPointCommand::Reduce(ExPointChange {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(30001, "DelExPoint"),
            },
            source_uid: 10,
            target_uid: -1,
            delta: -1,
            config_effect: 1,
            effect_type: 0,
        })
    };
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    determinism.replace_moxie_reduction_resistance_choices([
        crate::engine::runtime::determinism::MoxieReductionResistanceChoice {
            producer_id: 30001,
            source_uid: 10,
            target_uid: -1,
            resisted: true,
        },
    ]);

    let blocked = managers
        .execute_ex_point_rolled(reduction(), &mut determinism)
        .unwrap();
    assert!(matches!(
        blocked,
        ExPointChanges::Value { change, .. }
            if change.applied_delta == 0 && change.before == 3 && change.after == 3
    ));

    managers
        .execute_buff(BuffCommand::Remove(BuffRemove {
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(509, "ImmunityExpointChange"),
            },
            target_uid: -1,
            selector: BuffRemoveSelector::Uid(buff_uid),
        }))
        .unwrap();
    assert!(matches!(
        managers
            .execute_ex_point_rolled(reduction(), &mut determinism)
            .unwrap(),
        ExPointChanges::Resisted { target_uid: -1, .. }
    ));
}

fn entity_with_ex_point_type(uid: i64, ex_point_type: i32) -> FightEntityInfo {
    FightEntityInfo {
        uid: Some(uid),
        ex_point: Some(0),
        current_hp: Some(100),
        attr: Some(HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        ex_point_type: Some(ex_point_type),
        ..Default::default()
    }
}
