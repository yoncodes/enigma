use super::*;
use sonettobuf::{
    BuffInfo, Fight, FightEntityInfo, FightExPointInfo, FightStep, FightTeam, PowerInfo,
};

fn runtime() -> BattleRuntime {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ex_point: Some(1),
                power_infos: vec![PowerInfo {
                    power_id: Some(1),
                    num: Some(5),
                    max: Some(8),
                }],
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
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(100001),
                    buff_id: Some(109320109),
                    layer: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    BattleRuntime {
        managers: crate::engine::manager::BattleManagers::seeded(&fight),
        fight,
        ..Default::default()
    }
}

fn card(uid: i64, skill_id: i32) -> CardInfo {
    CardInfo {
        uid: Some(uid),
        skill_id: Some(skill_id),
        ..Default::default()
    }
}

#[test]
fn copies_captured_hp_ex_point_and_last_published_hand() {
    let mut runtime = runtime();
    let captured = FightRound {
        ex_point_info: vec![FightExPointInfo {
            uid: Some(10),
            ex_point: Some(4),
            power_infos: vec![PowerInfo {
                power_id: Some(1),
                num: Some(0),
                max: Some(8),
            }],
            current_hp: Some(60),
            ..Default::default()
        }],
        next_round_begin_step: vec![FightStep {
            act_effect: vec![
                sonettobuf::ActEffect {
                    effect_type: Some(EffectType::Cardspush as i32),
                    card_info_list: vec![card(10, 1)],
                    ..Default::default()
                },
                sonettobuf::ActEffect {
                    card_info_list: vec![card(10, 9)],
                    ..Default::default()
                },
                sonettobuf::ActEffect {
                    fight_step: Some(FightStep {
                        act_effect: vec![sonettobuf::ActEffect {
                            effect_type: Some(EffectType::Cardspush as i32),
                            card_info_list: vec![card(10, 1), card(10, 2)],
                            ..Default::default()
                        }],
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };

    let changes = runtime.resync_observed_state(&captured).unwrap();

    assert_eq!(changes.len(), 4);
    assert_eq!(runtime.managers.hp.current(10), 60);
    assert_eq!(runtime.managers.ex_point.get(10), 4);
    assert_eq!(runtime.managers.eureka.get(10, 1).current, 0);
    assert_eq!(skill_ids(runtime.card_hand()), vec![1, 2]);
}

#[test]
fn refuses_to_revive_or_continue_a_finished_battle() {
    let mut runtime = runtime();
    let dead = FightRound {
        ex_point_info: vec![FightExPointInfo {
            uid: Some(10),
            current_hp: Some(0),
            ..Default::default()
        }],
        ..Default::default()
    };
    assert!(runtime.resync_observed_state(&dead).is_err());
    assert_eq!(runtime.managers.hp.current(10), 100);

    runtime.round_state.is_finish = true;
    assert!(
        runtime
            .resync_observed_state(&FightRound::default())
            .is_err()
    );
}

#[test]
fn folds_captured_buff_changes_into_manager_owned_state() {
    let mut runtime = runtime();
    let observed = |effect_type, source_uid, target_uid, uid, layer| ActEffect {
        target_id: Some(target_uid),
        effect_type: Some(effect_type as i32),
        buff: Some(BuffInfo {
            uid: Some(uid),
            buff_id: Some(109320109),
            from_uid: Some(source_uid),
            layer: Some(layer),
            ..Default::default()
        }),
        ..Default::default()
    };
    let captured = FightRound {
        fight_step: vec![FightStep {
            act_effect: vec![
                observed(EffectType::Buffdel, -1, -1, 100001, 0),
                observed(EffectType::Buffadd, -1, -1, 100003, 1),
                observed(EffectType::Buffadd, 10, 10, 4, 1),
                observed(EffectType::Buffadd, -1, 10, 6, 3),
                observed(EffectType::Buffadd, 10, -1, 100005, 1),
                ActEffect {
                    fight_step: Some(FightStep {
                        act_effect: vec![observed(EffectType::Buffupdate, -1, -1, 100003, 2)],
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };

    runtime.resync_observed_state(&captured).unwrap();

    assert!(runtime.managers.buff.snapshot(-1, 100001).is_none());
    assert!(runtime.managers.buff.snapshot(10, 4).is_none());
    assert_eq!(
        runtime.managers.buff.snapshot(10, 6).unwrap().layer,
        Some(3)
    );
    assert!(runtime.managers.buff.snapshot(-1, 100005).is_none());
    assert_eq!(
        runtime.managers.buff.snapshot(-1, 100003).unwrap().layer,
        Some(2)
    );
    assert_eq!(
        runtime.fight.defender.as_ref().unwrap().entitys[0]
            .buffs
            .iter()
            .find(|buff| buff.uid == Some(100003))
            .and_then(|buff| buff.layer),
        Some(2)
    );
}
