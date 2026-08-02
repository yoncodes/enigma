use super::*;
use crate::engine::skill::rule::{DefinitionKey, RuleDomain};

#[test]
fn action_start_snapshot_is_stable_per_skill() {
    let mut hp = HpManager::default();
    hp.states.insert(
        10,
        HpState {
            current: 80,
            max: 100,
            base_max: 100,
        },
    );

    hp.capture_action_start(10, 1001);
    hp.lose(10, 30, 0).unwrap();
    hp.capture_action_start(10, 1002);

    assert_eq!(hp.action_start(10, 1001).unwrap().current, 80);
    assert_eq!(hp.action_start(10, 1002).unwrap().current, 50);
}

#[test]
fn damage_command_commits_shield_hp_crit_and_death_once() {
    let mut hp = HpManager::default();
    hp.states.insert(
        10,
        HpState {
            current: 50,
            max: 100,
            base_max: 100,
        },
    );
    hp.set_shield(10, 20);
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "Damage"),
    };

    let changes = hp
        .execute_command(HpCommand::Damage(HpDamage {
            origin,
            source_uid: 1,
            target_uid: 10,
            amount: 80,
            config_effect: 7,
            effect_kind: DamageEffectKind::Critical,
            assassinate: false,
            ignore_riposte: true,
            hurt: HurtInfoData {
                from_uid: 0,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 0,
                skill_id: 0,
                damage_from: HurtDamageFromType::Skill,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: 0,
                display_amount: None,
            },
        }))
        .unwrap();

    assert_eq!(changes.origin, origin);
    assert_eq!(changes.shield_absorbed.unwrap().absorbed, 20);
    assert_eq!(changes.hp.unwrap().delta, -50);
    assert_eq!(changes.hp.unwrap().hurt.unwrap().reduce_hp, -50);
    assert_eq!(changes.applied_damage(), 70);
    assert!(changes.hp.unwrap().hurt.unwrap().is_crit);
    assert_eq!(changes.death.unwrap().target_uid, 10);
    assert_eq!(
        changes.events(),
        vec![
            BattleEvent::HpLost {
                origin,
                source_uid: 1,
                skill_id: 0,
                target_uid: 10,
                amount: 50,
                buff_uid: None,
            },
            BattleEvent::Hit(HitEvent {
                origin,
                source_uid: 1,
                target_uid: 10,
                skill_id: 0,
                amount: 50,
                shield_absorbed: 20,
                damage_from: HurtDamageFromType::Skill,
                assassinate: false,
                ignore_riposte: true,
            }),
            BattleEvent::EntityDied(EntityDiedEvent {
                source_uid: 1,
                target_uid: 10,
            }),
        ]
    );
    assert_eq!(hp.current(10), 0);
    assert_eq!(hp.total_damage_dealt(1), 70);
    assert_eq!(hp.total_damage_taken(10), 70);
}

#[test]
fn shield_only_damage_is_a_hit_but_not_hp_loss() {
    let mut hp = HpManager::default();
    hp.states.insert(
        10,
        HpState {
            current: 50,
            max: 100,
            base_max: 100,
        },
    );
    hp.set_shield(10, 20);

    let changes = hp
        .execute_command(HpCommand::Damage(HpDamage {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "Damage"),
            },
            source_uid: 1,
            target_uid: 10,
            amount: 10,
            config_effect: 7,
            effect_kind: DamageEffectKind::Normal,
            assassinate: false,
            ignore_riposte: false,
            hurt: HurtInfoData {
                from_uid: 0,
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
        }))
        .unwrap();

    assert_eq!(
        changes.events(),
        vec![BattleEvent::Hit(HitEvent {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "Damage"),
            },
            source_uid: 1,
            target_uid: 10,
            skill_id: 123,
            amount: 0,
            shield_absorbed: 10,
            damage_from: HurtDamageFromType::Skill,
            assassinate: false,
            ignore_riposte: false,
        })]
    );
}

