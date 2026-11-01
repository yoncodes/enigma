use super::*;

#[test]
fn reconnect_snapshot_includes_current_field_and_clears_removed_field() {
    use crate::engine::manager::field::{FieldCommand, FieldDefinition, FieldOperation, FieldThreshold};
    use crate::engine::skill::rule::{CommandOrigin, DefinitionKey, RuleDomain};
    crate::test_support::init_config();
    let mut runtime = runtime(Fight::default());
    let origin = CommandOrigin { domain: RuleDomain::Behavior, key: DefinitionKey::new(50019, "AddMagicCircle") };
    let thresholds = vec![
        FieldThreshold { level: 1, progress: 0, definition: FieldDefinition { field_id: 30001, duration: 3 } },
        FieldThreshold { level: 2, progress: 50, definition: FieldDefinition { field_id: 30002, duration: 3 } },
        FieldThreshold { level: 3, progress: 120, definition: FieldDefinition { field_id: 30003, duration: 2 } },
    ];
    runtime.managers.execute_field(FieldCommand { origin, team: 1,
        operation: FieldOperation::DeployIfAbsent { definition: thresholds[0].definition,
            create_uid: 10, initial_level: 1, thresholds: thresholds.clone() } }).unwrap();
    runtime.managers.execute_field(FieldCommand { origin, team: 1,
        operation: FieldOperation::ChangeProgress { delta: 60 } }).unwrap();
    runtime.managers.execute_field(FieldCommand { origin, team: 1,
        operation: FieldOperation::ResolveLevel { thresholds } }).unwrap();
    let snapshot = runtime.reconnect_state().0.magic_circle.unwrap();
    assert_eq!(snapshot.magic_circle_id, Some(30002));
    assert_eq!(snapshot.create_uid, Some(10));
    assert_eq!(snapshot.round, Some(3));
    assert_eq!(snapshot.electric_level, Some(2));
    assert_eq!(snapshot.electric_progress, Some(60));
    assert_eq!(snapshot.max_electric_progress, Some(120));
    runtime.managers.execute_field(FieldCommand { origin, team: 1,
        operation: FieldOperation::Remove }).unwrap();
    assert!(runtime.reconnect_state().0.magic_circle.is_none());
}

#[test]
fn entity_info_projects_manager_owned_state() {
    use crate::engine::{
        manager::hp::{CurrentHpSet, HpCommand},
        skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
    };

    crate::test_support::init_config();
    let mut runtime = runtime(Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3127),
                current_hp: Some(100),
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            sp_entitys: vec![FightEntityInfo {
                uid: Some(-6),
                model_id: Some(900030304),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    runtime
        .managers
        .execute_hp(HpCommand::SetCurrent(CurrentHpSet {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "Test"),
            },
            source_uid: 10,
            target_uid: 10,
            value: 70,
            config_effect: 0,
            effect_type: 1,
        }))
        .unwrap();

    let entity = runtime.entity_info(10).unwrap();
    assert_eq!(entity.model_id, Some(3127));
    assert_eq!(entity.current_hp, Some(70));
    assert_eq!(runtime.entity_info(-6).unwrap().model_id, Some(900030304));
    assert!(runtime.entity_info(11).is_none());
}

