use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute, PowerInfo};

use super::*;
use crate::engine::{
    event::kind::EventKind,
    manager::{
        buff::{BuffCommand, BuffGrant, BuffRemove, BuffRemoveSelector, CommandOrigin},
        eureka::{EUREKA_RESOURCE_ID, EurekaChange, EurekaCommand},
        ex_point::{ExPointChange, ExPointCommand},
        hp::{DamageEffectKind, HpCommand, HpDamage, HpKill, HurtDamageFromType, HurtInfoData},
        shield::{ShieldCarrierUid, ShieldCommand, ShieldScope},
    },
    skill::rule::{DefinitionKey, RuleDomain},
};

#[test]
fn exact_rule_command_reaches_the_buff_transaction() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let output = RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
        origin: CommandOrigin {
            domain: RuleDomain::BuffAct,
            key: DefinitionKey::new(503, "AddToTarget"),
        },
        source_uid: 10,
        target_uid: 10,
        buff_id: 101,
        amount: None,
        occurrences: 1,
        child_uid_reservations: 0,
    })));
    let mut events = EventBus::default();

    let RuleOutcome::Buff(changes) =
        execute_rule_op(&mut managers, &mut events, output).expect("committed buff change")
    else {
        panic!("expected buff outcome");
    };

    assert_eq!(changes.change.added.unwrap().buff.buff_id, Some(101));
    assert_eq!(
        events.pop().map(|event| event.kind()),
        Some(EventKind::BuffAdded)
    );
}

#[test]
fn shield_event_observes_the_committed_team_shared_value() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let mut events = EventBus::default();
    let outcome = execute_rule_op(
        &mut managers,
        &mut events,
        RuleOp::Command(BattleCommand::Shield(ShieldCommand {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60290, "SupplyTeamShareShield"),
            },
            source_uid: 10,
            target_uid: 10,
            buff_id: 31430144,
            amount_attr: crate::engine::entity::attr::AttrId::Attack,
            amount_rate: 2_800,
            bonus: None,
            max_attr: crate::engine::entity::attr::AttrId::Attack,
            max_rate: 12_500,
            scope: ShieldScope::TeamShared,
            carrier_uid: ShieldCarrierUid::Definition,
        })),
    )
    .unwrap();

    let RuleOutcome::Shield(changes) = outcome else {
        panic!("expected shield outcome");
    };
    assert_eq!(
        changes
            .buff
            .as_ref()
            .and_then(|buff| buff.change.added.as_ref())
            .and_then(|added| {
                added
                    .buff
                    .act_info
                    .iter()
                    .find(|info| info.act_id == Some(1125))
            })
            .map(|info| info.param.as_slice()),
        Some([2_800].as_slice())
    );
    assert!(matches!(
        events.pop(),
        Some(crate::engine::event::payload::BattleEvent::BuffAdded(event))
            if event.act_id == 1125 && event.act_value == 2_800
    ));
}

#[test]
fn buff_events_are_available_between_fifo_commands() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let mut events = EventBus::default();
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "AddBuff"),
    };
    let grant = || {
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: 10,
            target_uid: 10,
            buff_id: 101,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        })))
    };

    execute_rule_op(&mut managers, &mut events, grant()).unwrap();
    assert!(matches!(
        events.pop(),
        Some(crate::engine::event::payload::BattleEvent::BuffAdded(_))
    ));
    execute_rule_op(
        &mut managers,
        &mut events,
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
            origin,
            target_uid: 10,
            selector: BuffRemoveSelector::ExactId(101),
        }))),
    )
    .unwrap();
    assert!(matches!(
        events.pop(),
        Some(crate::engine::event::payload::BattleEvent::BuffRemoved(_))
    ));
    execute_rule_op(&mut managers, &mut events, grant()).unwrap();
    assert!(matches!(
        events.pop(),
        Some(crate::engine::event::payload::BattleEvent::BuffAdded(_))
    ));
}

#[test]
fn committed_damage_publishes_hp_loss_hit_then_death() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(50),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let mut events = EventBus::default();
    let output = RuleOp::Command(BattleCommand::Hp(HpCommand::Damage(HpDamage {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(10001, "Damage"),
        },
        source_uid: 1,
        target_uid: 10,
        amount: 80,
        config_effect: 7,
        effect_kind: DamageEffectKind::Normal,
        assassinate: false,
        hurt: HurtInfoData {
            from_uid: 1,
            is_crit: false,
            career_restraint: false,
            reduce_hp: 0,
            effect_id: 0,
            skill_id: 123,
            damage_from: HurtDamageFromType::Skill,
            buff_act_id: 0,
            buff_uid: 0,
            hurt_effect_type: 0,
            display_amount: None,
        },
    })));

    assert!(matches!(
        execute_rule_op(&mut managers, &mut events, output),
        Ok(RuleOutcome::Hp(_))
    ));
    assert!(matches!(
        events.pop(),
        Some(crate::engine::event::payload::BattleEvent::HpLost { amount: 50, .. })
    ));
    assert!(matches!(
        events.pop(),
        Some(crate::engine::event::payload::BattleEvent::Hit(hit))
            if hit.target_uid == 10 && hit.skill_id == 123
    ));
    assert!(matches!(
        events.pop(),
        Some(crate::engine::event::payload::BattleEvent::EntityDied(death))
            if death.target_uid == 10
    ));
    assert!(events.is_empty());
}

