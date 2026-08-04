use super::*;

#[test]
fn fully_absorbed_damage_projects_shield_then_zero_damage() {
    let origin = CommandOrigin {
        domain: RuleDomain::Skill,
        key: DefinitionKey::new(123, "SkillDamage"),
    };
    let effects = project_change_for_test(&BattleChange::Hp(Box::new(HpChanges {
        origin,
        source_uid: 1,
        target_uid: 10,
        damage: Some(DamageRecord {
            amount: 40,
            config_effect: -1,
            effect_kind: DamageEffectKind::Normal,
            assassinate: false,
            ignore_riposte: false,
            hurt: HurtInfoData {
                from_uid: 1,
                is_crit: false,
                career_restraint: true,
                reduce_hp: 0,
                effect_id: 0,
                skill_id: 123,
                damage_from: HurtDamageFromType::Skill,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: EffectType::Damage as i32,
                display_amount: None,
            },
        }),
        team_shared_shield_absorbed: None,
        team_shared_shield_removed: None,
        shield_absorbed: Some(ShieldChange {
            target_uid: 10,
            buff_uid: 77,
            before: 100,
            absorbed: 40,
            after: 60,
        }),
        shield_granted: None,
        max_hp: None,
        hp: None,
        toughness: None,
        kill: None,
        death: None,
    })))
    .unwrap();

    assert_eq!(effects.len(), 2);
    assert_eq!(effects[0].effect_type, Some(EffectType::Shield as i32));
    assert_eq!(effects[0].effect_num, Some(60));
    assert_eq!(effects[1].effect_type, Some(EffectType::Damage as i32));
    assert_eq!(effects[1].effect_num, Some(0));
    assert_eq!(effects[1].hurt_info.as_ref().unwrap().damage, Some(0));
}

#[test]
fn version_seven_embeds_shield_absorption_in_hurt_info() {
    let origin = CommandOrigin {
        domain: RuleDomain::Skill,
        key: DefinitionKey::new(123, "SkillDamage"),
    };
    let effects = project_change(
        &BattleChange::Hp(Box::new(HpChanges {
            origin,
            source_uid: 1,
            target_uid: 10,
            damage: Some(DamageRecord {
                amount: 40,
                config_effect: -1,
                effect_kind: DamageEffectKind::Normal,
                assassinate: false,
                ignore_riposte: false,
                hurt: HurtInfoData {
                    from_uid: 1,
                    is_crit: false,
                    career_restraint: false,
                    reduce_hp: 0,
                    effect_id: 0,
                    skill_id: 123,
                    damage_from: HurtDamageFromType::Skill,
                    buff_act_id: 0,
                    buff_uid: 0,
                    hurt_effect_type: EffectType::Damage as i32,
                    display_amount: None,
                },
            }),
            team_shared_shield_absorbed: None,
            team_shared_shield_removed: None,
            shield_absorbed: Some(ShieldChange {
                target_uid: 10,
                buff_uid: 77,
                before: 100,
                absorbed: 40,
                after: 60,
            }),
            shield_granted: None,
            max_hp: None,
            hp: None,
            toughness: None,
            kill: None,
            death: None,
        })),
        true,
        HurtInfoWireLayout::Version7,
        RedealWireLayout::Version7,
    )
    .unwrap();

    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].effect_type, Some(EffectType::Damage as i32));
    assert_eq!(effects[0].effect_num, Some(0));
    assert_eq!(effects[0].hurt_info.as_ref().unwrap().damage, Some(40));
    assert_eq!(
        effects[0]
            .hurt_info
            .as_ref()
            .unwrap()
            .absorb_hurt_param
            .as_deref(),
        Some(
            r#"{"consumeFakeHpBuffMap":"","reduceTeamShareShieldBuffMap":"","reduceShieldBuffMap":"77#40"}"#
        )
    );
}

#[test]
fn version_seven_embeds_team_shared_shield_consumption_in_hurt_info() {
    let origin = CommandOrigin {
        domain: RuleDomain::Skill,
        key: DefinitionKey::new(123, "SkillDamage"),
    };
    let effects = project_change(
        &BattleChange::Hp(Box::new(HpChanges {
            origin,
            source_uid: 1,
            target_uid: 10,
            damage: Some(DamageRecord {
                amount: 418,
                config_effect: -1,
                effect_kind: DamageEffectKind::Normal,
                assassinate: false,
                ignore_riposte: false,
                hurt: HurtInfoData {
                    from_uid: 1,
                    is_crit: false,
                    career_restraint: false,
                    reduce_hp: 0,
                    effect_id: 0,
                    skill_id: 123,
                    damage_from: HurtDamageFromType::Skill,
                    buff_act_id: 0,
                    buff_uid: 0,
                    hurt_effect_type: EffectType::Damage as i32,
                    display_amount: None,
                },
            }),
            team_shared_shield_absorbed: Some(
                crate::engine::manager::hp::TeamSharedShieldAbsorption {
                    buff_uid: 77,
                    owner_uid: 11,
                    buff_act_id: 1125,
                    before: 1_000,
                    consumed: 349,
                    absorbed: 418,
                    after: 651,
                },
            ),
            team_shared_shield_removed: None,
            shield_absorbed: None,
            shield_granted: None,
            max_hp: None,
            hp: None,
            toughness: None,
            kill: None,
            death: None,
        })),
        true,
        HurtInfoWireLayout::Version7,
        RedealWireLayout::Version7,
    )
    .unwrap();

    assert_eq!(effects.len(), 1);
    assert_eq!(
        effects[0]
            .hurt_info
            .as_ref()
            .unwrap()
            .absorb_hurt_param
            .as_deref(),
        Some(
            r#"{"consumeFakeHpBuffMap":"","reduceTeamShareShieldBuffMap":"77#349","reduceShieldBuffMap":""}"#
        )
    );
}

