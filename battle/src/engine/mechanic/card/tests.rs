mod cost;
mod ultimate;

use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;

fn fight(ex_point: i32) -> Fight {
    Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3139),
                current_hp: Some(100),
                ex_point: Some(ex_point),
                ex_skill: Some(31390131),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31390181),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[test]
fn card_not_cal_size_adds_a_ready_ultimate_beyond_the_normal_hand() {
    crate::test_support::init_config();
    let mut fight = fight(5);
    fight
        .attacker
        .as_mut()
        .unwrap()
        .entitys
        .push(FightEntityInfo {
            uid: Some(11),
            model_id: Some(3134),
            current_hp: Some(100),
            ex_point: Some(5),
            ex_skill: Some(31340131),
            buffs: vec![BuffInfo {
                uid: Some(21),
                buff_id: Some(31390183),
                from_uid: Some(10),
                ..Default::default()
            }],
            ..Default::default()
        });
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let hand = (0..9)
        .map(|skill_id| CardInfo {
            uid: Some(10),
            skill_id: Some(skill_id + 1),
            ..Default::default()
        })
        .collect::<Vec<_>>();

    let extra = CardMechanic.special_team_cards(&pool, &managers, &hand);

    assert_eq!(hand.len() + extra.len(), 11);
    assert_eq!(
        extra
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![31390131, 31340131]
    );
}

#[test]
fn entity_ultimate_not_cal_size_is_scoped_to_its_buff_owner() {
    crate::test_support::init_config();
    let mut fight = fight(5);
    fight.attacker.as_mut().unwrap().entitys[0].buffs = vec![BuffInfo {
        uid: Some(20),
        buff_id: Some(31390190),
        from_uid: Some(10),
        ..Default::default()
    }];
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mechanic = CardMechanic;

    assert!(mechanic.ultimate_ignores_limit(&managers, 10, 31390131));
    assert!(!mechanic.ultimate_ignores_limit(&managers, 11, 31340131));
    assert!(mechanic.counts_toward_hand_limit(
        &CardInfo {
            uid: Some(10),
            skill_id: Some(1),
            ..Default::default()
        },
        &managers,
        &pool,
    ));
}

#[test]
fn card_not_cal_size_excludes_its_configured_non_ultimate() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3156),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31560140),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);

    assert!(!CardMechanic.counts_toward_hand_limit(
        &CardInfo {
            uid: Some(10),
            skill_id: Some(31560181),
            ..Default::default()
        },
        &managers,
        &pool,
    ));
}

#[test]
fn card_not_cal_size_does_not_duplicate_or_unlock_an_ultimate() {
    crate::test_support::init_config();
    let locked = fight(4);
    let locked_pool = TargetPool::from_fight(&locked);
    assert!(
        CardMechanic
            .special_team_cards(&locked_pool, &BattleManagers::seeded(&locked), &[])
            .is_empty()
    );

    let ready = fight(5);
    let ready_pool = TargetPool::from_fight(&ready);
    let ultimate = CardInfo {
        uid: Some(10),
        skill_id: Some(31390131),
        ..Default::default()
    };
    assert!(
        CardMechanic
            .special_team_cards(&ready_pool, &BattleManagers::seeded(&ready), &[ultimate],)
            .is_empty()
    );
}

#[test]
fn configured_ultimate_rank_alias_blocks_a_duplicate() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3134),
                current_hp: Some(100),
                ex_point: Some(5),
                ex_skill: Some(31345131),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::default();
    let card = CardInfo {
        uid: Some(10),
        skill_id: Some(31340131),
        ..Default::default()
    };

    assert!(CardMechanic.is_ultimate(&managers, &card, &pool.attacker_main[0]));
    assert!(
        CardMechanic
            .special_team_cards(&pool, &BattleManagers::seeded(&fight), &[card])
            .is_empty()
    );
}

#[test]
fn configured_synchronization_chain_is_the_owners_ultimate() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3123),
                current_hp: Some(100),
                ex_point_type: Some(ExPointKind::Synchronization.as_wire()),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_ex_point(
            crate::engine::manager::ex_point::ExPointCommand::ConfigureSynchronization(
                crate::engine::manager::ex_point::ExPointConfigureSynchronization {
                    origin: crate::engine::skill::rule::CommandOrigin {
                        domain: crate::engine::skill::rule::RuleDomain::Behavior,
                        key: crate::engine::skill::rule::DefinitionKey::new(100000, "EzioProps"),
                    },
                    target_uid: 10,
                    definition: crate::engine::manager::ex_point::SynchronizationDefinition::new(
                        [312301323, 312301333, 312301343],
                        4,
                        100,
                    )
                    .unwrap(),
                },
            ),
        )
        .unwrap();

    for skill_id in [312301323, 312301333, 312301343] {
        assert!(CardMechanic.is_ultimate_skill(&managers, skill_id, &pool.attacker_main[0]));
    }
}

#[test]
fn faith_channel_requires_a_full_gauge() {
    crate::test_support::init_config();
    let ultimate_cards = |ex_skill, ex_point, ex_point_max| {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3120),
                    current_hp: Some(100),
                    ex_point: Some(ex_point),
                    ex_point_type: Some(ExPointKind::Faith.as_wire()),
                    ex_point_max: Some(ex_point_max),
                    ex_skill: Some(ex_skill),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let managers = BattleManagers::seeded(&fight);

        CardMechanic
            .normal_ultimate_cards(&pool, &managers)
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>()
    };

    assert!(ultimate_cards(31200133, 7, 8).is_empty());
    assert_eq!(ultimate_cards(31200133, 8, 8), vec![31200133]);
    assert!(ultimate_cards(31200133, 5, 6).is_empty());
    assert_eq!(ultimate_cards(31200133, 6, 6), vec![31200133]);
    assert!(ultimate_cards(31201132, 2, 8).is_empty());
    assert_eq!(ultimate_cards(31201132, 8, 8), vec![31201132]);
}

#[test]
fn active_ultimate_cost_buff_controls_card_readiness() {
    crate::test_support::init_config();
    let mut fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(222001),
                current_hp: Some(100),
                ex_point: Some(1),
                ex_skill: Some(222001231),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(2220012),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);

    assert_eq!(
        CardMechanic.required_ultimate_cost(&managers, &pool.attacker_main[0]),
        1
    );
    assert_eq!(
        CardMechanic
            .normal_ultimate_cards(&pool, &managers)
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![222001231]
    );
    managers.sync_entities(&mut fight);
    assert_eq!(
        fight.attacker.unwrap().entitys[0].ex_skill_point_change,
        Some(-4)
    );
}

#[test]
fn configured_buff_acts_adjust_the_normal_hand_limit() {
    crate::test_support::init_config();
    let fight_with = |buff_id| Fight {
        attacker: Some(FightTeam {
            entitys: (0..4)
                .map(|index| FightEntityInfo {
                    uid: Some(index + 1),
                    model_id: Some(1000 + index as i32),
                    current_hp: Some(100),
                    buffs: (index == 0)
                        .then(|| BuffInfo {
                            uid: Some(20),
                            buff_id: Some(buff_id),
                            from_uid: Some(1),
                            ..Default::default()
                        })
                        .into_iter()
                        .collect(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };

    for (buff_id, expected) in [(31490001, 10), (31370131, 6)] {
        let fight = fight_with(buff_id);
        let managers = BattleManagers::seeded(&fight);
        let pool = TargetPool::from_fight(&fight);

        assert_eq!(
            CardMechanic.normal_hand_limit(8, &managers, &pool),
            expected
        );
    }
}
