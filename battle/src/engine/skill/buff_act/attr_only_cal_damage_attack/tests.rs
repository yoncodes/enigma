use super::*;
use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, effect_type_enum::EffectType};

use crate::engine::packet::effect::EffectPacket;

fn feature(values: Vec<i32>) -> ActiveBuffFeature {
    ActiveBuffFeature {
        owner_uid: 1,
        source_uid: 2,
        buff_uid: 3,
        buff_id: 301,
        amount: 1,
        team_type: 1,
        owner_alive: true,
        act_type: "AttrOnlyCalDamageAttack".to_owned(),
        effect_time: 0,
        effect_condition: 0,
        raw: String::new(),
        values,
    }
}

#[test]
fn type_owned_args_are_an_attack_local_consumable_attribute() {
    let morale = ActiveBuffFeature {
        amount: 3,
        ..feature(vec![113, AttrId::DmgBonus as i32, 500, 1])
    };
    assert_eq!(attribute_delta(&morale, AttrId::DmgBonus), 500);
    assert_eq!(attribute_delta(&morale, AttrId::CriticalDmg), 0);
    assert!(consumes_after_attack(&morale));

    let ultimate = ActiveBuffFeature {
        amount: 3,
        act_type: "AttrOnlyCalDamageAttackBigSkill".to_owned(),
        values: vec![1001, AttrId::DmgBonus as i32, 500, 1],
        ..feature(Vec::new())
    };
    assert_eq!(attribute_delta(&ultimate, AttrId::DmgBonus), 1_500);
}

#[test]
fn extra_action_attribute_waits_for_the_exact_action_kind() {
    let extra_action = ActiveBuffFeature {
        act_type: "AttrOnlyCalDamageInExtra".to_owned(),
        values: vec![740, AttrId::DmgBonus as i32, 200],
        ..feature(Vec::new())
    };
    let managers = BattleManagers::default();

    assert_eq!(
        super::super::attack_attribute_delta_for_skill(
            &extra_action,
            AttrId::DmgBonus,
            &managers.buff,
            &managers.hp,
            false,
            false,
        ),
        0
    );
    assert_eq!(
        super::super::attack_attribute_delta_for_skill(
            &extra_action,
            AttrId::DmgBonus,
            &managers.buff,
            &managers.hp,
            false,
            true,
        ),
        200
    );
    assert_eq!(
        super::super::calculated_attack_attribute_delta_for_skill(
            &extra_action,
            AttrId::DmgBonus,
            &managers.attribute,
            &managers.buff,
            &managers.hp,
            false,
            false,
            true,
            true,
        ),
        200
    );
    assert_eq!(
        super::super::calculated_attack_attribute_delta_for_skill(
            &extra_action,
            AttrId::DmgBonus,
            &managers.attribute,
            &managers.buff,
            &managers.hp,
            false,
            true,
            false,
            false,
        ),
        0
    );
    assert_eq!(
        extra_action_attribute(&extra_action),
        Some((AttrId::DmgBonus, 200))
    );
}

#[test]
fn extra_action_support_rejects_unknown_attributes() {
    assert!(supports_extra_action(&[AttrId::DmgBonus.id(), 200]));
    assert!(!supports_extra_action(&[999, 200]));
    assert!(!supports_extra_action(&[AttrId::DmgBonus.id(), 0]));
}

#[test]
fn consumed_attack_attribute_removes_its_exact_instance_at_zero() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(3),
                    buff_id: Some(301),
                    from_uid: Some(2),
                    count: Some(1),
                    layer: Some(0),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let morale = feature(vec![113, AttrId::DmgBonus as i32, 500, 1]);
    let changes = managers
        .execute_buff(consume_command(&managers, &morale).unwrap())
        .unwrap();

    assert!(managers.buff.snapshot(1, 3).is_none());
    assert_eq!(changes.change.removed[0].before_amount, 1);
    assert_eq!(changes.change.removed[0].buff.count, Some(0));
    let effects = EffectPacket::recorded_buff_changes(&changes);
    assert_eq!(effects[0].effect_type, Some(EffectType::Buffdel as i32));
}

