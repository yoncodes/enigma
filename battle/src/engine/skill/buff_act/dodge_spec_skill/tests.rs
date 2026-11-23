use super::*;
use crate::engine::{
    event::{kind::EventKind, subscription::SubscriptionKey},
    skill::{action::ActionEvent, rule::DefinitionKey, subscriber::BuffActSubscriber},
};
use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

#[test]
fn exact_dodge_argument_domains_are_distinct() {
    assert!(supports_skill_slots(&[1, 2]));
    assert!(!supports_skill_slots(&[1, 2, 3]));
    assert!(!supports_skill_slots(&[]));
    assert!(supports_damage_types(&[1, 2, 3]));
    assert!(!supports_damage_types(&[3]));
    assert!(!supports_damage_types(&[2, 3]));
    assert!(!supports_damage_types(&[1, 4]));
}

#[test]
fn skill_slot_and_damage_type_dodges_are_not_collapsed() {
    crate::test_support::init_config();
    let entity = |uid, buff_id| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(1_000),
        attr: Some(HeroAttribute {
            hp: Some(1_000),
            ..Default::default()
        }),
        buffs: vec![BuffInfo {
            uid: Some(uid.unsigned_abs() as i64),
            buff_id: Some(buff_id),
            from_uid: Some(uid),
            count: Some(1),
            ..Default::default()
        }],
        ..Default::default()
    };
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![
                entity(-1, 710601),
                entity(-2, 3070),
                entity(-3, 3080),
                entity(-4, 90201),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);

    assert!(avoidance(&managers, -1, 1, EntityDamageType::Mental).is_some());
    assert!(avoidance(&managers, -1, 3, EntityDamageType::Reality).is_none());
    assert!(avoidance(&managers, -2, 2, EntityDamageType::Reality).is_some());
    assert!(avoidance(&managers, -2, 1, EntityDamageType::Mental).is_none());
    assert!(avoidance(&managers, -2, 3, EntityDamageType::Reality).is_none());
    assert!(avoidance(&managers, -3, 1, EntityDamageType::Mental).is_some());
    assert!(avoidance(&managers, -4, 3, EntityDamageType::Reality).is_some());
}

#[test]
fn ignore_dodge_is_owned_by_the_attacker_buff() {
    crate::test_support::init_config();
    let managers = BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(30860141),
                    from_uid: Some(10),
                    count: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    assert!(ignored(&managers, 10));
    assert!(!ignored(&managers, 11));
}

#[test]
fn exact_dodge_acts_keep_distinct_wire_markers() {
    let feature = |act_id, act_type: &str| ActiveBuffFeature {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 30,
        amount: 1,
        team_type: 1,
        owner_alive: true,
        act_type: act_type.to_owned(),
        effect_time: 207,
        effect_condition: 0,
        raw: format!("{act_id}#1#2"),
        values: vec![act_id, 1, 2],
    };

    assert_eq!(
        marker_effect_type(&feature(505, "DodgeSpecSkill")),
        Some(sonettobuf::effect_type_enum::EffectType::Dodgespecskill as i32)
    );
    assert_eq!(
        marker_effect_type(&feature(507, "DodgeSpecSkill2")),
        Some(sonettobuf::effect_type_enum::EffectType::Dodgespecskill2 as i32)
    );
}

#[test]
fn temporary_dodge_expires_only_after_its_owner_acts() {
    let managers = BattleManagers::default();
    let subscriber = BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 710601,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::AllyAction,
            DefinitionKey::new(505, "DodgeSpecSkill"),
        ),
        act_type: "DodgeSpecSkill".to_owned(),
        effect_time: 207,
        effect_condition: 0,
        args: vec![1, 2],
        raw: "505#1#2".to_owned(),
    };
    let action = |source_uid| {
        BattleEvent::AllyAction(ActionEvent {
            source_uid,
            ..Default::default()
        })
    };

    assert!(
        rule_ops(&managers, &subscriber, &action(11))
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        rule_ops(&managers, &subscriber, &action(10))
            .unwrap()
            .as_slice(),
        [scoped] if matches!(
            &scoped.op,
            RuleOp::Command(BattleCommand::Buff(
                crate::engine::manager::buff::BuffCommand::ExpireAction(remove)
            )) if remove.target_uid == 10
            && remove.selector
                == crate::engine::manager::buff::BuffRemoveSelector::Uid(20)
        )
    ));
}

