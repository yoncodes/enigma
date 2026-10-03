use super::*;
use crate::engine::damage::DamageFormula;

#[test]
fn performed_extra_actions_add_the_shared_and_specific_action_lanes() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                career: Some(1),
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
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                career: Some(1),
                current_hp: Some(10_000),
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
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.attribute.override_sp(
        1,
        &HeroSpAttribute {
            extra_dmg: Some(1_000),
            rebound_dmg: Some(500),
            reuse_dmg: Some(200),
            ..Default::default()
        },
    );
    let amount = |kind, performs_extra_action| {
        let command = resolve_attack_command(
            &AttackPlan {
                source_uid: 1,
                target_uid: -1,
                skill_id: 100,
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
                extra_skill_kind: kind,
                performs_extra_action,
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
            CommandOrigin {
                domain: crate::engine::skill::rule::RuleDomain::Skill,
                key: crate::engine::skill::rule::DefinitionKey::new(100, "SkillDamage"),
            },
        )
        .unwrap();
        let HpCommand::Damage(damage) = command else {
            panic!("expected damage");
        };
        damage.amount
    };

    assert_eq!(
        amount(
            crate::engine::skill::condition::extra::ExtraSkillKind::Riposte.id(),
            true,
        ),
        2_500
    );
    assert_eq!(
        amount(
            crate::engine::skill::condition::extra::ExtraSkillKind::FollowUp.id(),
            true,
        ),
        2_200
    );
}

#[test]
fn additional_damage_ignores_direct_hit_crit_defense_and_career_lanes() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                career: Some(2),
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
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                career: Some(1),
                current_hp: Some(10_000),
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
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.attribute.override_ex(
        1,
        &HeroExAttribute {
            cri_dmg: Some(1_500),
            ..Default::default()
        },
    );
    managers.attribute.override_ex(
        -1,
        &HeroExAttribute {
            cri_def: Some(-100),
            ..Default::default()
        },
    );
    let runtime = DamageRuntime {
        fight_version: 6,
        pool: &pool,
        attributes: &managers.attribute,
        buffs: &managers.buff,
        target_buffs: &managers.buff,
        hp: &managers.hp,
        fields: None,
        emitter: None,
        team_inspiration: 0,
    };
    let command = resolve_additional_damage_command(
        DamageRequest {
            source_uid: 1,
            target_uid: -1,
            skill_id: 100,
            rate: 1_000,
            rate_terms: &[],
            attack_attributes: &[],
            career_ratio_bonus: 0,
            attack_career: None,
            additional_attack_career: None,
            force_career_restraint: false,
            critical_multiplier_remainder: 0,
            is_conduit: false,
            is_crit: true,
            extra_skill_kind: 0,
            performs_extra_action: false,
        },
        runtime,
        DamageFormula::AdditionalDamage,
        None,
        1,
        true,
        CommandOrigin {
            domain: crate::engine::skill::rule::RuleDomain::BuffAct,
            key: crate::engine::skill::rule::DefinitionKey::new(863, "CreateAdditionalDamage"),
        },
    )
    .unwrap();

    let HpCommand::Damage(damage) = command else {
        panic!("expected additional damage");
    };
    assert_eq!(damage.amount, 1_600);
    assert!(damage.assassinate);
    assert_eq!(damage.hurt.effect_id, 0);
    assert_eq!(damage.hurt.skill_id, 0);
    managers.attribute.override_sp(
        1,
        &HeroSpAttribute {
            normal_skill_rate: Some(189),
            extra_dmg: Some(25),
            ..Default::default()
        },
    );
    let command = resolve_additional_damage_command(
        DamageRequest {
            source_uid: 1,
            target_uid: -1,
            skill_id: 100,
            rate: 1_000,
            rate_terms: &[],
            attack_attributes: &[(AttrId::ExtraDmg, 300)],
            career_ratio_bonus: 0,
            attack_career: None,
            additional_attack_career: None,
            force_career_restraint: false,
            critical_multiplier_remainder: 0,
            is_conduit: false,
            is_crit: false,
            extra_skill_kind: 1,
            performs_extra_action: false,
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
        DamageFormula::AdditionalDamage,
        Some(crate::engine::skill::buff_act::AttackReplacement {
            replaced_attr: AttrId::Attack,
            source_attr: AttrId::Hp,
            amount: 1_000,
            formula: crate::engine::damage::DamageFormula::AdditionalDamage,
        }),
        1,
        false,
        CommandOrigin {
            domain: crate::engine::skill::rule::RuleDomain::BuffAct,
            key: crate::engine::skill::rule::DefinitionKey::new(1005, "HpAdditionalDamage"),
        },
    )
    .unwrap();

    let HpCommand::Damage(damage) = command else {
        panic!("expected additional damage");
    };
    assert_eq!(damage.amount, 1_575);
}

