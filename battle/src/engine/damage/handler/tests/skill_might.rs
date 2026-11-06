use super::*;

#[test]
fn forced_restraint_marks_the_resolved_hit() {
    init_config();
    let mut source = entity(10, 1, 1, 1_000, 0);
    source.buffs = vec![BuffInfo {
        uid: Some(1),
        buff_id: Some(11990041),
        from_uid: Some(10),
        count: Some(1),
        ..Default::default()
    }];
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![source],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, 1, 0, 0)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let command = resolve_attack_command(
        &AttackPlan {
            source_uid: 10,
            target_uid: -1,
            skill_id: 1,
            rate: 1_000,
            rate_terms: Vec::new(),
            attack_attributes: Vec::new(),
            career_ratio_bonus: 0,
            attack_career: None,
            additional_attack_career: None,
            force_career_restraint: false,
            critical_multiplier_remainder: 0,
            is_conduit: false,
            is_crit: false,
            assassinate: false,
            main_target: true,
            extra_skill_kind: 0,
            performs_extra_action: false,
            additional_enabled: false,
            additional_is_crit: None,
        },
        DamageRuntime {
            fight_version: 7,
            pool: &pool,
            attributes: &managers.attribute,
            buffs: &managers.buff,
            target_buffs: &managers.buff,
            hp: &managers.hp,
            fields: None,
            emitter: None,
            team_inspiration: 0,
        },
        CommandOrigin {
            domain: crate::engine::skill::rule::RuleDomain::Skill,
            key: crate::engine::skill::rule::DefinitionKey::new(1, "SkillDamage"),
        },
    )
    .unwrap();

    assert!(matches!(
        command,
        HpCommand::Damage(HpDamage {
            hurt: HurtInfoData {
                career_restraint: true,
                ..
            },
            ..
        })
    ));
}

#[test]
fn emitter_career_marks_the_resolved_hit() {
    init_config();
    let mut emitter = entity(crate::engine::manager::emitter::UID, 1, 0, 1_000, 0);
    emitter.buffs.push(BuffInfo {
        uid: Some(1),
        buff_id: Some(31410404),
        from_uid: Some(10),
        ..Default::default()
    });
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 1, 1_000, 0), emitter],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, 2, 0, 0)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let command = resolve_attack_command(
        &AttackPlan {
            source_uid: crate::engine::manager::emitter::UID,
            target_uid: -1,
            skill_id: 1,
            rate: 1_000,
            rate_terms: Vec::new(),
            attack_attributes: Vec::new(),
            career_ratio_bonus: 0,
            attack_career: None,
            additional_attack_career: None,
            force_career_restraint: false,
            critical_multiplier_remainder: 0,
            is_conduit: false,
            is_crit: false,
            assassinate: false,
            main_target: true,
            extra_skill_kind: 0,
            performs_extra_action: false,
            additional_enabled: false,
            additional_is_crit: None,
        },
        DamageRuntime {
            fight_version: 7,
            pool: &pool,
            attributes: &managers.attribute,
            buffs: &managers.buff,
            target_buffs: &managers.buff,
            hp: &managers.hp,
            fields: None,
            emitter: None,
            team_inspiration: 0,
        },
        CommandOrigin {
            domain: crate::engine::skill::rule::RuleDomain::Skill,
            key: crate::engine::skill::rule::DefinitionKey::new(1, "SkillDamage"),
        },
    )
    .unwrap();

    assert!(matches!(
        command,
        HpCommand::Damage(HpDamage {
            hurt: HurtInfoData {
                career_restraint: true,
                ..
            },
            ..
        })
    ));
}