#[test]
fn avoided_damage_projects_zero_without_publishing_a_hit() {
    let mut hp = HpManager::default();
    hp.states.insert(
        10,
        HpState {
            current: 50,
            max: 100,
            base_max: 100,
        },
    );
    let changes = hp
        .execute_command(
            crate::engine::damage::handler::resolve_avoided_attack_command(
                1,
                10,
                CommandOrigin {
                    domain: RuleDomain::Skill,
                    key: DefinitionKey::new(1, "SkillDamage"),
                },
            ),
        )
        .unwrap();

    assert_eq!(changes.damage.unwrap().amount, 0);
    assert_eq!(
        changes.damage.unwrap().effect_kind,
        DamageEffectKind::Avoided
    );
    assert_eq!(hp.current(10), 50);
    assert!(changes.events().is_empty());
}

#[test]
fn round_skill_damage_ledger_counts_committed_skill_damage_only() {
    let mut hp = HpManager::default();
    hp.states.insert(
        10,
        HpState {
            current: 1_000,
            max: 1_000,
            base_max: 1_000,
        },
    );
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "Damage"),
    };
    let damage = |source_uid, amount, damage_from| {
        HpCommand::Damage(HpDamage {
            origin,
            source_uid,
            target_uid: 10,
            amount,
            config_effect: -1,
            effect_kind: DamageEffectKind::Normal,
            assassinate: false,
            ignore_riposte: false,
            hurt: HurtInfoData {
                from_uid: source_uid,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 1,
                skill_id: 1,
                damage_from,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: 0,
                display_amount: None,
            },
        })
    };

    hp.execute_command(damage(1, 200, HurtDamageFromType::Skill))
        .unwrap();
    hp.execute_command(damage(2, 300, HurtDamageFromType::Skill))
        .unwrap();
    hp.execute_command(damage(1, 100, HurtDamageFromType::SkillEffect))
        .unwrap();

    assert_eq!(hp.skill_damage_from_sources(10, &[1, 2]), 500);
    assert_eq!(hp.skill_damage_from_sources(10, &[1]), 200);
    hp.begin_round();
    assert_eq!(hp.skill_damage_from_sources(10, &[1, 2]), 0);
}

#[test]
fn shielded_buff_damage_does_not_publish_another_hit() {
    let mut hp = HpManager::default();
    hp.states.insert(
        10,
        HpState {
            current: 50,
            max: 100,
            base_max: 100,
        },
    );
    hp.set_shield(10, 20);

    let changes = hp
        .execute_command(HpCommand::Damage(HpDamage {
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(721, "DotNoLimit"),
            },
            source_uid: 1,
            target_uid: 10,
            amount: 10,
            config_effect: 0,
            effect_kind: DamageEffectKind::Genesis,
            assassinate: false,
            ignore_riposte: false,
            hurt: HurtInfoData {
                from_uid: 1,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 0,
                skill_id: 0,
                damage_from: HurtDamageFromType::Buff,
                buff_act_id: 721,
                buff_uid: 82,
                hurt_effect_type: sonettobuf::effect_type_enum::EffectType::Origindamage as i32,
                display_amount: None,
            },
        }))
        .unwrap();

    assert_eq!(changes.shield_absorbed.unwrap().absorbed, 10);
    assert!(changes.hp.is_none());
    assert!(changes.events().is_empty());
}

#[test]
fn fixed_loss_heal_and_shield_share_the_command_path() {
    let mut hp = HpManager::default();
    hp.states.insert(
        10,
        HpState {
            current: 50,
            max: 100,
            base_max: 100,
        },
    );
    let origin = CommandOrigin {
        domain: RuleDomain::BuffAct,
        key: DefinitionKey::new(1, "HpCommand"),
    };

    hp.execute_command(HpCommand::Lose(HpLoss {
        origin,
        source_uid: 1,
        target_uid: 10,
        amount: 20,
        config_effect: 0,
        hurt: None,
    }))
    .unwrap();
    let heal = hp
        .execute_command(HpCommand::Heal(HpHeal {
            origin,
            source_uid: 1,
            target_uid: 10,
            amount: 10,
            config_effect: 0,
            kind: HpHealKind::Critical,
        }))
        .unwrap();
    let shield = hp
        .execute_command(HpCommand::GrantShield(ShieldGrant {
            origin,
            source_uid: 1,
            target_uid: 10,
            amount: 30,
            max: 50,
        }))
        .unwrap();

    assert_eq!(hp.current(10), 40);
    assert_eq!(
        heal.hp.unwrap().effect_type,
        sonettobuf::effect_type_enum::EffectType::Healcrit as i32
    );
    assert!(matches!(
        heal.events().as_slice(),
        [BattleEvent::HpHealed {
            source_uid: 1,
            target_uid: 10,
            amount: 10,
            ..
        }]
    ));
    assert_eq!(shield.shield_granted.unwrap().after, 30);
}