#[test]
fn hero_special_attributes_do_not_depend_on_record_mode() {
    crate::test_support::init_config();
    let mut runtime = runtime(Fight {
        version: Some(7),
        is_record: Some(true),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(1),
                team_type: Some(2),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    runtime.fight.is_record = Some(false);

    assert_eq!(
        runtime.managers.hero_sp_attributes(&runtime.fight)[0]
            .attribute
            .as_ref()
            .and_then(|attribute| attribute.device_skill_rate),
        Some(0)
    );
}

#[test]
fn third_wave_entity_info_uses_the_latest_authoritative_roster() {
    crate::test_support::init_config();
    let (entitys, sub_entitys) =
        crate::engine::fight::defender::Defender::build_wave_entities(161301, 3, 2, 0).unwrap();
    let mut runtime = runtime(Fight {
        battle_id: Some(1613),
        episode_id: Some(10624),
        version: Some(7),
        cur_wave: Some(1),
        is_record: Some(true),
        defender: Some(FightTeam {
            entitys,
            sub_entitys,
            ..Default::default()
        }),
        ..Default::default()
    });

    runtime
        .managers
        .advance_wave(&mut runtime.fight)
        .unwrap()
        .unwrap();
    let third = runtime
        .managers
        .advance_wave(&mut runtime.fight)
        .unwrap()
        .unwrap();
    let uid = third.entering_uids[0];
    let expected = third
        .fight
        .defender
        .as_ref()
        .unwrap()
        .entitys
        .iter()
        .find(|entity| entity.uid == Some(uid))
        .unwrap();

    assert_eq!(third.wave, 3);
    assert_eq!(third.fight.is_record, Some(false));
    assert!(expected.current_hp.unwrap_or_default() > 0);
    assert_eq!(runtime.entity_info(uid).as_ref(), Some(expected));
    assert_eq!(runtime.reconnect_state().0.cur_wave, Some(3));
}

#[test]
fn refill_and_all_compositions_grant_cloth_power() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            cloth_id: Some(1),
            ..Default::default()
        }),
        ..Default::default()
    };
    let power = crate::catalog::BattleCatalog::new(crate::test_support::game_data())
        .cloth_power(&fight)
        .unwrap();
    assert_eq!(
        power,
        crate::engine::round::power::ClothPower::for_fight(
            crate::test_support::game_data(),
            &fight,
        )
        .unwrap()
    );

    assert_eq!(
        round::cloth_power_after_card_change(
            &power,
            15,
            crate::engine::manager::card::CardChangeKind::Refilled,
            1,
        ),
        17
    );
    assert_eq!(
        round::cloth_power_after_card_change(
            &power,
            15,
            crate::engine::manager::card::CardChangeKind::Composed,
            1,
        ),
        17
    );
    assert_eq!(
        round::cloth_power_after_card_change(
            &power,
            15,
            crate::engine::manager::card::CardChangeKind::Played,
            0,
        ),
        15
    );
}

#[test]
fn played_cards_grant_cloth_power_only_after_action_completion() {
    use crate::engine::{
        manager::card::{CardChangeKind, CardChanges, PlayedCard},
        round::state::RoundState,
        runtime::{drain::DrainResult, executor::RuleOutcome},
        skill::action::{ActionEvent, SkillLifecycle},
    };

    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            cloth_id: Some(1),
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = crate::engine::manager::BattleManagers::seeded(&fight);
    let played = |card_index| {
        RuleOutcome::Card(Box::new(CardChanges {
            origin: None,
            kind: CardChangeKind::Played,
            before: Vec::new(),
            after: Vec::new(),
            added: None,
            played: Some(PlayedCard {
                card: Default::default(),
                caster_uid: 10,
                card_index,
                skill_id: 100,
                rank_change_pending: false,
                rewritten: false,
                target_uid: Some(-1),
                recorded_skill: None,
            }),
            drawn: Vec::new(),
            composed_owners: Vec::new(),
            operation: None,
            action_queue: None,
            queued_use_card: None,
            rank_results: Vec::new(),
            consumed_indices: Vec::new(),
            expired_indices: Vec::new(),
            consumed_owner_uid: None,
            consumption_kind: None,
            entity: None,
            hand_limit: None,
            owner_removal: None,
            ai_invalidations: Vec::new(),
            ai_queue: None,
            team_cards: None,
        }))
    };

    let mut state = RoundState {
        power: 15,
        ..Default::default()
    };
    round::apply_cloth_power(
        &fight,
        &managers,
        &mut state,
        &DrainResult {
            outcomes: vec![played(1)],
            ..Default::default()
        },
    );
    assert_eq!(state.power, 15);

    round::apply_cloth_power(
        &fight,
        &managers,
        &mut state,
        &DrainResult {
            outcomes: vec![
                played(1),
                RuleOutcome::SkillLifecycle(SkillLifecycle::ActionCompleted(ActionEvent {
                    card_index: 1,
                    ..Default::default()
                })),
            ],
            ..Default::default()
        },
    );
    assert_eq!(state.power, 19);

    state.power = 15;
    round::apply_cloth_power(
        &fight,
        &managers,
        &mut state,
        &DrainResult {
            outcomes: vec![
                RuleOutcome::SkillLifecycle(SkillLifecycle::ActionCompleted(ActionEvent {
                    card_index: 1,
                    ..Default::default()
                })),
                played(1),
            ],
            ..Default::default()
        },
    );
    assert_eq!(state.power, 15);

    // Atonal Sequence gains its configured device-card reward, only on completion.
    let energy_fight = Fight { attacker: Some(FightTeam { cloth_id: Some(7), ..Default::default() }), ..Default::default() };
    let mut energy_play = played(2);
    if let RuleOutcome::Card(changes) = &mut energy_play {
        changes.played.as_mut().unwrap().card.skill_id = Some(31446011);
    }
    state.power = 20;
    round::apply_cloth_power(&energy_fight, &managers, &mut state, &DrainResult {
        outcomes: vec![energy_play, RuleOutcome::SkillLifecycle(SkillLifecycle::ActionCompleted(ActionEvent {
            card_index: 2, ..Default::default()
        }))], ..Default::default()
    });
    assert_eq!(state.power, 25);
}

