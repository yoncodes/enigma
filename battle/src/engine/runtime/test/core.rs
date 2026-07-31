use super::*;

#[test]
fn entity_info_projects_manager_owned_state() {
    use crate::engine::{
        manager::hp::{CurrentHpSet, HpCommand},
        skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
    };

    crate::test_support::init_config();
    let mut runtime = BattleRuntime::new(Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3127),
                current_hp: Some(100),
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            sp_entitys: vec![FightEntityInfo {
                uid: Some(-6),
                model_id: Some(900030304),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    runtime
        .managers
        .execute_hp(HpCommand::SetCurrent(CurrentHpSet {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "Test"),
            },
            source_uid: 10,
            target_uid: 10,
            value: 70,
            config_effect: 0,
            effect_type: 1,
        }))
        .unwrap();

    let entity = runtime.entity_info(10).unwrap();
    assert_eq!(entity.model_id, Some(3127));
    assert_eq!(entity.current_hp, Some(70));
    assert_eq!(runtime.entity_info(-6).unwrap().model_id, Some(900030304));
    assert!(runtime.entity_info(11).is_none());
}

#[test]
fn refill_and_player_move_compositions_grant_cloth_power() {
    crate::test_support::init_config();
    let power = crate::engine::round::power::ClothPower::for_fight(&Fight {
        attacker: Some(FightTeam {
            cloth_id: Some(1),
            ..Default::default()
        }),
        ..Default::default()
    })
    .unwrap();

    assert_eq!(
        round::cloth_power_after_card_change(
            &power,
            15,
            crate::engine::manager::card::CardChangeKind::Refilled,
            false,
            1,
            false,
        ),
        17
    );
    assert_eq!(
        round::cloth_power_after_card_change(
            &power,
            15,
            crate::engine::manager::card::CardChangeKind::Composed,
            false,
            1,
            false,
        ),
        15
    );
    assert_eq!(
        round::cloth_power_after_card_change(
            &power,
            15,
            crate::engine::manager::card::CardChangeKind::Composed,
            false,
            1,
            true,
        ),
        17
    );
}

#[test]
fn cloth_composition_uses_the_owners_resource_type() {
    let mut managers = crate::engine::manager::BattleManagers::default();
    managers.ex_point.register(&FightEntityInfo {
        uid: Some(1),
        ex_point_type: Some(crate::engine::manager::ex_point::ExPointKind::Common.as_wire()),
        ..Default::default()
    });
    managers.ex_point.register(&FightEntityInfo {
        uid: Some(2),
        ex_point_type: Some(crate::engine::manager::ex_point::ExPointKind::Adrenaline.as_wire()),
        ..Default::default()
    });

    assert_eq!(round::eligible_composition_count(&managers, &[1, 2]), 1);
}

#[test]
fn end_fight_statistics_project_owned_runtime_history() {
    use crate::engine::{
        manager::{
            buff::{BuffCommand, BuffGrant},
            hp::{DamageEffectKind, HpCommand, HpDamage, HpHeal, HpHealKind, HurtInfoData},
        },
        skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
    };

    crate::test_support::init_config();
    let mut runtime = BattleRuntime::new(Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(50),
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "Test"),
    };
    runtime
        .managers
        .execute_hp(HpCommand::Damage(HpDamage {
            origin,
            source_uid: 10,
            target_uid: -1,
            amount: 30,
            config_effect: 1,
            effect_kind: DamageEffectKind::Normal,
            assassinate: false,
            hurt: HurtInfoData {
                from_uid: 10,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 1,
                skill_id: 101,
                damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: 0,
                display_amount: None,
            },
        }))
        .unwrap();
    runtime
        .managers
        .execute_hp(HpCommand::Heal(HpHeal {
            origin,
            source_uid: 10,
            target_uid: 10,
            amount: 20,
            config_effect: 1,
            kind: HpHealKind::Normal,
        }))
        .unwrap();
    runtime.managers.card.reset(
        vec![CardInfo {
            uid: Some(10),
            skill_id: Some(101),
            ..Default::default()
        }],
        0,
    );
    runtime.managers.card.play_card(0, Some(-1), None, None);
    runtime
        .managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: 10,
            target_uid: 10,
            buff_id: 70015,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();

    let stats = runtime.attack_statistics();
    assert_eq!(stats.len(), 1);
    assert_eq!(
        (stats[0].harm, stats[0].hurt, stats[0].heal),
        (Some(30), Some(0), Some(20))
    );
    assert_eq!(stats[0].cards[0].skill_id, Some(101));
    assert_eq!(stats[0].cards[0].use_count, Some(1));
    assert_eq!(stats[0].get_buffs, vec![70015]);
}
