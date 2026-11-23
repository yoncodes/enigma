use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;
use crate::engine::{
    event::{bus::EventBus, payload::BattleEvent},
    manager::buff::{BuffCommand, BuffGrant},
    runtime::executor::execute_rule_op,
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};

#[test]
fn replacing_hp_rate_keeps_the_aggregate_and_the_clamped_current_hp() {
    crate::test_support::init_config();
    let mut managers = BattleManagers::seeded(&Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(13_096),
                attr: Some(HeroAttribute {
                    hp: Some(13_096),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let mut events = EventBus::default();
    for (buff_id, expected, current) in [
        (2140002, vec![15_715], 15_715),
        (31460121, vec![17_286], 17_286),
        (31460121, vec![15_715, 17_286], 15_715),
    ] {
        let changes = managers
            .execute_buff(BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 10,
                target_uid: 10,
                buff_id,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }))
            .unwrap();
        let mut observed = Vec::new();
        for event in changes.events() {
            for (feature, op) in transaction_rule_ops(&managers, &event) {
                if matches!(&op, RuleOp::Command(BattleCommand::Hp(_))) {
                    execute_rule_op(&mut managers, &mut events, op.clone()).unwrap();
                    observed.push(managers.hp.max(10));
                    assert_eq!(
                        super::super::transaction_publication(&feature, &op, &event, &managers),
                        Some(crate::engine::event::subscription::PublicationPhase::BeforePublish)
                    );
                }
            }
        }
        assert_eq!(observed, expected);
        assert_eq!(managers.hp.current(10), current);
    }
}

#[test]
fn configured_hp_rate_emits_one_max_hp_command() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(70015),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let feature = ActiveBuffFeature {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 70015,
        amount: 1,
        team_type: 1,
        owner_alive: true,
        act_type: "Attr".to_owned(),
        effect_time: 0,
        effect_condition: 0,
        raw: "100#101#500".to_owned(),
        values: vec![100, 101, 500],
    };
    let output = max_hp_rule_op(&managers, &feature, 1).unwrap();

    execute_rule_op(&mut managers, &mut EventBus::default(), output).unwrap();

    assert_eq!(managers.hp.max(10), 1_500);
    assert_eq!(managers.hp.current(10), 1_500);
}

#[test]
fn aggregate_rate_keeps_fractional_progress_between_stacks() {
    assert_eq!(scaled_hp(18_148, 150) - scaled_hp(18_148, 120), 545);
    assert_eq!(scaled_hp(18_148, 120) - scaled_hp(18_148, 90), 544);
}

#[test]
fn consumed_attribute_stack_does_not_emit_a_refresh_marker() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(433911),
                    from_uid: Some(10),
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
    let event = BattleEvent::BuffChanged(crate::engine::event::payload::BuffChangeEvent {
        source_uid: 10,
        target_uid: 10,
        buff_uid: 20,
        buff_id: 433911,
        before_amount: 2,
        after_amount: 1,
        act_id: 0,
        act_value: 0,
        replaced_fixed_max_hp: None,
        replaced_buff_id: None,
        expiry: crate::engine::event::payload::BuffExpiry::None,
    });

    assert!(transaction_rule_ops(&managers, &event).is_empty());
}

#[test]
fn source_snapshot_refresh_owns_its_attribute_marker_decision() {
    crate::test_support::init_config();
    let managers = BattleManagers::default();
    let event = BattleEvent::BuffChanged(crate::engine::event::payload::BuffChangeEvent {
        source_uid: 10,
        target_uid: 11,
        buff_uid: 20,
        buff_id: 31450111,
        before_amount: 1,
        after_amount: 2,
        act_id: 0,
        act_value: 0,
        replaced_fixed_max_hp: None,
        replaced_buff_id: None,
        expiry: crate::engine::event::payload::BuffExpiry::None,
    });

    assert!(transaction_rule_ops(&managers, &event).is_empty());
}

#[test]
fn halo_children_leave_refresh_marker_ownership_to_their_fanout() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: [10, 11, 12]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    team_type: Some(1),
                    current_hp: Some(1_000),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let grant = || {
        BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid: 10,
            target_uid: 10,
            buff_id: 312411443,
            amount: Some(1),
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };
    managers.execute_buff(grant()).unwrap();
    let changes = managers.execute_buff(grant()).unwrap();
    let changed = changes
        .events()
        .into_iter()
        .filter(|event| matches!(event, BattleEvent::BuffChanged(_)))
        .collect::<Vec<_>>();

    assert_eq!(changed.len(), 3);
    assert_eq!(transaction_rule_ops(&managers, &changed[0]).len(), 1);
    assert!(transaction_rule_ops(&managers, &changed[1]).is_empty());
    assert!(transaction_rule_ops(&managers, &changed[2]).is_empty());
}