#[test]
fn reduce_hp_wire_value_is_gated_by_fight_protocol_version() {
    let change = BattleChange::Hp(Box::new(HpChanges {
        origin: CommandOrigin {
            domain: RuleDomain::Skill,
            key: DefinitionKey::new(123, "SkillDamage"),
        },
        source_uid: 1,
        target_uid: 10,
        damage: None,
        team_shared_shield_absorbed: None,
        team_shared_shield_removed: None,
        shield_absorbed: None,
        shield_granted: None,
        max_hp: None,
        hp: Some(HpChange {
            target_uid: 10,
            before: 100,
            delta: -40,
            after: 60,
            max: 100,
            config_effect: -1,
            hurt: Some(HurtInfoData {
                from_uid: 1,
                is_crit: false,
                career_restraint: false,
                reduce_hp: -40,
                effect_id: 0,
                skill_id: 123,
                damage_from: HurtDamageFromType::Skill,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: EffectType::Damage as i32,
                display_amount: Some(40),
            }),
            assassinate: false,
            effect_type: 0,
            display_amount: Some(40),
        }),
        toughness: None,
        kill: None,
        death: None,
    }));

    let v6 =
        project_change_with_reduce_hp(&change, crate::engine::fight::versions::writes_reduce_hp(6))
            .unwrap();
    let v7 =
        project_change_with_reduce_hp(&change, crate::engine::fight::versions::writes_reduce_hp(7))
            .unwrap();

    assert_eq!(v6[0].hurt_info.as_ref().unwrap().reduce_hp, Some(0));
    assert_eq!(v7[0].hurt_info.as_ref().unwrap().reduce_hp, Some(-40));

    let v6 = project_change(
        &change,
        crate::engine::fight::versions::writes_reduce_hp(6),
        crate::engine::fight::versions::hurt_info_wire_layout(6).unwrap(),
        crate::engine::fight::versions::redeal_wire_layout(6).unwrap(),
    )
    .unwrap();
    let v7 = project_change(
        &change,
        crate::engine::fight::versions::writes_reduce_hp(7),
        crate::engine::fight::versions::hurt_info_wire_layout(7).unwrap(),
        crate::engine::fight::versions::redeal_wire_layout(7).unwrap(),
    )
    .unwrap();
    assert_eq!(v6[0].hurt_info.as_ref().unwrap().absorb_hurt_param, None);
    assert_eq!(v6[0].hurt_info.as_ref().unwrap().toughness_value, None);
    assert_eq!(v6[0].hurt_info.as_ref().unwrap().toughness_point, None);
    assert_eq!(v6[0].hurt_info.as_ref().unwrap().broken, None);
    assert_eq!(v6[0].hurt_info.as_ref().unwrap().hurt_merge_flag, None);
    assert_eq!(v6[0].hurt_info.as_ref().unwrap().critical, None);
    assert_eq!(v6[0].hurt_info.as_ref().unwrap().reduce_shield, Some(0));
    assert_eq!(v7[0].hurt_info.as_ref().unwrap().toughness_value, Some(0));
    assert_eq!(v7[0].hurt_info.as_ref().unwrap().toughness_point, Some(0));
    assert_eq!(v7[0].hurt_info.as_ref().unwrap().broken, Some(false));
    assert_eq!(v7[0].hurt_info.as_ref().unwrap().hurt_merge_flag, Some(0));
    assert_eq!(v7[0].hurt_info.as_ref().unwrap().critical, None);
    assert_eq!(v7[0].hurt_info.as_ref().unwrap().reduce_shield, None);
    assert_eq!(
        v7[0]
            .hurt_info
            .as_ref()
            .unwrap()
            .absorb_hurt_param
            .as_deref(),
        Some(
            r#"{"consumeFakeHpBuffMap":"","reduceTeamShareShieldBuffMap":"","reduceShieldBuffMap":""}"#
        )
    );
}

