use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;
use crate::engine::{
    event::{kind::EventKind, payload::ShellChangeEvent, subscription::SubscriptionKey},
    manager::buff::CommandOrigin,
    skill::rule::{DefinitionKey, RuleDomain},
};

fn subscriber(
    owner_uid: i64,
    source_uid: i64,
    buff_id: i32,
    act_id: i32,
    act_type: &'static str,
    args: Vec<i32>,
) -> BuffActSubscriber {
    BuffActSubscriber {
        owner_uid,
        source_uid,
        buff_uid: 20,
        buff_id,
        team_type: 1,
        owner_alive: true,
        amount: 3,
        key: SubscriptionKey::new(EventKind::BeAttacked, DefinitionKey::new(act_id, act_type)),
        act_type: act_type.to_owned(),
        effect_time: 0,
        effect_condition: 0,
        args,
        raw: String::new(),
    }
}

#[test]
fn shell_pair_comes_from_the_stock_buffs_shell_process_feature() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::try_global().unwrap();

    assert_eq!(deployed_buff_id(31090111), Some(31090112));
    assert_eq!(resolve_deployed_buff_id(catalog, 31090111), Some(31090112));
    assert_eq!(deployed_buff_id(31090113), Some(31090114));
    assert_eq!(
        process_spec(31090118),
        Some(ShellProcessSpec {
            stock_buff_id: 31090117,
            deployed_buff_id: 31090118,
            moxie_chance: 250,
            moxie_delta: 1,
            heal_attr_id: 102,
            heal_rate: 400,
        })
    );
}

#[test]
fn stock_shell_preserves_shared_damage_cardinality() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31090111),
                    layer: Some(8),
                    ..Default::default()
                }],
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
    };
    let managers = BattleManagers::seeded(&fight);
    let event = BattleEvent::DamageShared {
        origin: CommandOrigin {
            domain: RuleDomain::BuffAct,
            key: DefinitionKey::new(872, "ShareHurt"),
        },
        source_uid: -1,
        target_uid: 10,
        participant_uids: vec![10, 11, 12],
        amount: 20,
        share_count: 3,
        action_share_count: 6,
        damage_from: HurtDamageFromType::Skill,
    };

    let pool = TargetPool::from_fight(&fight);
    let ops = rule_ops(
        &managers,
        &pool,
        &mut RoundDeterminism::default(),
        &subscriber(10, 10, 31090111, 870, "Shell", vec![1]),
        &event,
    )
    .unwrap();

    assert_eq!(ops.len(), 3);
    assert!(ops.iter().all(|op| matches!(
        op,
        RuleOp::Command(BattleCommand::Shell(ShellCommand::Deploy {
            source_uid: 10,
            target_uid: -1,
            stock_buff_id: 31090111,
            amount: 1,
            ..
        }))
    )));
}

#[test]
fn stock_shell_deploys_after_a_skill_effect_hit_but_not_after_buff_damage() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31090111),
                    layer: Some(8),
                    ..Default::default()
                }],
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
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let deploys = |damage_from| {
        let event = BattleEvent::Hit(crate::engine::event::payload::HitEvent {
            origin: CommandOrigin {
                domain: RuleDomain::Skill,
                key: DefinitionKey::new(109380001, "SkillDamage"),
            },
            source_uid: -1,
            target_uid: 10,
            skill_id: 109380001,
            amount: 20,
            resolved_damage: 20,
            shield_absorbed: 0,
            career_restraint: false,
            damage_from,
            share_count: 0,
            assassinate: false,
            ignore_riposte: false,
        });
        rule_ops(
            &managers,
            &pool,
            &mut RoundDeterminism::default(),
            &subscriber(10, 10, 31090111, 870, "Shell", vec![1]),
            &event,
        )
        .unwrap()
        .len()
    };

    // "After being attacked": an attack's additional skill-effect damage is still the attack.
    assert_eq!(deploys(HurtDamageFromType::SkillEffect), 1);
    assert_eq!(deploys(HurtDamageFromType::Buff), 0);
}

#[test]
fn deployed_shell_retrieves_after_a_skill_effect_hit_but_not_after_buff_damage() {
    crate::test_support::init_config();
    let managers = BattleManagers::default();
    let pool = TargetPool::default();
    let retrieves = |damage_from| {
        let event = BattleEvent::Hit(crate::engine::event::payload::HitEvent {
            origin: CommandOrigin {
                domain: RuleDomain::Skill,
                key: DefinitionKey::new(100002, "EzioBigSkillTyp2"),
            },
            source_uid: 10,
            target_uid: -1,
            skill_id: 312301333,
            amount: 20,
            resolved_damage: 20,
            shield_absorbed: 0,
            career_restraint: false,
            damage_from,
            share_count: 0,
            assassinate: true,
            ignore_riposte: false,
        });
        rule_ops(
            &managers,
            &pool,
            &mut RoundDeterminism::default(),
            &subscriber(-1, 10, 31090112, 871, "ShellDebuff", vec![1]),
            &event,
        )
        .unwrap()
        .len()
    };

    assert_eq!(retrieves(HurtDamageFromType::SkillEffect), 1);
    assert_eq!(retrieves(HurtDamageFromType::Buff), 0);
}

