use super::*;

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
            critical_multiplier_remainder: 0,
            is_conduit: false,
            is_crit: true,
            extra_skill_kind: 0,
        },
        runtime,
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
            critical_multiplier_remainder: 0,
            is_conduit: false,
            is_crit: false,
            extra_skill_kind: 1,
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
        critical_multiplier_remainder: 0,
        is_conduit: false,
        is_crit: false,
        assassinate: false,
        main_target: true,
        extra_skill_kind: 0,
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