#[test]
fn incantation_might_leaves_consumption_to_its_action_lifetime() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(3),
                    buff_id: Some(430111),
                    from_uid: Some(1),
                    count: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let morale = feature(vec![113, AttrId::DmgBonus as i32, 200, 1]);
    assert!(applies_to_skill(&morale, true));
    let incantation = ActiveBuffFeature {
        buff_id: 430111,
        values: vec![113, AttrId::IncantationMight as i32, 200, 1],
        ..morale
    };
    assert!(!applies_to_skill(&incantation, true));
    assert!(applies_to_skill(&incantation, false));
    assert!(managers.buff.settles_action_lifetime(1, 3));
    assert!(super::super::attack_consumption_rule_ops(&managers, 1, true).is_empty());
    assert!(super::super::attack_consumption_rule_ops(&managers, 1, false).is_empty());
    assert!(managers.buff.snapshot(1, 3).is_some());
}

#[test]
fn only_plain_attack_consumption_precedes_attacked_reactions() {
    let plain = feature(vec![113, AttrId::DmgBonus as i32, 500, 1]);
    assert_eq!(
        super::super::attack_consumption_timing(&plain),
        Some(super::super::registry::AttackConsumptionTiming::BeforeAttackedReactions)
    );
    let ultimate = ActiveBuffFeature {
        act_type: "AttrOnlyCalDamageAttackBigSkill".to_owned(),
        values: vec![1001, AttrId::UltimateMight as i32, 500, 1],
        ..plain
    };
    assert_eq!(super::super::attack_consumption_timing(&ultimate), None);
}

#[test]
fn be_attacked_attribute_is_applied_then_consumed_from_the_target() {
    crate::test_support::init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                buffs: vec![BuffInfo {
                    uid: Some(3),
                    buff_id: Some(302),
                    from_uid: Some(-1),
                    count: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);

    assert_eq!(
        super::super::incoming_target_attack_attribute_delta(
            &managers,
            -1,
            EntityDamageType::Mental,
            false,
            AttrId::DmgTakenReduction,
        ),
        250
    );
    let ops = super::super::be_attacked_consumption_rule_ops(
        &managers,
        -1,
        &[EntityDamageType::Mental],
        false,
    );
    let [(_, op)] = ops.as_slice() else {
        panic!("expected one target-local consumption");
    };
    let RuleOp::Command(BattleCommand::Buff(command)) = op else {
        panic!("expected a buff command");
    };
    managers.execute_buff(command.clone()).unwrap();

    assert!(managers.buff.snapshot(-1, 3).is_none());
}

#[test]
fn typed_be_attacked_attribute_matches_damage_type_before_consumption() {
    crate::test_support::init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(3),
                    buff_id: Some(3131),
                    from_uid: Some(-1),
                    count: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);

    assert_eq!(
        super::super::incoming_target_attack_attribute_delta(
            &managers,
            -1,
            EntityDamageType::Mental,
            false,
            AttrId::DmgTakenReduction,
        ),
        -250
    );
    assert_eq!(
        super::super::incoming_target_attack_attribute_delta(
            &managers,
            -1,
            EntityDamageType::Reality,
            false,
            AttrId::DmgTakenReduction,
        ),
        0
    );
    assert_eq!(
        super::super::be_attacked_consumption_rule_ops(
            &managers,
            -1,
            &[EntityDamageType::Mental],
            false,
        )
        .len(),
        1
    );
    assert!(
        super::super::be_attacked_consumption_rule_ops(
            &managers,
            -1,
            &[EntityDamageType::Reality],
            false,
        )
        .is_empty()
    );
    assert_eq!(
        super::super::be_attacked_consumption_rule_ops(
            &managers,
            -1,
            &[EntityDamageType::Reality, EntityDamageType::Mental],
            false,
        )
        .len(),
        1
    );
}

#[test]
fn ultimate_only_attribute_waits_for_an_ultimate_before_consumption() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(3),
                    buff_id: Some(228101),
                    from_uid: Some(1),
                    count: Some(1),
                    layer: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let feature = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find(|feature| feature.owner_uid == 1)
        .unwrap();
    assert_eq!(feature.values, vec![1001, 211, 30, 1, 1]);
    assert_eq!(
        super::super::feature_kind(&feature),
        Some(super::super::registry::BuffActKind::AttrOnlyCalDamageAttackBigSkill)
    );

    assert_eq!(
        super::super::attack_attribute_delta_for_skill(
            &feature,
            AttrId::UltimateMight,
            &managers.buff,
            &managers.hp,
            false,
            false,
        ),
        0
    );
    assert_eq!(
        super::super::attack_attribute_delta_for_skill(
            &feature,
            AttrId::UltimateMight,
            &managers.buff,
            &managers.hp,
            true,
            false,
        ),
        30
    );
    assert!(super::super::attack_consumption_rule_ops(&managers, 1, false).is_empty());
    assert_eq!(
        super::super::attack_consumption_rule_ops(&managers, 1, true).len(),
        1
    );
}