#[test]
fn credited_source_additional_damage_uses_the_sources_career() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                career: Some(2),
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
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                career: Some(1),
                current_hp: Some(10_000),
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
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let command = resolve_additional_damage_command(
        DamageRequest {
            source_uid: 1,
            target_uid: -1,
            skill_id: 100,
            rate: 1_000,
            rate_terms: &[],
            attack_attributes: &[],
            career_ratio_bonus: 0,
            attack_career: None,
            additional_attack_career: None,
            force_career_restraint: false,
            critical_multiplier_remainder: 0,
            is_conduit: false,
            is_crit: false,
            extra_skill_kind: 0,
            performs_extra_action: false,
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
        DamageFormula::CreditedSourceAdditional,
        None,
        1,
        false,
        CommandOrigin {
            domain: crate::engine::skill::rule::RuleDomain::BuffAct,
            key: crate::engine::skill::rule::DefinitionKey::new(863, "CreateAdditionalDamage"),
        },
    )
    .unwrap();

    let HpCommand::Damage(damage) = command else {
        panic!("expected additional damage");
    };
    assert_eq!(damage.amount, 1_300);
    assert!(damage.hurt.career_restraint);
}

#[test]
fn proportional_additional_damage_uses_the_credited_sources_career() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(1, 1, 1, 1_000, 0), entity(2, 1, 2, 1_000, 0)],
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
    let runtime = DamageRuntime {
        fight_version: 6,
        pool: &pool,
        attributes: &managers.attribute,
        buffs: &managers.buff,
        target_buffs: &managers.buff,
        hp: &managers.hp,
        fields: None,
        emitter: None,
        team_inspiration: 0,
    };
    let origin = CommandOrigin {
        domain: crate::engine::skill::rule::RuleDomain::BuffAct,
        key: crate::engine::skill::rule::DefinitionKey::new(
            10003,
            "AssassinateCreateAdditionalDamage",
        ),
    };
    let main = HpDamage {
        origin,
        source_uid: 1,
        target_uid: -1,
        amount: 76_984,
        config_effect: -1,
        effect_kind: DamageEffectKind::Critical,
        assassinate: true,
        ignore_riposte: false,
        hurt: HurtInfoData {
            from_uid: 1,
            is_crit: true,
            career_restraint: false,
            reduce_hp: 0,
            effect_id: 100,
            skill_id: 100,
            damage_from: HurtDamageFromType::Skill,
            buff_act_id: 0,
            buff_uid: 0,
            hurt_effect_type: EffectType::Crit as i32,
            display_amount: None,
        },
    };

    let command = resolve_proportional_additional_damage_command(
        ProportionalAdditionalDamageRequest {
            main,
            rate: 1_250,
            main_rate: 6_000,
            credited_source_uid: 2,
            force_career_restraint: false,
            assassinate: false,
            origin,
        },
        runtime,
    )
    .unwrap();
    let HpCommand::Damage(damage) = command else {
        panic!("expected proportional additional damage");
    };

    assert_eq!(damage.amount, 16_038);
    assert_eq!(damage.source_uid, 2);
    assert!(damage.hurt.career_restraint);
}

#[test]
fn career_ratio_fix_extends_the_existing_advantage_lane() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                career: Some(2),
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
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                career: Some(1),
                current_hp: Some(10_000),
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
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let runtime = DamageRuntime {
        fight_version: 6,
        pool: &pool,
        attributes: &managers.attribute,
        buffs: &managers.buff,
        target_buffs: &managers.buff,
        hp: &managers.hp,
        fields: None,
        emitter: None,
        team_inspiration: 0,
    };
    let attack = |career_ratio_bonus| AttackPlan {
        source_uid: 1,
        target_uid: -1,
        skill_id: 100,
        rate: 1_000,
        rate_terms: Vec::new(),
        attack_attributes: Vec::new(),
        career_ratio_bonus,
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
    };
    let amount = |career_ratio_bonus| {
        let HpCommand::Damage(damage) = resolve_attack_command(
            &attack(career_ratio_bonus),
            runtime,
            CommandOrigin {
                domain: crate::engine::skill::rule::RuleDomain::Behavior,
                key: crate::engine::skill::rule::DefinitionKey::new(60058, "CareerRatioFix"),
            },
        )
        .expect("the attack should resolve") else {
            panic!("expected damage");
        };
        damage.amount
    };

    assert_eq!(amount(0), 1_300);
    assert_eq!(amount(300), 1_600);
}
