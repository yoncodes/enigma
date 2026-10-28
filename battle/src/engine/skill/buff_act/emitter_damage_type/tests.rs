use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute, HeroExAttribute};

use super::*;
use crate::engine::{
    damage::AttackPlan,
    entity::attr::AttrId,
    manager::{BattleManagers, emitter, hp::HpCommand},
    skill::{
        rule::{CommandOrigin, DefinitionKey, RuleDomain},
        target::TargetPool,
    },
};

fn buff(uid: i64, buff_id: i32) -> BuffInfo {
    BuffInfo {
        uid: Some(uid),
        buff_id: Some(buff_id),
        from_uid: Some(emitter::UID),
        count: Some(1),
        layer: Some(1),
        ..Default::default()
    }
}

fn fight(converted: bool, source_buffs: &[i32], target_buffs: &[i32]) -> Fight {
    let mut source_buffs = source_buffs
        .iter()
        .enumerate()
        .map(|(index, buff_id)| buff(index as i64 + 10, *buff_id))
        .collect::<Vec<_>>();
    if converted {
        source_buffs.push(buff(20, 31410410));
    }
    Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(emitter::UID),
                model_id: Some(3087),
                entity_type: Some(1),
                team_type: Some(1),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                buffs: source_buffs,
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-2),
                model_id: Some(3141),
                entity_type: Some(1),
                team_type: Some(2),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    defense: Some(900),
                    mdefense: Some(0),
                    ..Default::default()
                }),
                buffs: target_buffs
                    .iter()
                    .enumerate()
                    .map(|(index, buff_id)| buff(index as i64 + 30, *buff_id))
                    .collect(),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn managers() -> BattleManagers {
    crate::test_support::init_config();
    BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(emitter::UID),
                team_type: Some(1),
                current_hp: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(10),
                    buff_id: Some(31410410),
                    from_uid: Some(-1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    })
}

#[test]
fn exact_static_owner_converts_only_emitter_damage_to_reality() {
    assert!(supports(&[1]));
    assert!(!supports(&[2]));
    assert!(!supports(&[]));
    assert_eq!(
        super::super::registry::destination(1092, "EmitterChangeDmgType", &[1]),
        Some(super::super::registry::BuffActDestination::StateConsumer)
    );

    let managers = managers();
    assert_eq!(
        resolve(
            EntityDamageType::Mental,
            emitter::UID,
            &managers.buff,
            &managers.hp,
        ),
        EntityDamageType::Reality
    );
    assert_eq!(
        resolve(EntityDamageType::Mental, -1, &managers.buff, &managers.hp,),
        EntityDamageType::Mental
    );
}

#[test]
fn converted_type_drives_defense_dodge_mitigation_consumption_and_critical_attributes() {
    crate::test_support::init_config();
    let base_fight = fight(false, &[30870111], &[3071, 3051]);
    let converted_fight = fight(true, &[30870111], &[3071, 3051]);
    let base_pool = TargetPool::from_fight(&base_fight);
    let converted_pool = TargetPool::from_fight(&converted_fight);
    let mut base = BattleManagers::seeded(&base_fight);
    let mut converted = BattleManagers::seeded(&converted_fight);
    for managers in [&mut base, &mut converted] {
        managers.attribute.override_ex(
            emitter::UID,
            &HeroExAttribute {
                cri_dmg: Some(1_500),
                ..Default::default()
            },
        );
    }
    let base_type = source_type(emitter::UID, &base_pool, &base);
    let converted_type = source_type(emitter::UID, &converted_pool, &converted);

    assert_eq!(base_type, EntityDamageType::Mental);
    assert_eq!(converted_type, EntityDamageType::Reality);
    assert!(super::super::dodge_spec_skill::avoidance(&base, -2, 1, base_type).is_none());
    assert!(super::super::dodge_spec_skill::avoidance(&converted, -2, 1, converted_type).is_some());
    assert_eq!(
        super::super::incoming_target_attack_attribute_delta(
            &converted,
            -2,
            converted_type,
            AttrId::DmgTakenReduction,
        ),
        300
    );
    assert!(super::super::be_attacked_consumption_rule_ops(&base, -2, &[base_type]).is_empty());
    assert_eq!(
        super::super::be_attacked_consumption_rule_ops(&converted, -2, &[converted_type]).len(),
        1
    );
    assert_eq!(
        crate::engine::damage::handler::crit_damage_multiplier(emitter::UID, -2, &base_pool, &base,)
            - crate::engine::damage::handler::crit_damage_multiplier(
                emitter::UID,
                -2,
                &converted_pool,
                &converted,
            ),
        200
    );

    let damage = |pool: &TargetPool, managers: &BattleManagers| {
        let HpCommand::Damage(damage) = crate::engine::damage::handler::resolve_attack_command(
            &AttackPlan {
                source_uid: emitter::UID,
                target_uid: -2,
                skill_id: 2240001,
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
            crate::engine::damage::handler::DamageRuntime {
                fight_version: 7,
                pool,
                attributes: &managers.attribute,
                buffs: &managers.buff,
                target_buffs: &managers.buff,
                hp: &managers.hp,
                fields: None,
                emitter: None,
                team_inspiration: 0,
            },
            CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(1092, "EmitterChangeDmgType"),
            },
        )
        .unwrap() else {
            panic!("expected damage command")
        };
        damage.amount
    };
    assert!(damage(&converted_pool, &converted) < damage(&base_pool, &base));
}