#[test]
fn shell_lock_redeploys_only_retrievals_from_its_carrier() {
    let managers = BattleManagers::default();
    let pool = TargetPool::default();
    let event = BattleEvent::ShellChanged(ShellChangeEvent {
        kind: ShellChangeKind::Retrieved,
        source_uid: 10,
        target_uid: -1,
        stock_buff_id: 31090111,
        deployed_buff_id: 31090112,
        amount: 3,
        transaction_amount: 3,
        settles_transaction: true,
    });
    let ops = rule_ops(
        &managers,
        &pool,
        &mut RoundDeterminism::default(),
        &subscriber(-1, 10, 31090131, 873, "ShellLock", Vec::new()),
        &event,
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Shell(
            ShellCommand::Deploy {
                source_uid: 10,
                target_uid: -1,
                stock_buff_id: 31090111,
                amount: 3,
                ..
            }
        ))]
    ));
}

#[test]
fn deployed_shell_reads_each_configured_extra_action_attribute_per_layer() {
    let feature = crate::engine::manager::buff::ActiveBuffFeature {
        owner_uid: -1,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 31090118,
        amount: 4,
        team_type: 2,
        owner_alive: true,
        act_type: "ShellDebuff".into(),
        effect_time: 0,
        effect_condition: 0,
        raw: "871#1#203#60#205#60".into(),
        values: vec![871, 1, 203, 60, 205, 60],
    };

    assert_eq!(
        extra_action_attribute_delta(&feature, AttrId::CriticalDmg),
        240
    );
    assert_eq!(
        extra_action_attribute_delta(&feature, AttrId::DmgBonus),
        240
    );
    assert_eq!(extra_action_attribute_delta(&feature, AttrId::Attack), 0);
}

#[test]
fn linked_shell_attributes_keep_critical_damage_on_extra_actions() {
    use crate::engine::skill::condition::extra::ExtraSkillKind;

    for kind in [
        0,
        ExtraSkillKind::ExtraAction.id(),
        ExtraSkillKind::FollowUp.id(),
        ExtraSkillKind::Riposte.id(),
    ] {
        assert!(linked_damage_inherits_attribute(kind, AttrId::DmgBonus));
    }
    assert!(linked_damage_inherits_attribute(
        ExtraSkillKind::ExtraAction.id(),
        AttrId::CriticalDmg,
    ));
    for kind in [
        0,
        ExtraSkillKind::FollowUp.id(),
        ExtraSkillKind::Riposte.id(),
    ] {
        assert!(!linked_damage_inherits_attribute(kind, AttrId::CriticalDmg,));
    }
    assert!(!linked_damage_inherits_attribute(
        ExtraSkillKind::ExtraAction.id(),
        AttrId::Attack,
    ));
}

#[test]
fn shell_process_uses_configured_retrieval_total_to_heal_each_living_ally_once() {
    crate::test_support::init_config();
    let entity = |uid, buff: Option<BuffInfo>| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(500),
        attr: Some(HeroAttribute {
            hp: Some(2_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        buffs: buff.into_iter().collect(),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(
                    10,
                    Some(BuffInfo {
                        uid: Some(20),
                        buff_id: Some(31090111),
                        from_uid: Some(10),
                        layer: Some(8),
                        ..Default::default()
                    }),
                ),
                entity(11, None),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let event = BattleEvent::ShellChanged(ShellChangeEvent {
        kind: ShellChangeKind::Retrieved,
        source_uid: 10,
        target_uid: -1,
        stock_buff_id: 31090111,
        deployed_buff_id: 31090112,
        amount: 2,
        transaction_amount: 2,
        settles_transaction: true,
    });
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_indirect_heal_crits([
        ((999, 10, 10), false),
        ((31090112, 10, 10), true),
        ((31090112, 10, 11), false),
    ]);

    let ops = rule_ops(
        &managers,
        &pool,
        &mut determinism,
        &subscriber(
            10,
            10,
            31090111,
            869,
            "ShellProcess",
            vec![31090111, 31090112, 200, 1, 102, 300],
        ),
        &event,
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::HpBatch(heals))]
            if matches!(
                heals.as_slice(),
                [
                    crate::engine::manager::hp::HpCommand::Heal(
                        crate::engine::manager::hp::HpHeal {
                            target_uid: 10,
                            kind: crate::engine::manager::hp::HpHealKind::Critical,
                            ..
                        }
                    ),
                    crate::engine::manager::hp::HpCommand::Heal(
                        crate::engine::manager::hp::HpHeal {
                            target_uid: 11,
                            amount: 600,
                            kind: crate::engine::manager::hp::HpHealKind::Normal,
                            ..
                        }
                    )
                ]
            )
    ));
}

#[test]
fn deployed_shell_rolls_the_configured_moxie_gain_from_the_shared_rng() {
    let managers = BattleManagers::default();
    let pool = TargetPool::default();
    let event = BattleEvent::ShellChanged(ShellChangeEvent {
        kind: ShellChangeKind::Deployed,
        source_uid: 10,
        target_uid: -1,
        stock_buff_id: 31090111,
        deployed_buff_id: 31090112,
        amount: 3,
        transaction_amount: 3,
        settles_transaction: true,
    });
    let process = |owner_uid, buff_id| {
        let mut determinism = RoundDeterminism::default();
        determinism.enqueue_permille_rolls([0]);
        rule_ops(
            &managers,
            &pool,
            &mut determinism,
            &subscriber(
                owner_uid,
                10,
                buff_id,
                869,
                "ShellProcess",
                vec![31090111, 31090112, 200, 1, 102, 300],
            ),
            &event,
        )
        .unwrap()
    };

    // A fully deployed stock is removed, so the shells that received the stacks answer.
    assert!(process(10, 31090111).is_empty());
    let ops = process(-1, 31090112);

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::ExPoint(
            crate::engine::manager::ex_point::ExPointCommand::Change(
                crate::engine::manager::ex_point::ExPointChange {
                    target_uid: 10,
                    delta: 1,
                    ..
                }
            )
        ))]
    ));
}
