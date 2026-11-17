use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use crate::engine::{
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrant},
        hp::{DamageEffectKind, HpCommand, HpDamage, HurtDamageFromType, HurtInfoData},
    },
    runtime::{change::BattleChange, executor::RuleOutcome},
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};

use super::*;

fn origin() -> CommandOrigin {
    CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(103374111, "Damage"),
    }
}

fn damage(target_uid: i64, amount: i32) -> HpCommand {
    HpCommand::Damage(HpDamage {
        origin: origin(),
        source_uid: 99,
        target_uid,
        amount,
        config_effect: -1,
        effect_kind: DamageEffectKind::Normal,
        assassinate: false,
        ignore_riposte: false,
        hurt: HurtInfoData {
            from_uid: 99,
            is_crit: false,
            career_restraint: false,
            reduce_hp: 0,
            effect_id: 103374111,
            skill_id: 103374111,
            damage_from: HurtDamageFromType::Skill,
            buff_act_id: 0,
            buff_uid: 0,
            hurt_effect_type: 0,
            display_amount: None,
        },
    })
}

fn fight(current: i32) -> Fight {
    Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        attack: Some(1_622),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    team_type: Some(1),
                    current_hp: Some(10_903),
                    attr: Some(HeroAttribute {
                        hp: Some(10_903),
                        ..Default::default()
                    }),
                    buffs: vec![BuffInfo {
                        uid: Some(30),
                        buff_id: Some(31450143),
                        from_uid: Some(10),
                        layer: Some(1),
                        act_info: vec![BuffActInfo {
                            act_id: Some(1150),
                            param: vec![current, 8_108],
                            str_param: Some(String::new()),
                        }],
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn feature(managers: &BattleManagers) -> ActiveBuffFeature {
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find(|feature| {
            feature.owner_uid == 20 && super::super::is_kind(feature, BuffActKind::LostHpToFakeHp)
        })
        .expect("Everecho Remembrance feature should be active")
}

#[test]
fn initial_state_scales_the_source_attack_into_the_configured_capacity() {
    assert_eq!(
        initial_act_info(2_027, &[1150, 102, 4_000]),
        Some(BuffActInfo {
            act_id: Some(1150),
            param: vec![0, 8_108],
            str_param: Some(String::new()),
        })
    );
    assert!(initial_act_info(2_027, &[1150, 101, 4_000]).is_none());
    assert!(initial_act_info(2_027, &[1150, 102, 0]).is_none());
}

#[test]
fn losing_hp_accumulates_the_same_fraction_of_remembrance_before_damage_publish() {
    crate::test_support::init_config();
    let mut managers = BattleManagers::seeded(&fight(0));
    let updates = hp_loss_state_update(&managers, &feature(&managers), 2_786)
        .expect("Remembrance state is valid");
    assert!(matches!(
        updates.as_slice(),
        [super::super::HpLossStateUpdate {
            command: BuffCommand::SetInternalState(BuffSetState {
                act_info: Some(info), ..
            }),
            marker: BuffActInfoMarkerResult {
                target_uid: 20,
                buff_uid: 30,
                act_id: 1150,
                params,
                ..
            },
            after_hp: false,
        }] if info[0].param == [2_071, 8_108] && params == &[2_071, 8_108]
    ));

    managers.execute_buff(updates[0].command.clone()).unwrap();
    assert_eq!(state(&managers, 20).unwrap().unwrap().current, 2_071);
}

#[test]
fn losing_hp_caps_remembrance_at_its_initialized_maximum() {
    crate::test_support::init_config();
    let managers = BattleManagers::seeded(&fight(8_000));
    let updates = hp_loss_state_update(&managers, &feature(&managers), 2_786).unwrap();
    assert_eq!(updates[0].marker.params, [8_108, 8_108]);
}

#[test]
fn damage_records_zero_consumption_for_stored_remembrance() {
    crate::test_support::init_config();
    let managers = BattleManagers::seeded(&fight(2_071));

    assert_eq!(
        plan_damage(&managers, 20, 2_786)
            .unwrap()
            .unwrap()
            .consumption,
        crate::engine::manager::hp::FakeHpConsumption {
            buff_uid: 30,
            consumed: 0,
        }
    );
}

#[test]
fn hp_execution_commits_and_publishes_remembrance_before_the_damage() {
    crate::test_support::init_config();
    let mut managers = BattleManagers::seeded(&fight(0));
    let execution = managers.execute_rule_hp(damage(20, 2_786)).unwrap();

    assert_eq!(state(&managers, 20).unwrap().unwrap().current, 2_071);
    assert_eq!(
        execution.changes.fake_hp_consumed,
        Some(crate::engine::manager::hp::FakeHpConsumption {
            buff_uid: 30,
            consumed: 0,
        })
    );
    let changes = RuleOutcome::Hp(Box::new(execution)).changes();
    assert!(matches!(
        changes.as_slice(),
        [
            BattleChange::BuffActInfoMarker(BuffActInfoMarkerResult {
                act_id: 1150,
                params,
                ..
            }),
            BattleChange::Hp(_)
        ] if params == &[2_071, 8_108]
    ));
}

#[test]
fn granting_remembrance_embeds_state_without_a_separate_initial_marker() {
    crate::test_support::init_config();
    let mut initial = fight(0);
    initial.attacker.as_mut().unwrap().entitys[1].buffs.clear();
    initial.attacker.as_mut().unwrap().entitys[0]
        .buffs
        .push(BuffInfo {
            uid: Some(31),
            buff_id: Some(104310124),
            from_uid: Some(10),
            layer: Some(1),
            ..Default::default()
        });
    let mut managers = BattleManagers::seeded(&initial);
    assert_eq!(managers.attribute.base(10, AttrId::Attack), 1_622);
    assert_eq!(managers.origin_attribute(10, AttrId::Attack), 2_027);
    let changes = managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin: origin(),
            source_uid: 10,
            target_uid: 20,
            buff_id: 31450143,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();
    let added = changes.change.added.expect("Remembrance should be granted");

    assert_eq!(added.buff.act_info[0].param, [0, 8_108]);
    assert!(added.pre_markers.is_empty());
}

#[test]
fn duplicate_remembrance_state_rejects_hp_loss_before_damage() {
    crate::test_support::init_config();
    let mut initial = fight(0);
    let mut duplicate = initial.attacker.as_ref().unwrap().entitys[1].buffs[0].clone();
    duplicate.uid = Some(31);
    initial.attacker.as_mut().unwrap().entitys[1]
        .buffs
        .push(duplicate);
    let mut managers = BattleManagers::seeded(&initial);

    assert_eq!(
        managers.execute_hp(damage(20, 100)),
        Err(crate::engine::manager::hp::HpCommandError::InvalidHpLossState)
    );
    assert_eq!(managers.hp.current(20), 10_903);
}

#[test]
fn invalid_later_remembrance_state_keeps_the_hp_batch_atomic() {
    crate::test_support::init_config();
    let mut initial = fight(0);
    initial
        .attacker
        .as_mut()
        .unwrap()
        .entitys
        .push(FightEntityInfo {
            uid: Some(30),
            team_type: Some(1),
            current_hp: Some(1_000),
            attr: Some(HeroAttribute {
                hp: Some(1_000),
                ..Default::default()
            }),
            buffs: vec![BuffInfo {
                uid: Some(40),
                buff_id: Some(31450143),
                from_uid: Some(10),
                layer: Some(1),
                act_info: vec![BuffActInfo {
                    act_id: Some(1150),
                    param: vec![0],
                    str_param: Some(String::new()),
                }],
                ..Default::default()
            }],
            ..Default::default()
        });
    for (hp, max, params, incoming) in [(1_000, 1_000, vec![0], 100), (1, 0, vec![1, 8_108], 1)] {
        let holder = &mut initial.attacker.as_mut().unwrap().entitys[2];
        holder.current_hp = Some(hp);
        holder.attr.as_mut().unwrap().hp = Some(max);
        holder.buffs[0].act_info[0].param = params;
        let mut managers = BattleManagers::seeded(&initial);

        assert_eq!(
            managers.execute_hp_batch(vec![damage(20, 100), damage(30, incoming)]),
            Err(crate::engine::manager::hp::HpCommandError::InvalidHpLossState)
        );
        assert_eq!(managers.hp.current(20), 10_903);
        assert_eq!(managers.hp.current(30), hp);
        assert_eq!(state(&managers, 20).unwrap().unwrap().current, 0);
    }
}

#[test]
fn remembrance_settles_lethal_hits_at_the_damage_boundary() {
    crate::test_support::init_config();
    for (hp, max, stored, capacity, raw, after_hp, after_stored, spend) in [
        (12_153, 12_153, 8_108, 8_108, 14_959, 1, 8_108, 0),
        (9_716, 11_461, 7_708, 7_708, 17_384, 1, 6_572, 1_136),
        (10_173, 11_461, 6_293, 7_708, 14_587, 1, 7_708, -1_415),
        (9_501, 11_106, 7_708, 7_708, 17_735, 0, 0, 7_708),
        (9_206, 9_206, 0, 7_708, 10_093, 0, 0, 0),
    ] {
        let mut initial = fight(stored);
        let holder = &mut initial.attacker.as_mut().unwrap().entitys[1];
        holder.current_hp = Some(hp);
        holder.attr.as_mut().unwrap().hp = Some(max);
        holder.buffs[0].act_info[0].param[1] = capacity;
        let mut managers = BattleManagers::seeded(&initial);

        let hit = managers.execute_rule_hp(damage(20, raw)).unwrap();
        assert_eq!(managers.hp.current(20), after_hp, "incoming {raw}");
        assert_eq!(hit.changes.caused_death(), after_hp == 0);
        let state = managers.buff.snapshot(20, 30).unwrap();
        assert_eq!(state.act_info[0].param, [after_stored, capacity]);
        assert_eq!(hit.changes.fake_hp_consumed.unwrap().consumed, spend);
        assert_eq!(hit.before_hp_markers.len(), 1);
        assert_eq!(hit.before_hp_markers[0].params, [after_stored, capacity]);
        let change = hit.changes.hp.unwrap();
        assert_eq!(change.hurt.unwrap().display_amount, Some(raw));
        assert_eq!(change.hurt.unwrap().reduce_hp, after_hp - hp);
        assert_eq!(
            change.display_amount,
            Some(if after_hp == 1 { hp - 1 } else { raw })
        );
    }
}