#[test]
fn shield_transaction_projects_carrier_refresh_and_stacked_value() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
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
            key: DefinitionKey::new(60183, "SupplyShield2"),
        },
        source_uid: 10,
        target_uid: 10,
        buff_id: 31170002,
        amount_attr: crate::engine::entity::attr::AttrId::Attack,
        amount_rate: 1_500,
        multiplier_bonus: None,
        max_attr: crate::engine::entity::attr::AttrId::Attack,
        max_rate: 6_500,
        scope: crate::engine::manager::shield::ShieldScope::Entity,
        carrier_uid: crate::engine::manager::shield::ShieldCarrierUid::Definition,
    };
    let changes = crate::engine::manager::shield::execute(&mut managers, command).unwrap();

    let effects = project_change_for_test(&BattleChange::Shield(Box::new(changes))).unwrap();

    assert_eq!(effects.len(), 4);
    assert_eq!(
        effects
            .iter()
            .map(|effect| effect.effect_type.unwrap())
            .collect::<Vec<_>>(),
        vec![
            EffectType::Buffadd as i32,
            EffectType::Shield as i32,
            EffectType::Attr as i32,
            EffectType::Attr as i32,
        ]
    );
    assert_eq!(effects[1].effect_num, Some(1_500));

    let changes = crate::engine::manager::shield::execute(&mut managers, command).unwrap();
    let effects = project_change_for_test(&BattleChange::Shield(Box::new(changes))).unwrap();
    assert_eq!(effects.len(), 2);
    assert_eq!(
        effects[0].effect_type,
        Some(EffectType::Changeshield as i32)
    );
    assert_eq!(effects[0].effect_num, Some(1_500));
    assert_eq!(effects[1].effect_type, Some(EffectType::Buffupdate as i32));
}

#[test]
fn team_shared_shield_projects_buff_state_then_stack_updates() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
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
        source_uid: 10,
        target_uid: 10,
        buff_id: 31430144,
        amount_attr: crate::engine::entity::attr::AttrId::Attack,
        amount_rate: 2_800,
        multiplier_bonus: None,
        max_attr: crate::engine::entity::attr::AttrId::Attack,
        max_rate: 12_500,
        scope: crate::engine::manager::shield::ShieldScope::TeamShared,
        carrier_uid: crate::engine::manager::shield::ShieldCarrierUid::Definition,
    };

    let changes = crate::engine::manager::shield::execute(&mut managers, command).unwrap();
    let effects = project_change_for_test(&BattleChange::Shield(Box::new(changes))).unwrap();
    assert_eq!(
        effects
            .iter()
            .map(|effect| effect.effect_type.unwrap())
            .collect::<Vec<_>>(),
        vec![
            EffectType::Buffactinfoupdate as i32,
            EffectType::Buffadd as i32,
            EffectType::None as i32,
            EffectType::None as i32,
        ]
    );
    assert_eq!(effects[2].effect_num, Some(2_800));
    assert_eq!(effects[3].effect_num, Some(0));

    let changes = crate::engine::manager::shield::execute(&mut managers, command).unwrap();
    let effects = project_change_for_test(&BattleChange::Shield(Box::new(changes))).unwrap();
    assert_eq!(effects.len(), 1);
    assert_eq!(
        effects[0].effect_type,
        Some(EffectType::Buffactinfoupdate as i32)
    );
    assert_eq!(
        effects[0].buff_act_info.as_ref().unwrap().param,
        vec![5_600]
    );
}

#[test]
fn depleted_team_shared_shield_projects_its_committed_removal() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                buffs: vec![sonettobuf::BuffInfo {
                    uid: Some(50),
                    buff_id: Some(31430144),
                    from_uid: Some(10),
                    act_info: vec![sonettobuf::BuffActInfo {
                        act_id: Some(1125),
                        param: vec![100],
                        str_param: Some(String::new()),
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
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
    let changes = managers
        .execute_hp(crate::engine::manager::hp::HpCommand::Damage(
            crate::engine::manager::hp::HpDamage {
                origin: CommandOrigin {
                    domain: RuleDomain::Skill,
                    key: DefinitionKey::new(1, "Damage"),
                },
                source_uid: -1,
                target_uid: 10,
                amount: 120,
                config_effect: -1,
                effect_kind: DamageEffectKind::Normal,
                assassinate: false,
                ignore_riposte: false,
                hurt: HurtInfoData {
                    from_uid: -1,
                    is_crit: false,
                    career_restraint: false,
                    reduce_hp: 0,
                    effect_id: 0,
                    skill_id: 1,
                    damage_from: HurtDamageFromType::Skill,
                    buff_act_id: 0,
                    buff_uid: 0,
                    hurt_effect_type: EffectType::Damage as i32,
                    display_amount: None,
                },
            },
        ))
        .unwrap();

    let effects = project_change_for_test(&BattleChange::Hp(Box::new(changes))).unwrap();
    assert_eq!(
        effects.last().and_then(|effect| effect.effect_type),
        Some(EffectType::Buffdel as i32)
    );
}
