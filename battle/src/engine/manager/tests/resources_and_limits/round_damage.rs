use super::*;

#[test]
fn round_start_skill_damage_is_available_during_player_actions() {
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(1_000),
        attr: Some(HeroAttribute {
            hp: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let hit = |amount| {
        HpCommand::Damage(HpDamage {
            origin: CommandOrigin {
                domain: RuleDomain::Skill,
                key: DefinitionKey::new(1, "SkillDamage"),
            },
            source_uid: 10,
            target_uid: -1,
            amount,
            config_effect: -1,
            effect_kind: DamageEffectKind::Normal,
            assassinate: false,
            ignore_riposte: false,
            hurt: HurtInfoData {
                from_uid: 10,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 1,
                skill_id: 1,
                damage_from: HurtDamageFromType::Skill,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: sonettobuf::effect_type_enum::EffectType::Damage as i32,
                display_amount: None,
            },
        })
    };

    managers.execute_hp(hit(50)).unwrap();
    managers.begin_reaction_round();
    assert_eq!(managers.hp.skill_damage_from_sources(-1, &[10]), 0);
    managers.execute_hp(hit(100)).unwrap();
    managers.begin_action_round();
    managers.execute_hp(hit(200)).unwrap();
    assert_eq!(managers.hp.skill_damage_from_sources(-1, &[10]), 300);
    managers.begin_reaction_round();
    assert_eq!(managers.hp.skill_damage_from_sources(-1, &[10]), 0);
}
