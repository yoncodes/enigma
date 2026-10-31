use sonettobuf::{BuffActInfo, BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;
use crate::engine::{
    manager::hp::{DamageEffectKind, HpCommand, HpDamage, HurtDamageFromType, HurtInfoData},
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};

fn fight(enhanced: bool) -> Fight {
    let entity = |uid, attack, buffs| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(5_000),
        shield_value: Some(3_000),
        attr: Some(HeroAttribute {
            hp: Some(5_000),
            attack: Some(attack),
            ..Default::default()
        }),
        buffs,
        ..Default::default()
    };
    let mut source_buffs = Vec::new();
    if enhanced {
        source_buffs.push(BuffInfo {
            uid: Some(12),
            buff_id: Some(31_400_007),
            from_uid: Some(10),
            duration: Some(1),
            ..Default::default()
        });
    }
    Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(10, 999, source_buffs),
                entity(
                    11,
                    2_121,
                    vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(31_400_002),
                        from_uid: Some(10),
                        act_info: vec![BuffActInfo {
                            act_id: Some(1146),
                            param: vec![2_121],
                            str_param: Some(String::new()),
                        }],
                        ..Default::default()
                    }],
                ),
            ],
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn damage(amount: i32) -> HpCommand {
    HpCommand::Damage(HpDamage {
        origin: CommandOrigin {
            domain: RuleDomain::Skill,
            key: DefinitionKey::new(1, "Damage"),
        },
        source_uid: -1,
        target_uid: 11,
        amount,
        config_effect: 1,
        effect_kind: DamageEffectKind::Normal,
        assassinate: false,
        ignore_riposte: false,
        hurt: HurtInfoData {
            from_uid: -1,
            is_crit: false,
            career_restraint: false,
            reduce_hp: 0,
            effect_id: 1,
            skill_id: 1,
            damage_from: HurtDamageFromType::Skill,
            buff_act_id: 0,
            buff_uid: 0,
            hurt_effect_type: 0,
            display_amount: None,
        },
    })
}

fn damage_kind(amount: i32, effect_kind: DamageEffectKind) -> HpCommand {
    let HpCommand::Damage(mut damage) = damage(amount) else {
        unreachable!();
    };
    damage.effect_kind = effect_kind;
    HpCommand::Damage(damage)
}

#[test]
fn mitigates_with_the_grant_time_attack_snapshot_before_ordinary_shields() {
    crate::test_support::init_config();
    let mut managers = BattleManagers::seeded(&fight(false));

    let changes = managers.execute_hp(damage(3_823)).unwrap();

    assert_eq!(changes.damage.unwrap().amount, 2_127);
    assert_eq!(changes.shield_absorbed.unwrap().absorbed, 2_127);
    assert!(changes.hp.is_none());
}

#[test]
fn delivery_process_selects_the_configured_enhanced_rate() {
    crate::test_support::init_config();
    let mut managers = BattleManagers::seeded(&fight(true));

    let changes = managers.execute_hp(damage(5_000)).unwrap();

    assert_eq!(changes.damage.unwrap().amount, 1_607);
    assert_eq!(changes.shield_absorbed.unwrap().absorbed, 1_607);
}

#[test]
fn critical_damage_settles_the_mitigated_remainder_through_shield_and_hp() {
    crate::test_support::init_config();
    let mut fight = fight(false);
    fight.attacker.as_mut().unwrap().entitys[1].shield_value = Some(1_000);
    let mut managers = BattleManagers::seeded(&fight);

    let changes = managers
        .execute_hp(damage_kind(3_823, DamageEffectKind::Critical))
        .unwrap();

    assert_eq!(changes.damage.unwrap().amount, 2_127);
    assert_eq!(changes.shield_absorbed.unwrap().absorbed, 1_000);
    assert_eq!(changes.hp.unwrap().delta, -1_127);
    assert_eq!(changes.hp.unwrap().hurt.unwrap().reduce_hp, -1_127);
    assert!(changes.hp.unwrap().hurt.unwrap().is_crit);
}

#[test]
fn genesis_and_accumulated_settlement_do_not_use_foundation() {
    crate::test_support::init_config();

    let mut genesis = BattleManagers::seeded(&fight(false));
    let changes = genesis
        .execute_hp(damage_kind(3_823, DamageEffectKind::Genesis))
        .unwrap();
    assert_eq!(changes.damage.unwrap().amount, 3_823);

    let mut accumulated = BattleManagers::seeded(&fight(false));
    let HpCommand::Damage(damage) = damage(3_823) else {
        unreachable!();
    };
    let changes = accumulated
        .execute_hp(HpCommand::SettleAccumulatedDamage(damage))
        .unwrap();
    assert_eq!(changes.damage.unwrap().amount, 3_823);
}

#[test]
fn unproven_protection_overlap_fails_loudly() {
    crate::test_support::init_config();
    let mut fight = fight(false);
    fight.attacker.as_mut().unwrap().entitys[1]
        .buffs
        .push(BuffInfo {
            uid: Some(21),
            buff_id: Some(6_240_530),
            from_uid: Some(11),
            count: Some(1),
            layer: Some(1),
            ..Default::default()
        });
    let mut managers = BattleManagers::seeded(&fight);

    assert_eq!(
        managers.execute_hp(damage(3_823)),
        Err(crate::engine::manager::hp::HpCommandError::UnsupportedFoundationInteraction)
    );
}
