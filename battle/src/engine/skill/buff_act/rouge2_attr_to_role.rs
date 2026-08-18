use crate::engine::{
    entity::attr::AttrId,
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffAccumulateActValue, BuffCommand, BuffManager},
        hp::{HpCommand, MaxHpAdjust},
    },
    skill::{
        buff_act::{self, registry::BuffActKind},
        rule::output::{BattleCommand, RuleOp},
    },
};

pub const ACT_ID: i32 = 10_021;

const RULES: &[(i32, AttrId, i32)] = &[
    (101, AttrId::Hp, 20),
    (102, AttrId::ExtraDmg, 40),
    (103, AttrId::IncantationMight, 40),
    (104, AttrId::GenesisDmgBonus, 60),
    (104, AttrId::UltimateMight, 40),
];

pub fn supports(args: &[i32]) -> bool {
    let [mode_attr, raw_attr, multiplier] = args else {
        return false;
    };
    let Some(role_attr) = AttrId::from_raw(*raw_attr) else {
        return false;
    };
    RULES.contains(&(*mode_attr, role_attr, *multiplier))
}

pub fn attribute_delta(values: &[i32], attr_id: AttrId, buffs: &BuffManager) -> i32 {
    let [_, mode_attr, raw_attr, multiplier] = values else {
        return 0;
    };
    if !supports(&values[1..]) || AttrId::from_raw(*raw_attr) != Some(attr_id) {
        return 0;
    }
    buffs
        .mode_attribute(*mode_attr)
        .checked_mul(*multiplier)
        .unwrap_or_default()
}

pub fn hp_delta_fits(
    values: &[i32],
    buffs: &BuffManager,
    hp: &crate::engine::manager::hp::HpManager,
    target_uid: i64,
) -> bool {
    if values.get(2).copied() != Some(AttrId::Hp.id()) {
        return true;
    }
    let delta = attribute_delta(values, AttrId::Hp, buffs);
    delta == 0
        || (hp.current(target_uid).checked_add(delta).is_some()
            && hp.max(target_uid).checked_add(delta).is_some())
}

fn add_hp_delta(managers: &BattleManagers, feature: &ActiveBuffFeature) -> Option<i32> {
    let [_, mode_attr, raw_attr, multiplier] = feature.values.as_slice() else {
        return None;
    };
    if !supports(&feature.values[1..]) || AttrId::from_raw(*raw_attr) != Some(AttrId::Hp) {
        return None;
    }
    let delta = managers
        .buff
        .mode_attribute(*mode_attr)
        .checked_mul(*multiplier)?;
    if delta == 0 {
        return None;
    }
    managers.hp.current(feature.owner_uid).checked_add(delta)?;
    managers.hp.max(feature.owner_uid).checked_add(delta)?;
    Some(delta)
}

fn max_hp_rule_op(feature: &ActiveBuffFeature, delta: i32) -> Option<RuleOp> {
    if delta == 0 {
        return None;
    }
    Some(RuleOp::Command(BattleCommand::Hp(HpCommand::AdjustMax(
        MaxHpAdjust {
            origin: buff_act::feature_command_origin(feature)?,
            source_uid: feature.source_uid,
            target_uid: feature.owner_uid,
            delta,
        },
    ))))
}

