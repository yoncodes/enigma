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
            added_type_total: None,
            act_id: ACT_ID,
            act_value,
            replaced_fixed_max_hp: None,
            replaced_buff_id: None,
            expiry: crate::engine::event::payload::BuffExpiry::None,
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
        added_type_total: None,
        act_id: 0,
        act_value: 0,
        replaced_fixed_max_hp: None,
        replaced_buff_id: None,
        expiry: crate::engine::event::payload::BuffExpiry::None,
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
    assert_eq!(
        wire.max_hp.unwrap(),
        crate::engine::skill::buff_act::wire::MaxHpWireRule::Fixed {
            repeats: 1,
            buff_act_id: 0,
        }
    );
    assert!(crate::engine::manager::buff::wire_markers(108_300_003, WirePhase::Add).is_empty());
}
