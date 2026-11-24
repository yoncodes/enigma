use super::*;
use crate::engine::skill::action::{SkillActionEvent, SkillPhase};
use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

#[test]
fn permanent_action_marker_does_not_expire() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(431231),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);

    assert!(
        manager
            .plan_action_settlements(&action(), &ConduitManager::default())
            .is_empty()
    );
}

fn action() -> SkillActionEvent {
    SkillActionEvent {
        source_uid: 10,
        card_index: 0,
        skill_id: 1,
        target_uid: -1,
        target_uids: vec![-1],
        attacked_target_uids: vec![-1],
        phase: SkillPhase::HitPassives,
        skill_slot: 3,
        is_attack: true,
        rank: 1,
        skill_type: 1,
        effect_tag: 1,
        assassinate: false,
        ignore_riposte: false,
        damage_amount: 0,
        kill_count: 0,
        crit_count: 0,
        guard_break_count: 0,
        additional_moxie: 0,
        extra_skill_kind: 0,
        mode: crate::engine::skill::action::SkillExecutionMode::Active,
        teammate_injury_count: 0,
        teammate_injury_count_not_reset: 0,
        team_injury_count_round: 0,
        card_enchants: Vec::new(),
        buff_additions: Vec::new(),
    }
}

#[test]
fn counted_action_settlement_consumes_one_charge() {
    crate::test_support::init_config();
    for (buff_id, count, remaining_counts) in [(30631, 3, &[2, 1, 0][..]), (2130015, 0, &[0][..])] {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(buff_id),
                        from_uid: Some(10),
                        count: Some(count),
                        layer: Some(2),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut manager = BuffManager::default();
        manager.seed(&fight);
        let hp = HpManager::default();
        for &remaining in remaining_counts {
            let settlements =
                manager.plan_action_settlements(&action(), &ConduitManager::default());
            assert_eq!(settlements.len(), 1);
            let result = manager
                .execute(&hp, settlements[0].command.clone())
                .unwrap();
            if remaining > 0 {
                assert_eq!(manager.snapshot(10, 20).unwrap().count, Some(remaining));
            } else {
                assert!(!manager.has_buff_id(10, buff_id));
                assert_eq!(result.change.removed[0].buff.count, Some(0));
            }
        }
    }
}

#[test]
fn next_incantation_might_expires_on_a_cast_without_damage() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(430111),
                    from_uid: Some(10),
                    count: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);
    assert!(
        manager
            .plan_action_settlements(&action(), &ConduitManager::default())
            .is_empty()
    );

    let cast = SkillActionEvent {
        skill_slot: 2,
        is_attack: false,
        ..action()
    };
    let settlements = manager.plan_action_settlements(&cast, &ConduitManager::default());
    let [settlement] = settlements.as_slice() else {
        panic!("expected one cast settlement");
    };
    assert!(settlement.after_hit_reactions);
    manager
        .execute(&HpManager::default(), settlement.command.clone())
        .unwrap();
    assert!(manager.snapshot(10, 20).is_none());
}

#[test]
fn conduit_incantations_spend_the_next_incantation_might() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3144),
                ex_skill_level: Some(1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(430111),
                    from_uid: Some(10),
                    count: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let conduit = ConduitManager::seed(&fight);
    for (skill_id, skill_slot, mode, consumes) in [
        (31441111, -1, SkillExecutionMode::Device, true),
        (31441121, -1, SkillExecutionMode::Device, true),
        (31441131, 3, SkillExecutionMode::Device, false),
        (31446011, -1, SkillExecutionMode::DeviceCard, false),
    ] {
        let cast = SkillActionEvent {
            skill_id,
            skill_slot,
            mode,
            is_attack: skill_id != 31441121,
            ..action()
        };
        let mut manager = BuffManager::default();
        manager.seed(&fight);
        let settlements = manager.plan_action_settlements(&cast, &conduit);
        if consumes {
            let [settlement] = settlements.as_slice() else {
                panic!("expected one cast settlement for {skill_id}");
            };
            assert_eq!(settlement.buff_uid, 20);
            assert!(settlement.after_hit_reactions);
            manager
                .execute(&HpManager::default(), settlement.command.clone())
                .unwrap();
            assert!(manager.snapshot(10, 20).is_none());
            assert!(manager.plan_action_settlements(&cast, &conduit).is_empty());
        } else {
            assert!(settlements.is_empty());
            assert!(manager.snapshot(10, 20).is_some());
        }
    }
}

#[test]
fn attack_drops_only_the_oldest_include_fourteen_copy() {
    crate::test_support::init_config();
    let buff = |uid, buff_id| BuffInfo {
        uid: Some(uid),
        buff_id: Some(buff_id),
        from_uid: Some(-1),
        count: Some(2),
        ..Default::default()
    };
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                buffs: vec![
                    buff(1017, 11410011),
                    buff(1018, 11410011),
                    buff(30, 3120012),
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);
    let hp = HpManager::default();

    let settlements = manager.plan_action_settlements(&action(), &ConduitManager::default());
    assert_eq!(settlements.len(), 1);
    assert_eq!(settlements[0].buff_uid, 1017);
    assert!(settlements[0].in_action);
    let removed = manager
        .execute(&hp, settlements[0].command.clone())
        .unwrap()
        .change
        .removed;

    assert_eq!(removed[0].buff.count, Some(2));
    assert_eq!(removed[0].buff.duration, Some(-1));
    assert_eq!(removed[0].config_effect, 0);
    assert!(manager.snapshot(-1, 1018).is_some());
    assert!(manager.snapshot(-1, 30).is_some());
}

#[test]
fn attack_spends_a_featureless_attacked_marker_in_its_own_frame() {
    crate::test_support::init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1029),
                    buff_id: Some(303020010),
                    from_uid: Some(-1),
                    count: Some(1),
                    duration: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);
    let hp = HpManager::default();

    let settlements = manager.plan_action_settlements(&action(), &ConduitManager::default());
    assert_eq!(settlements.len(), 1);
    assert!(!settlements[0].in_action);
    let removed = manager
        .execute(&hp, settlements[0].command.clone())
        .unwrap()
        .change
        .removed;

    assert_eq!(removed[0].buff.count, Some(0));
    assert!(!manager.has_buff_id(-1, 303020010));
    assert!(
        manager
            .plan_action_settlements(&action(), &ConduitManager::default())
            .is_empty()
    );
}

#[test]
fn attack_leaves_featureless_layer_markers_to_their_own_rule() {
    crate::test_support::init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(40),
                    buff_id: Some(7160019),
                    from_uid: Some(-1),
                    layer: Some(3),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);

    assert!(
        manager
            .plan_action_settlements(&action(), &ConduitManager::default())
            .is_empty()
    );
}

#[test]
fn attack_settles_copies_in_the_attack_target_order() {
    crate::test_support::init_config();
    let copy = |owner: i64, uid| FightEntityInfo {
        uid: Some(owner),
        current_hp: Some(100),
        buffs: vec![BuffInfo {
            uid: Some(uid),
            buff_id: Some(11410011),
            from_uid: Some(owner),
            count: Some(2),
            ..Default::default()
        }],
        ..Default::default()
    };
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![copy(-2, 1023), copy(-1, 1063)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);
    let mut mass = action();
    mass.target_uids = vec![-1, -2];
    mass.attacked_target_uids = vec![-1, -2];

    assert_eq!(
        manager
            .plan_action_settlements(&mass, &ConduitManager::default())
            .iter()
            .map(|settlement| settlement.owner_uid)
            .collect::<Vec<_>>(),
        vec![-1, -2]
    );
}