#[test]
fn cloth_composition_uses_the_owners_resource_type() {
    let mut managers = crate::engine::manager::BattleManagers::default();
    managers.ex_point.register(&FightEntityInfo {
        uid: Some(1),
        ex_point_type: Some(crate::engine::manager::ex_point::ExPointKind::Common.as_wire()),
        ..Default::default()
    });
    managers.ex_point.register(&FightEntityInfo {
        uid: Some(2),
        ex_point_type: Some(crate::engine::manager::ex_point::ExPointKind::Adrenaline.as_wire()),
        ..Default::default()
    });

    assert_eq!(round::eligible_composition_count(&managers, &[1, 2]), 1);
}

#[test]
fn end_fight_statistics_project_owned_runtime_history() {
    use crate::engine::{
        manager::{
            buff::{BuffCommand, BuffGrant},
            hp::{DamageEffectKind, HpCommand, HpDamage, HpHeal, HpHealKind, HurtInfoData},
        },
        skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
    };

    crate::test_support::init_config();
    let mut runtime = runtime(Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(50),
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
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
    });
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "Test"),
    };
    runtime
        .managers
        .execute_hp(HpCommand::Damage(HpDamage {
            origin,
            source_uid: 10,
            target_uid: -1,
            amount: 30,
            config_effect: 1,
            effect_kind: DamageEffectKind::Normal,
            assassinate: false,
            ignore_riposte: false,
            hurt: HurtInfoData {
                from_uid: 10,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 1,
                skill_id: 101,
                damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: 0,
                display_amount: None,
            },
        }))
        .unwrap();
    runtime
        .managers
        .execute_hp(HpCommand::Heal(HpHeal {
            origin,
            source_uid: 10,
            target_uid: 10,
            amount: 20,
            config_effect: 1,
            kind: HpHealKind::Normal,
        }))
        .unwrap();
    runtime.managers.card.reset(
        vec![CardInfo {
            uid: Some(10),
            skill_id: Some(101),
            ..Default::default()
        }],
        0,
    );
    runtime.managers.card.play_card(0, Some(-1), None, None);
    runtime
        .managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: 10,
            target_uid: 10,
            buff_id: 70015,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();

    let stats = runtime.attack_statistics();
    assert_eq!(stats.len(), 1);
    assert_eq!(
        (stats[0].harm, stats[0].hurt, stats[0].heal),
        (Some(30), Some(0), Some(20))
    );
    assert_eq!(stats[0].cards[0].skill_id, Some(101));
    assert_eq!(stats[0].cards[0].use_count, Some(1));
    assert_eq!(stats[0].get_buffs, vec![70015]);
}


#[test]
fn reconnect_snapshot_restores_emitter_buffs_and_both_inspiration_gauges() {
    use crate::engine::manager::gauge::{GaugeCommand, GaugeOperation};
    use crate::engine::manager::buff::{BuffCommand, BuffGrantChild};
    use crate::engine::skill::rule::{CommandOrigin, DefinitionKey, RuleDomain};
    use crate::engine::mechanic::impromptu::{inspiration_key, team_energy_key};
    crate::test_support::init_config();
    let mut runtime = runtime(Fight { attacker: Some(FightTeam::default()), ..Default::default() });
    let uid = crate::engine::manager::emitter::UID;
    let origin = CommandOrigin { domain: RuleDomain::Lifecycle, key: DefinitionKey::new(0, "ReconnectEmitterTest") };
    runtime.managers.hp.register(&crate::engine::manager::emitter::activation_entity());
    runtime.managers.execute_buff(BuffCommand::GrantInternalChild(BuffGrantChild {
        origin, source_uid: 10, target_uid: uid, buff_id: 31410410, amount: None,
        params: None, act_info: None,
    })).unwrap();
    for (key, amount) in [(inspiration_key(uid), 7), (team_energy_key(1), 8)] {
        runtime.managers.execute_gauge(GaugeCommand::new(origin, key, GaugeOperation::Enable { max: None })).unwrap();
        runtime.managers.execute_gauge(GaugeCommand::new(origin, key, GaugeOperation::ChangeValue { delta: amount })).unwrap();
    }
    let team = runtime.reconnect_state().0.attacker.unwrap();
    assert_eq!(team.emitter_info.unwrap().energy, Some(7));
    assert_eq!(team.energy, Some(8));
    assert!(team.emitter.unwrap().buffs.iter().any(|buff| buff.buff_id == Some(31410410)));
}