pub fn transaction_rule_ops(
    managers: &BattleManagers,
    event: &crate::engine::event::payload::BattleEvent,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    use crate::engine::event::payload::BattleEvent;

    let (change, added) = match event {
        BattleEvent::BuffAdded(change) => (change, true),
        BattleEvent::BuffRemoved(change) => (change, false),
        BattleEvent::BuffChanged(_) => return Vec::new(),
        _ => return Vec::new(),
    };
    managers
        .buff
        .definition_features(change.buff_id)
        .into_iter()
        .filter(|feature| buff_act::is_kind(feature, BuffActKind::Rouge2AttrToRole))
        .flat_map(|mut feature| {
            feature.owner_uid = change.target_uid;
            feature.source_uid = change.source_uid;
            feature.buff_uid = change.buff_uid;
            let mut output = Vec::new();
            let hp_delta = if added {
                add_hp_delta(managers, &feature)
            } else if supports(&feature.values[1..])
                && feature.values.get(2).copied() == Some(AttrId::Hp.id())
            {
                change.act_value.checked_neg()
            } else {
                None
            };
            if let Some(delta) = hp_delta {
                if added && let Some(origin) = buff_act::feature_command_origin(&feature) {
                    output.push((
                        feature.clone(),
                        RuleOp::Command(BattleCommand::Buff(BuffCommand::AccumulateActValue(
                            BuffAccumulateActValue {
                                origin,
                                target_uid: feature.owner_uid,
                                buff_uid: feature.buff_uid,
                                act_id: ACT_ID,
                                delta,
                            },
                        ))),
                    ));
                }
                if let Some(op) = max_hp_rule_op(&feature, delta) {
                    output.push((feature.clone(), op));
                }
            }
            if added {
                output.push((
                    feature.clone(),
                    RuleOp::BuffFeatureMarker {
                        target_uid: feature.owner_uid,
                        effect_type: sonettobuf::effect_type_enum::EffectType::None as i32,
                        effect_num: 0,
                        buff_act_id: 0,
                    },
                ));
            }
            output
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, CustomData, Fight, FightEntityInfo, FightTeam, HeroAttribute};

    use super::*;
    use crate::engine::{
        event::{
            bus::EventBus,
            payload::{BattleEvent, BuffChangeEvent},
        },
        manager::buff::BuffGrant,
        runtime::executor::execute_rule_op,
        skill::buff_act::{registry, wire::WirePhase},
    };

    const MODE_DATA: &str = r#"{"attrFinalVal":{"101":9,"102":5,"103":4,"104":2}}"#;

    fn fight(custom_data: Vec<CustomData>, with_buff: bool) -> Fight {
        Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    buffs: with_buff
                        .then_some(BuffInfo {
                            uid: Some(20),
                            buff_id: Some(108_300_003),
                            from_uid: Some(0),
                            layer: Some(1),
                            ..Default::default()
                        })
                        .into_iter()
                        .collect(),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            custom_data,
            ..Default::default()
        }
    }

    fn rouge2(data: &str) -> CustomData {
        CustomData {
            r#type: Some(sonettobuf::custom_data::CustomDataType::Rouge2 as i32),
            data: Some(data.to_owned()),
        }
    }

    fn feature() -> ActiveBuffFeature {
        ActiveBuffFeature {
            owner_uid: 10,
            source_uid: 0,
            buff_uid: 20,
            buff_id: 108_300_003,
            amount: 1,
            team_type: 1,
            owner_alive: true,
            act_type: "Rouge2AttrToRole".to_owned(),
            effect_time: 0,
            effect_condition: 0,
            raw: "10021#101#101#20".to_owned(),
            values: vec![10_021, 101, 101, 20],
        }
    }

    #[test]
    fn accepts_only_the_five_observed_role_attribute_rules() {
        for &(mode_attr, role_attr, multiplier) in RULES {
            assert!(supports(&[mode_attr, role_attr.id(), multiplier]));
        }
        for args in [
            vec![101, 101],
            vec![101, 101, 21],
            vec![101, 102, 20],
            vec![999, 101, 20],
            vec![101, 999, 20],
        ] {
            assert!(!supports(&args), "unexpected supported args: {args:?}");
        }
    }

    #[test]
    fn malformed_or_ambiguous_mode_data_fails_closed() {
        crate::test_support::init_config();
        for custom_data in [
            Vec::new(),
            vec![rouge2("not json")],
            vec![rouge2(r#"{"attrFinalVal":{"bad":9}}"#)],
            vec![rouge2(r#"{"attrFinalVal":{"101":-1}}"#)],
            vec![rouge2(r#"{"attrFinalVal":{"101":9,"0101":3}}"#)],
            vec![rouge2(MODE_DATA), rouge2(MODE_DATA)],
        ] {
            let managers = BattleManagers::seeded(&fight(custom_data, false));
            assert_eq!(managers.buff.mode_attribute(101), 0);
        }
    }

    #[test]
    fn configured_buff_reads_each_mode_attribute_as_a_flat_role_delta() {
        crate::test_support::init_config();
        let mut managers = BattleManagers::seeded(&fight(vec![rouge2(MODE_DATA)], true));

        assert_eq!(
            managers.persistent_attribute_delta(10, AttrId::ExtraDmg),
            200
        );
        assert_eq!(
            managers.persistent_attribute_delta(10, AttrId::IncantationMight),
            160
        );
        assert_eq!(
            managers.persistent_attribute_delta(10, AttrId::GenesisDmgBonus),
            120
        );
        assert_eq!(
            managers.persistent_attribute_delta(10, AttrId::UltimateMight),
            80
        );
        assert_eq!(managers.buff.act_value(20, ACT_ID), 180);

        let overflow = BattleManagers::seeded(&fight(
            vec![rouge2(r#"{"attrFinalVal":{"102":2147483647}}"#)],
            true,
        ));
        assert_eq!(overflow.persistent_attribute_delta(10, AttrId::ExtraDmg), 0);
        assert_eq!(
            attribute_delta(&[10_021, 101, 102, 20], AttrId::ExtraDmg, &managers.buff),
            0
        );
        let removed = managers.buff.remove_by_id(10, 108_300_003);
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].fixed_max_hp_delta, 180);
    }

    #[test]
    fn hp_rule_shifts_current_and_max_on_add_and_remove() {
        crate::test_support::init_config();
        let mut managers = BattleManagers::seeded(&fight(vec![rouge2(MODE_DATA)], false));
        let mut events = EventBus::default();

        let add = max_hp_rule_op(&feature(), add_hp_delta(&managers, &feature()).unwrap()).unwrap();
        execute_rule_op(&mut managers, &mut events, add).unwrap();
        assert_eq!(
            (managers.hp.current(10), managers.hp.max(10)),
            (1_180, 1_180)
        );

        let remove = max_hp_rule_op(&feature(), -180).unwrap();
        execute_rule_op(&mut managers, &mut events, remove).unwrap();
        assert_eq!(
            (managers.hp.current(10), managers.hp.max(10)),
            (1_000, 1_000)
        );

        let mut overflow = managers;
        overflow.hp.set_max(10, i32::MAX);
        assert!(add_hp_delta(&overflow, &feature()).is_none());
        let rejected = overflow
            .execute_buff(BuffCommand::Grant(BuffGrant {
                origin: buff_act::feature_command_origin(&feature()).unwrap(),
                source_uid: 0,
                target_uid: 10,
                buff_id: 108_300_003,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 2,
            }))
            .unwrap();
        assert!(rejected.change.added.is_none());
        assert!(!overflow.buff.has_buff_id(10, 108_300_003));
        overflow.hp.set_max(10, 1_000);
        let accepted = overflow
            .execute_buff(BuffCommand::Grant(BuffGrant {
                origin: buff_act::feature_command_origin(&feature()).unwrap(),
                source_uid: 0,
                target_uid: 10,
                buff_id: 108_300_003,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }))
            .unwrap();
        let mut clean = BattleManagers::seeded(&fight(vec![rouge2(MODE_DATA)], false));
        let clean = clean
            .execute_buff(BuffCommand::Grant(BuffGrant {
                origin: buff_act::feature_command_origin(&feature()).unwrap(),
                source_uid: 0,
                target_uid: 10,
                buff_id: 108_300_003,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }))
            .unwrap();
        assert_eq!(
            accepted.change.added.unwrap().buff.uid,
            clean.change.added.unwrap().buff.uid
        );

        let removed = |act_value| {
            BattleEvent::BuffRemoved(BuffChangeEvent {
                source_uid: 0,
                target_uid: 10,
                buff_uid: 20,
                buff_id: 108_300_003,
                before_amount: 1,
                after_amount: 0,
                act_id: ACT_ID,
                act_value,
            })
        };
        assert!(transaction_rule_ops(&overflow, &removed(0)).is_empty());
        let remove_ops = transaction_rule_ops(&overflow, &removed(180));
        assert!(matches!(
            remove_ops.as_slice(),
            [(
                _,
                RuleOp::Command(BattleCommand::Hp(HpCommand::AdjustMax(MaxHpAdjust {
                    delta: -180,
                    ..
                })))
            )]
        ));
    }

    #[test]
    fn configured_transaction_emits_one_hp_change_and_five_ordered_markers() {
        crate::test_support::init_config();
        let managers = BattleManagers::seeded(&fight(vec![rouge2(MODE_DATA)], true));
        let event = BattleEvent::BuffAdded(BuffChangeEvent {
            source_uid: 0,
            target_uid: 10,
            buff_uid: 20,
            buff_id: 108_300_003,
            before_amount: 0,
            after_amount: 1,
            act_id: 0,
            act_value: 0,
        });
        let ops = transaction_rule_ops(&managers, &event);

        assert_eq!(ops.len(), 7);
        assert!(matches!(
            ops[0].1,
            RuleOp::Command(BattleCommand::Buff(BuffCommand::AccumulateActValue(_)))
        ));
        assert!(matches!(
            ops[1].1,
            RuleOp::Command(BattleCommand::Hp(HpCommand::AdjustMax(_)))
        ));
        assert!(ops[2..].iter().all(|(_, op)| matches!(
            op,
            RuleOp::BuffFeatureMarker { effect_type, effect_num: 0, buff_act_id: 0, .. }
                if *effect_type == sonettobuf::effect_type_enum::EffectType::None as i32
        )));
    }

    #[test]
    fn exact_registry_keeps_add_only_markers_and_one_hp_snapshot_pair() {
        crate::test_support::init_config();
        let definition = registry::find(10_021, "Rouge2AttrToRole").unwrap();
        assert_eq!(definition.kind, BuffActKind::Rouge2AttrToRole);
        let wire = definition.wire.unwrap();
        assert_eq!(
            wire.markers(WirePhase::Add),
            &[sonettobuf::effect_type_enum::EffectType::None as i32]
        );
        assert!(wire.markers(WirePhase::Refresh).is_empty());
        assert!(wire.markers(WirePhase::Static).is_empty());
        assert_eq!(wire.max_hp.unwrap().repeats, 1);
        assert!(crate::engine::manager::buff::wire_markers(108_300_003, WirePhase::Add).is_empty());
    }
}