#[test]
fn hp_batch_preserves_command_death_order() {
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: [-1, -2]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    current_hp: Some(1),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let mut events = EventBus::default();
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60019, "KillTargets"),
    };
    let mut outcome = execute_rule_op(
        &mut managers,
        &mut events,
        RuleOp::Command(BattleCommand::HpBatch(
            [-2, -1]
                .into_iter()
                .map(|target_uid| {
                    HpCommand::Kill(HpKill {
                        origin,
                        source_uid: 10,
                        target_uid,
                        config_effect: 60019,
                    })
                })
                .collect(),
        )),
    )
    .unwrap();

    assert_eq!(
        outcome
            .take_deaths()
            .into_iter()
            .map(|death| death.target_uid)
            .collect::<Vec<_>>(),
        vec![-2, -1]
    );
}

#[test]
fn fixed_hurt_resolves_damage_before_hp_commit_but_not_hp_loss() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-3),
                current_hp: Some(100),
                buffs: vec![sonettobuf::BuffInfo {
                    uid: Some(1),
                    buff_id: Some(2_112_021),
                    from_uid: Some(-2),
                    duration: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(10001, "Damage"),
    };
    let mut managers = BattleManagers::seeded(&fight);
    let mut events = EventBus::default();

    let damage = RuleOp::Command(BattleCommand::Hp(HpCommand::Damage(HpDamage {
        origin,
        source_uid: 10,
        target_uid: -3,
        amount: 500,
        config_effect: 0,
        effect_kind: DamageEffectKind::Critical,
        assassinate: false,
        hurt: HurtInfoData {
            from_uid: 10,
            is_crit: true,
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
    })));
    let RuleOutcome::Hp(execution) = execute_rule_op(&mut managers, &mut events, damage).unwrap()
    else {
        panic!("expected HP outcome");
    };
    assert_eq!(execution.changes.damage.as_ref().unwrap().amount, 1);
    assert_eq!(execution.changes.hp.as_ref().unwrap().delta, -1);
    assert_eq!(managers.hp.current(-3), 99);

    execute_rule_op(
        &mut managers,
        &mut events,
        RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(
            crate::engine::manager::hp::HpLoss {
                origin,
                source_uid: -3,
                target_uid: -3,
                amount: 10,
                config_effect: 0,
                hurt: None,
            },
        ))),
    )
    .unwrap();
    assert_eq!(managers.hp.current(-3), 89);
}

#[test]
fn committed_resources_publish_change_and_overflow_facts() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                ex_point: Some(4),
                power_infos: vec![PowerInfo {
                    power_id: Some(EUREKA_RESOURCE_ID),
                    num: Some(4),
                    max: Some(5),
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let mut events = EventBus::default();
    let ex_origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(20002, "AddExPoint"),
    };

    execute_rule_op(
        &mut managers,
        &mut events,
        RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Change(
            ExPointChange {
                origin: ex_origin,
                source_uid: 10,
                target_uid: 10,
                delta: 3,
                config_effect: 0,
                effect_type: 0,
            },
        ))),
    )
    .unwrap();

    assert!(matches!(
        events.pop(),
        Some(crate::engine::event::payload::BattleEvent::ExPointChanged(event))
            if event.applied_delta == 1 && event.kind.as_wire() == 0
    ));
    assert!(matches!(
        events.pop(),
        Some(crate::engine::event::payload::BattleEvent::ExPointOverflow(event))
            if event.overflow == 2
    ));

    execute_rule_op(
        &mut managers,
        &mut events,
        RuleOp::Command(BattleCommand::Eureka(EurekaCommand::Change(EurekaChange {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(50017, "ChangePower"),
            },
            source_uid: 10,
            target_uid: 10,
            power_id: EUREKA_RESOURCE_ID,
            delta: 3,
            effect_type: 0,
        }))),
    )
    .unwrap();

    assert!(matches!(
        events.pop(),
        Some(crate::engine::event::payload::BattleEvent::EurekaChanged(event))
            if event.applied_delta == 1 && event.overflow == 2
    ));
    assert!(events.is_empty());
}