#[test]
fn counted_damage_type_dodge_removes_its_depleted_buff() {
    crate::test_support::init_config();
    let managers = BattleManagers::seeded(&Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(90201),
                    from_uid: Some(-1),
                    count: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let feature = managers
        .buff
        .active_features(&crate::engine::manager::hp::HpManager::default())
        .into_iter()
        .next()
        .unwrap();

    assert!(matches!(
        trigger_rule_ops(&feature).unwrap().as_slice(),
        [RuleOp::BuffFeatureMarker {
            effect_type,
            buff_act_id: 507,
            ..
        }] if *effect_type
            == sonettobuf::effect_type_enum::EffectType::Dodgespecskill2 as i32
    ));
    let subscriber =
        super::super::subscriber_from_feature(feature.clone(), EventKind::BuffFeatureTriggered)
            .unwrap();
    let event = BattleEvent::BuffFeatureTriggered(
        crate::engine::event::payload::BuffFeatureTriggeredEvent {
            owner_uid: -1,
            source_uid: -1,
            target_uid: -1,
            buff_uid: 20,
            buff_id: 90201,
            act_id: 507,
        },
    );
    assert!(matches!(
        rule_ops(&managers, &subscriber, &event).unwrap().as_slice(),
        [scoped]
            if scoped.scope == super::super::BuffActFrameScope::SubscriberFrame
                && scoped.source == super::super::BuffActFrameSource::Applier
                && matches!(
                    &scoped.op,
                    RuleOp::Command(BattleCommand::Buff(BuffCommand::ConsumeCount(
                        BuffConsume {
                            selector: BuffSelector::Uid(20),
                            amount: 1,
                            depleted: DepletedBuff::Remove,
                            ..
                        }
                    )))
                )
    ));
}

#[test]
fn configured_owner_attack_expiry_is_not_consumed_on_dodge() {
    crate::test_support::init_config();
    let managers = BattleManagers::default();
    let subscriber = BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 2220010,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::AllyAction,
            DefinitionKey::new(507, "DodgeSpecSkill2"),
        ),
        act_type: "DodgeSpecSkill2".to_owned(),
        effect_time: 207,
        effect_condition: 0,
        args: vec![1, 2],
        raw: "507#1#2".to_owned(),
    };
    let feature = ActiveBuffFeature {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 2220010,
        amount: 1,
        team_type: 1,
        owner_alive: true,
        act_type: "DodgeSpecSkill2".to_owned(),
        effect_time: 207,
        effect_condition: 0,
        raw: "507#1#2".to_owned(),
        values: vec![507, 1, 2],
    };

    assert_eq!(trigger_rule_ops(&feature).unwrap().len(), 1);
    assert!(
        rule_ops(
            &managers,
            &subscriber,
            &BattleEvent::AllyAction(ActionEvent {
                source_uid: 10,
                is_attack: false,
                ..Default::default()
            })
        )
        .unwrap()
        .is_empty()
    );
    assert!(matches!(
        rule_ops(
            &managers,
            &subscriber,
            &BattleEvent::AllyAction(ActionEvent {
                source_uid: 10,
                is_attack: true,
                ..Default::default()
            })
        )
        .unwrap()
        .as_slice(),
        [scoped] if matches!(
            &scoped.op,
            RuleOp::Command(BattleCommand::Buff(BuffCommand::ExpireAction(_)))
        )
    ));
}

#[test]
fn counted_damage_type_dodge_is_a_supported_noop_on_owner_action() {
    crate::test_support::init_config();
    let managers = BattleManagers::default();
    let subscriber = BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 90201,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::AllyAction,
            DefinitionKey::new(507, "DodgeSpecSkill2"),
        ),
        act_type: "DodgeSpecSkill2".to_owned(),
        effect_time: 207,
        effect_condition: 0,
        args: vec![1, 2, 3],
        raw: "507#1#2#3".to_owned(),
    };

    assert!(
        rule_ops(
            &managers,
            &subscriber,
            &BattleEvent::AllyAction(ActionEvent {
                source_uid: 10,
                is_attack: true,
                ..Default::default()
            })
        )
        .unwrap()
        .is_empty()
    );
}
