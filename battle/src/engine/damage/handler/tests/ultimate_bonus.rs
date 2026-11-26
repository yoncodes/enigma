use super::*;

#[test]
fn tactical_coordination_applies_to_ultimate_damage_before_target_reduction() {
    init_config();
    let mut source = entity(10, 1, 1, 1_000, 0);
    source.buffs.push(BuffInfo {
        uid: Some(1),
        buff_id: Some(312101442),
        from_uid: Some(10),
        act_common_params: Some("790#90.0".into()),
        ..Default::default()
    });
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
    let mut managers = BattleManagers::seeded(&fight);
    managers.attribute.override_ex(
        -1,
        &HeroExAttribute {
            drop_dmg: Some(500),
            ..Default::default()
        },
    );

    let runtime = DamageRuntime {
        fight_version: 7,
        pool: &pool,
        attributes: &managers.attribute,
        buffs: &managers.buff,
        target_buffs: &managers.buff,
        hp: &managers.hp,
        fields: None,
        emitter: None,
        team_inspiration: 0,
    };

    for (skill_id, expected) in [(312101313, 1_400), (312101143, 500)] {
        let plan = AttackPlan {
            source_uid: 10,
            target_uid: -1,
            skill_id,
            rate: 1_000,
            rate_terms: Vec::new(),
            attack_attributes: Vec::new(),
            attack_bonus: 0,
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
        };
        let result = resolve_attack_command(
            &plan,
            runtime,
            CommandOrigin {
                domain: crate::engine::skill::rule::RuleDomain::Skill,
                key: crate::engine::skill::rule::DefinitionKey::new(skill_id, "SkillDamage"),
            },
        );
        assert!(matches!(
            result,
            Some(HpCommand::Damage(HpDamage { amount, .. })) if amount == expected
        ));
    }

    let linked_hit = resolve_additional_damage_command(
        DamageRequest {
            source_uid: 10,
            action_owner_uid: 11,
            target_uid: -1,
            skill_id: 312101313,
            rate: 1_000,
            rate_terms: &[],
            attack_bonus: 0,
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
        runtime,
        DamageFormula::CreditedSourceAdditional,
        None,
        10,
        false,
        CommandOrigin {
            domain: crate::engine::skill::rule::RuleDomain::BuffAct,
            key: crate::engine::skill::rule::DefinitionKey::new(863, "CreateAdditionalDamage"),
        },
    );
    assert!(matches!(
        linked_hit,
        Some(HpCommand::Damage(HpDamage { amount: 500, .. }))
    ));
}