#[test]
fn max_hp_adjustment_mutates_current_and_max_without_hp_loss() {
    let mut hp = HpManager::default();
    hp.states.insert(
        10,
        HpState {
            current: 50,
            max: 100,
            base_max: 100,
        },
    );
    let changes = hp
        .execute_command(HpCommand::AdjustMax(MaxHpAdjust {
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(100, "Attr"),
            },
            source_uid: 10,
            target_uid: 10,
            delta: 20,
        }))
        .unwrap();

    assert_eq!(
        changes.max_hp,
        Some(MaxHpChange {
            target_uid: 10,
            before_current: 50,
            before_max: 100,
            delta: 20,
            after_current: 70,
            after_max: 120,
        })
    );
    assert!(changes.events().is_empty());
}

#[test]
fn shield_absorbs_damage_before_hp() {
    let mut hp = HpManager::default();
    hp.set_shield(10, 100);

    assert_eq!(hp.absorb_shield(10, 60).unwrap().after, 40);
    let change = hp.absorb_shield(10, 80).unwrap();
    assert_eq!(change.absorbed, 40);
    assert_eq!(change.after, 0);
}

#[test]
fn shield_gain_reports_the_added_value_and_keeps_the_total() {
    let mut hp = HpManager::default();
    hp.set_shield(10, 100);

    let change = hp.add_shield(10, 40, 200);

    assert_eq!(change.added, 40);
    assert_eq!(change.after, 140);
}

#[test]
fn overkill_keeps_the_rolled_damage_for_the_packet() {
    let mut hp = HpManager::default();
    hp.states.insert(
        10,
        HpState {
            current: 50,
            max: 100,
            base_max: 100,
        },
    );
    let change = hp
        .lose_with_hurt(
            10,
            80,
            0,
            Some(HurtInfoData {
                from_uid: 1,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 0,
                skill_id: 0,
                damage_from: HurtDamageFromType::Skill,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: 0,
                display_amount: None,
            }),
        )
        .unwrap();

    assert_eq!(change.delta, -50);
    assert_eq!(change.hurt.unwrap().display_amount, Some(80));
}

#[test]
fn set_current_reports_the_new_absolute_hp() {
    let mut hp = HpManager::default();
    hp.states.insert(
        10,
        HpState {
            current: 80,
            max: 100,
            base_max: 100,
        },
    );
    let changes = hp
        .execute_command(HpCommand::SetCurrent(CurrentHpSet {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(20011, "AverageLife"),
            },
            source_uid: 1,
            target_uid: 10,
            value: 50,
            config_effect: 20011,
            effect_type: sonettobuf::effect_type_enum::EffectType::Averagelife as i32,
        }))
        .unwrap();

    let change = changes.hp.unwrap();
    assert_eq!((change.before, change.delta, change.after), (80, -30, 50));
    assert_eq!(change.display_amount, Some(50));
}

#[test]
fn kill_bypasses_shield_and_publishes_only_the_death_transition() {
    let mut hp = HpManager::default();
    hp.states.insert(
        10,
        HpState {
            current: 80,
            max: 100,
            base_max: 100,
        },
    );
    hp.set_shield(10, 50);
    let mut changes = hp
        .execute_command(HpCommand::Kill(HpKill {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60019, "KillTargets"),
            },
            source_uid: 1,
            target_uid: 10,
            config_effect: 60019,
        }))
        .unwrap();

    assert_eq!(hp.current(10), 0);
    assert_eq!(hp.shield(10), 50);
    assert_eq!(changes.kill, Some(60019));
    assert!(changes.caused_death());
    assert!(matches!(
        changes.events().as_slice(),
        [BattleEvent::EntityDied(EntityDiedEvent {
            source_uid: 1,
            target_uid: 10,
        })]
    ));
    changes.death.take();
    assert!(changes.caused_death());
}
