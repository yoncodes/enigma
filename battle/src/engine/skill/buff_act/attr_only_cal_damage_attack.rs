use crate::engine::{
    entity::attr::AttrId,
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffCommand, BuffConsume, BuffSelector, DepletedBuff},
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        target::EntityDamageType,
    },
};

pub fn supports_extra_action(args: &[i32]) -> bool {
    matches!(args, [raw_attr, value]
        if AttrId::from_raw(*raw_attr).is_some() && *value != 0)
}

pub fn supports_be_attacked_type(args: &[i32]) -> bool {
    matches!(
        args,
        [raw_damage_type, raw_attr, value, 1]
            if matches!(EntityDamageType::from_wire(*raw_damage_type), EntityDamageType::Reality | EntityDamageType::Mental)
                && AttrId::from_raw(*raw_attr) == Some(AttrId::DmgTakenReduction)
                && *value != 0
    )
}

pub fn supports_be_attacked(args: &[i32]) -> bool {
    matches!(
        args,
        [raw_attr, value, 1]
            if AttrId::from_raw(*raw_attr) == Some(AttrId::DmgTakenReduction) && *value != 0
    )
}

pub fn attribute_delta(feature: &ActiveBuffFeature, attr_id: AttrId) -> i32 {
    match feature.values.as_slice() {
        [_, raw_attr_id, value, ..] if *raw_attr_id == attr_id as i32 => value * feature.amount,
        _ => 0,
    }
}

pub fn consumes_after_attack(feature: &ActiveBuffFeature) -> bool {
    match super::feature_kind(feature) {
        Some(super::registry::BuffActKind::AttrOnlyCalDamageBeAttackedType) => {
            matches!(feature.values.as_slice(), [_, _, _, _, consume] if *consume != 0)
        }
        _ => matches!(feature.values.as_slice(), [_, _, _, consume, ..] if *consume != 0),
    }
}

pub fn applies_to_incoming_damage(
    feature: &ActiveBuffFeature,
    damage_type: EntityDamageType,
) -> bool {
    match super::feature_kind(feature) {
        Some(super::registry::BuffActKind::AttrOnlyCalDamageBeAttacked) => true,
        Some(super::registry::BuffActKind::AttrOnlyCalDamageBeAttackedType) => {
            matches!(feature.values.as_slice(), [_, raw_damage_type, ..]
                if EntityDamageType::from_wire(*raw_damage_type) == damage_type)
        }
        _ => false,
    }
}

pub fn applies_to_any_incoming_damage(
    feature: &ActiveBuffFeature,
    damage_types: &[EntityDamageType],
) -> bool {
    match super::feature_kind(feature) {
        Some(super::registry::BuffActKind::AttrOnlyCalDamageBeAttacked) => true,
        Some(super::registry::BuffActKind::AttrOnlyCalDamageBeAttackedType) => damage_types
            .iter()
            .any(|damage_type| applies_to_incoming_damage(feature, *damage_type)),
        _ => false,
    }
}

pub fn incoming_attribute_delta(
    feature: &ActiveBuffFeature,
    damage_type: EntityDamageType,
    attr_id: AttrId,
) -> i32 {
    if !applies_to_incoming_damage(feature, damage_type) {
        return 0;
    }
    match super::feature_kind(feature) {
        Some(super::registry::BuffActKind::AttrOnlyCalDamageBeAttackedType) => {
            match feature.values.as_slice() {
                [_, _, raw_attr, value, _] if AttrId::from_raw(*raw_attr) == Some(attr_id) => {
                    value.saturating_mul(feature.amount)
                }
                _ => 0,
            }
        }
        _ => attribute_delta(feature, attr_id),
    }
}

pub fn applies_to_skill(feature: &ActiveBuffFeature, is_big_skill: bool) -> bool {
    match feature.values.as_slice() {
        [_, raw_attr_id, ..] if *raw_attr_id == AttrId::UltimateMight as i32 => is_big_skill,
        [_, raw_attr_id, ..] if *raw_attr_id == AttrId::IncantationMight as i32 => !is_big_skill,
        _ => true,
    }
}

pub fn consume_rule_op(managers: &BattleManagers, feature: &ActiveBuffFeature) -> Option<RuleOp> {
    let command = consume_command(managers, feature)?;
    Some(RuleOp::Command(BattleCommand::Buff(command)))
}

fn consume_command(managers: &BattleManagers, feature: &ActiveBuffFeature) -> Option<BuffCommand> {
    if !consumes_after_attack(feature) {
        return None;
    }
    managers
        .buff
        .snapshot(feature.owner_uid, feature.buff_uid)?;
    Some(BuffCommand::Consume(BuffConsume {
        origin: super::feature_command_origin(feature)?,
        target_uid: feature.owner_uid,
        selector: BuffSelector::Uid(feature.buff_uid),
        amount: 1,
        depleted: DepletedBuff::Remove,
    }))
}

#[cfg(test)]
mod tests {
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
        let morale = feature(vec![113, AttrId::DmgBonus as i32, 500, 1]);
        assert_eq!(attribute_delta(&morale, AttrId::DmgBonus), 500);
        assert_eq!(attribute_delta(&morale, AttrId::CriticalDmg), 0);
        assert!(consumes_after_attack(&morale));
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
    fn incantation_might_waits_for_a_basic_attack_before_consumption() {
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
        assert!(super::super::attack_consumption_rule_ops(&managers, 1, true).is_empty());
        assert_eq!(
            super::super::attack_consumption_rule_ops(&managers, 1, false).len(),
            1
        );
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
                AttrId::DmgTakenReduction,
            ),
            250
        );
        let ops = super::super::be_attacked_consumption_rule_ops(
            &managers,
            -1,
            &[EntityDamageType::Mental],
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
                AttrId::DmgTakenReduction,
            ),
            -250
        );
        assert_eq!(
            super::super::incoming_target_attack_attribute_delta(
                &managers,
                -1,
                EntityDamageType::Reality,
                AttrId::DmgTakenReduction,
            ),
            0
        );
        assert_eq!(
            super::super::be_attacked_consumption_rule_ops(
                &managers,
                -1,
                &[EntityDamageType::Mental],
            )
            .len(),
            1
        );
        assert!(
            super::super::be_attacked_consumption_rule_ops(
                &managers,
                -1,
                &[EntityDamageType::Reality],
            )
            .is_empty()
        );
        assert_eq!(
            super::super::be_attacked_consumption_rule_ops(
                &managers,
                -1,
                &[EntityDamageType::Reality, EntityDamageType::Mental],
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
}