#[test]
fn skill_might_uses_classification_and_cross_attributes() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 1, 1_000, 0)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, 1, 0, 0)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let cross = AttrId::from_raw(232).expect("configured ultimate incantation might multiplier");
    for (skill_id, incantation_might, conversion, expected) in [
        (30230134, 0, 0, 1_180),
        (30230134, 333, 500, 1_346),
        (31472131, 333, 1_000, 1_513),
        (31470112, 333, 0, 1_513),
        (31470112, 333, 1_000, 1_513),
    ] {
        for fight_version in [6, 7] {
            let command = resolve_attack_command(
                &AttackPlan {
                    source_uid: 10,
                    target_uid: -1,
                    skill_id,
                    rate: 1_000,
                    rate_terms: Vec::new(),
                    attack_attributes: vec![
                        (AttrId::UltimateMight, 180),
                        (AttrId::IncantationMight, incantation_might),
                        (AttrId::IncantationSkillUltMightMultiplier, 1_000),
                        (cross, conversion),
                    ],
                    career_ratio_bonus: 0,
                    attack_career: None,
                    additional_attack_career: None,
                    force_career_restraint: false,
                    critical_multiplier_remainder: 0,
                    is_conduit: false,
                    is_crit: false,
                    assassinate: false,
                    main_target: true,
                    extra_skill_kind: 0,
                    performs_extra_action: false,
                    additional_enabled: false,
                    additional_is_crit: None,
                },
                DamageRuntime {
                    fight_version,
                    pool: &pool,
                    attributes: &managers.attribute,
                    buffs: &managers.buff,
                    target_buffs: &managers.buff,
                    hp: &managers.hp,
                    fields: None,
                    emitter: None,
                    team_inspiration: 0,
                },
                CommandOrigin {
                    domain: crate::engine::skill::rule::RuleDomain::Skill,
                    key: crate::engine::skill::rule::DefinitionKey::new(skill_id, "SkillDamage"),
                },
            )
            .unwrap();

            assert!(matches!(
                command,
                HpCommand::Damage(HpDamage { amount, .. }) if amount == expected
            ));
        }
    }
}

#[test]
fn playmode_attack_attributes_use_the_final_damage_lane() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 1, 1_000, 100)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, 1, 0, 100)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let origin = CommandOrigin {
        domain: crate::engine::skill::rule::RuleDomain::Behavior,
        key: crate::engine::skill::rule::DefinitionKey::new(10004, "AttrFix"),
    };

    for (attack_attributes, expected) in [
        (vec![(AttrId::PlaymodeDmgIncrease, -150)], 765),
        (vec![(AttrId::PlaymodeDmgImmunity, 150)], 765),
        (
            vec![
                (AttrId::FinalDmgBonus, 330),
                (AttrId::PlaymodeDmgImmunity, 200),
            ],
            957,
        ),
    ] {
        let command = resolve_attack_command(
            &AttackPlan {
                source_uid: 10,
                target_uid: -1,
                skill_id: 1,
                rate: 1_000,
                rate_terms: Vec::new(),
                attack_attributes,
                career_ratio_bonus: 0,
                attack_career: None,
                additional_attack_career: None,
                force_career_restraint: false,
                critical_multiplier_remainder: 0,
                is_conduit: false,
                is_crit: false,
                assassinate: false,
                main_target: true,
                extra_skill_kind: 0,
                performs_extra_action: false,
                additional_enabled: false,
                additional_is_crit: None,
            },
            DamageRuntime {
                fight_version: 6,
                pool: &pool,
                attributes: &managers.attribute,
                buffs: &managers.buff,
                target_buffs: &managers.buff,
                hp: &managers.hp,
                fields: None,
                emitter: None,
                team_inspiration: 0,
            },
            origin,
        )
        .unwrap();

        assert!(matches!(
            command,
            HpCommand::Damage(HpDamage { amount, .. }) if amount == expected
        ));
    }
}

#[test]
fn conduit_might_only_scales_conduit_skills() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 1, 1_000, 0)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, 1, 0, 0)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let origin = CommandOrigin {
        domain: crate::engine::skill::rule::RuleDomain::Behavior,
        key: crate::engine::skill::rule::DefinitionKey::new(10004, "AttrFix"),
    };

    for (is_conduit, expected) in [(false, 1_000), (true, 1_300)] {
        let command = resolve_attack_command(
            &AttackPlan {
                source_uid: 10,
                target_uid: -1,
                skill_id: 1,
                rate: 1_000,
                rate_terms: Vec::new(),
                attack_attributes: vec![(AttrId::ConduitMight, 300)],
                career_ratio_bonus: 0,
                attack_career: None,
                additional_attack_career: None,
                force_career_restraint: false,
                critical_multiplier_remainder: 0,
                is_conduit,
                is_crit: false,
                assassinate: false,
                main_target: true,
                extra_skill_kind: 0,
                performs_extra_action: false,
                additional_enabled: false,
                additional_is_crit: None,
            },
            DamageRuntime {
                fight_version: 7,
                pool: &pool,
                attributes: &managers.attribute,
                buffs: &managers.buff,
                target_buffs: &managers.buff,
                hp: &managers.hp,
                fields: None,
                emitter: None,
                team_inspiration: 0,
            },
            origin,
        )
        .unwrap();

        assert!(matches!(
            command,
            HpCommand::Damage(HpDamage { amount, .. }) if amount == expected
        ));
    }
}
