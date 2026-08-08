use crate::engine::skill::{
    behavior::classify::BehaviorSpec,
    effect::slot::{ParsedBehavior, ParsedSkillEffect, SkillEffectSlot},
    rule::output::{BattleCommand, RuleOp},
    target::TargetRequest,
};
use sonettobuf::{BuffActInfo, BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;

#[test]
fn intermezzo_restores_and_clears_every_stored_value() {
    crate::test_support::init_config();
    let stored = [(21, 30810301, 945), (22, 30810302, 3_982)];
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    hp: Some(10_000),
                    ..Default::default()
                }),
                buffs: stored
                    .iter()
                    .map(|(uid, buff_id, value)| BuffInfo {
                        uid: Some(*uid),
                        buff_id: Some(*buff_id),
                        from_uid: Some(10),
                        act_info: vec![BuffActInfo {
                            act_id: Some(1062),
                            param: vec![*value],
                            str_param: Some(String::new()),
                        }],
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let origin = crate::engine::skill::rule::CommandOrigin {
        domain: crate::engine::skill::rule::RuleDomain::Behavior,
        key: crate::engine::skill::rule::DefinitionKey::new(60254, "AddHeatScaleFromBuff"),
    };
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            super::super::lingering_glow::key(1),
            GaugeOperation::Enable { max: Some(750) },
        ))
        .unwrap();
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60254, "AddHeatScaleFromBuff"),
        Vec::new(),
        Vec::new(),
    );
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 30810341,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::Buff(BuffCommand::SetInternalState(first))),
            RuleOp::BuffActInfoMarker(first_marker),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::SetInternalState(second))),
            RuleOp::BuffActInfoMarker(second_marker),
            RuleOp::Command(BattleCommand::Gauge(GaugeCommand {
                operation: GaugeOperation::AccumulateRawValue { amount: 4_927, .. },
                raw_delta: Some(4_927),
                ..
            }))
        ] if first.buff_uid == 21
            && first.act_info.as_ref().is_some_and(|info| info[0].param == [0])
            && first_marker.buff_uid == 21
            && first_marker.params == [0]
            && second.buff_uid == 22
            && second.act_info.as_ref().is_some_and(|info| info[0].param == [0])
            && second_marker.buff_uid == 22
            && second_marker.params == [0]
    ));
    for op in ops {
        match op {
            RuleOp::Command(BattleCommand::Buff(command)) => {
                managers.execute_buff(command).unwrap();
            }
            RuleOp::Command(BattleCommand::Gauge(command)) => {
                managers.execute_gauge(command).unwrap();
            }
            _ => {}
        }
    }
    for (buff_uid, _, _) in stored {
        let value = managers
            .buff
            .snapshot(10, buff_uid)
            .and_then(|buff| {
                buff.act_info
                    .into_iter()
                    .find(|info| info.act_id == Some(1062))
            })
            .and_then(|info| info.param.first().copied());
        assert_eq!(value, Some(0));
    }
    assert_eq!(
        managers
            .gauge
            .raw_value(super::super::lingering_glow::key(1)),
        Some(4_927)
    );
}

#[test]
fn intermezzo_preserves_sub_stack_raw_progress() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    hp: Some(10_000),
                    ..Default::default()
                }),
                buffs: vec![
                    BuffInfo {
                        uid: Some(21),
                        buff_id: Some(30810301),
                        from_uid: Some(10),
                        act_info: vec![BuffActInfo {
                            act_id: Some(1062),
                            param: vec![945],
                            str_param: Some(String::new()),
                        }],
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(22),
                        buff_id: Some(31270401),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let origin = crate::engine::skill::rule::CommandOrigin {
        domain: crate::engine::skill::rule::RuleDomain::Behavior,
        key: crate::engine::skill::rule::DefinitionKey::new(60254, "AddHeatScaleFromBuff"),
    };
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            super::super::lingering_glow::key(1),
            GaugeOperation::Enable { max: Some(750) },
        ))
        .unwrap();
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60254, "AddHeatScaleFromBuff"),
        Vec::new(),
        Vec::new(),
    );
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 30810341,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    assert!(ops.iter().any(|op| matches!(
        op,
        RuleOp::Command(BattleCommand::Gauge(GaugeCommand {
            operation: GaugeOperation::AccumulateRawValue { amount: 973, .. },
            raw_delta: Some(973),
            ..
        }))
    )));
    for op in ops {
        match op {
            RuleOp::Command(BattleCommand::Buff(command)) => {
                managers.execute_buff(command).unwrap();
            }
            RuleOp::Command(BattleCommand::Gauge(command)) => {
                managers.execute_gauge(command).unwrap();
            }
            _ => {}
        }
    }
    assert_eq!(
        managers
            .gauge
            .raw_value(super::super::lingering_glow::key(1)),
        Some(973)
    );
    assert_eq!(
        managers
            .gauge
            .get(super::super::lingering_glow::key(1))
            .map(|state| state.current),
        Some(0)
    );
}

#[test]
fn emanation_card_adds_its_configured_lingering_glow() {
    let managers = BattleManagers::default();
    let pool = crate::engine::skill::target::TargetPool::default();
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60246, "HeatScaleUseSkillAddCount"),
        vec![75_000],
        vec!["75000".to_owned()],
    );

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 0,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Gauge(GaugeCommand {
            key,
            operation: GaugeOperation::AccumulateProgress { raw_amount: 75_000 },
            config_effect: 1,
            raw_delta: None,
            ..
        }))] if *key == super::super::lingering_glow::key(1)
    ));
    assert!(crate::engine::skill::behavior::has_destination(&behavior));
}

#[test]
fn green_emanation_upgrades_the_configured_number_of_following_cards() {
    let managers = BattleManagers::default();
    let pool = crate::engine::skill::target::TargetPool::default();
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext {
        active_card_index: 2,
        ..Default::default()
    };
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60247, "AddCardRankNext"),
        vec![3, 1],
        vec!["3".to_owned(), "1".to_owned()],
    );

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 0,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Card(
            CardCommand::RankUpQueued {
                after_card_index: 2,
                count: 3,
                levels: 1,
                ..
            }
        ))]
    ));
}

#[test]
fn creates_heat_scale_from_card_not_cal_size_linked_skill() {
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 31340163,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::from_spec(
                BehaviorSpec::new(60246, "HeatScaleUseSkillAddCount"),
                vec![75000],
                vec!["75000".to_owned()],
            ),
            TargetRequest::self_only(),
        )],
    });
    let card = ActiveBuffFeature {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 31340008,
        amount: 1,
        team_type: 1,
        owner_alive: true,
        act_type: "CardNotCalSize".to_owned(),
        effect_time: 0,
        effect_condition: 0,
        raw: "951#31340161#31340163".to_owned(),
        values: vec![951, 31340161, 31340163],
    };
    let tag = ActiveBuffFeature {
        buff_uid: 21,
        buff_id: 31340003,
        act_type: "HeatScaleTag".to_owned(),
        effect_time: 103,
        raw: "1052".to_owned(),
        values: vec![1052],
        ..card.clone()
    };
    let mut heat_scale = HeatScale::default();

    assert!(
        heat_scale
            .create_from_features(std::slice::from_ref(&card), &catalog)
            .is_empty()
    );
    let creates = heat_scale.create_from_features(&[tag, card], &catalog);

    assert_eq!(
        creates,
        vec![HeatScaleCreate {
            team: 1,
            amount: 750,
            raw_amount: 75000,
            source_buff_id: 31340003,
        }]
    );
    assert!(heat_scale.create_from_features(&[], &catalog).is_empty());
}

#[test]
fn halo_addition_increases_lingering_glow() {
    let mut heat_scale = HeatScale::default();
    assert!(heat_scale.create(HeatScaleCreate {
        team: 1,
        amount: 750,
        raw_amount: 75_000,
        source_buff_id: 31340008,
    }));
    let halo = ActiveBuffFeature {
        owner_uid: -1,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 31340001,
        amount: 1,
        team_type: 2,
        owner_alive: true,
        act_type: "MasterHalo".to_owned(),
        effect_time: 0,
        effect_condition: 0,
        raw: "771#31340001".to_owned(),
        values: vec![771, 31340001],
    };

    let change = heat_scale
        .on_burn_added(
            &[halo],
            BurnOrHaloAdded {
                source_team: 1,
                target_uid: -1,
                buff_uid: 20,
                added_layers: 1,
                alive_enemy_index: 0,
                alive_enemy_count: 3,
            },
        )
        .unwrap();

    assert_eq!(change.amount, 1);
    assert_eq!(change.current, 1);
}

#[test]
fn burn_gain_truncates_each_layer_by_enemy_count() {
    let burn = ActiveBuffFeature {
        owner_uid: -1,
        source_uid: 10,
        buff_uid: 90,
        buff_id: 4150001,
        amount: 2,
        team_type: 2,
        owner_alive: true,
        act_type: "Burn".to_owned(),
        effect_time: 0,
        effect_condition: 0,
        raw: "726#150#102#40".to_owned(),
        values: vec![726, 150, 102, 40],
    };
    let mut earlier_uid = burn.clone();
    earlier_uid.buff_uid = 1;

    let added = |buff_uid| BurnOrHaloAdded {
        source_team: 1,
        target_uid: -1,
        buff_uid,
        added_layers: 2,
        alive_enemy_index: 0,
        alive_enemy_count: 3,
    };
    let late = burn_or_halo_gain(std::slice::from_ref(&burn), added(90)).unwrap();
    let early = burn_or_halo_gain(&[earlier_uid], added(1)).unwrap();

    assert_eq!(late, early);
    assert_eq!(late.raw_amount, 3_332);
}

#[test]
fn vision_reduces_threshold_and_green_crystal_selects_doom_rank_variant() {
    let mut heat_scale = HeatScale::default();
    assert!(heat_scale.create(HeatScaleCreate {
        team: 1,
        amount: 750,
        raw_amount: 75_000,
        source_buff_id: 31340008,
    }));
    heat_scale.apply_value(1, 153, 153_000);
    let mut emanation = crate::engine::manager::emanation::EmanationManager::default();
    assert!(emanation.select(10, 101));

    let cast = heat_scale
        .take_ready_cast(
            &emanation,
            HeatScaleCastRequest {
                owner_uid: 10,
                buff_uid: 2,
                buff_id: 31340005,
                act_id: 1050,
                team: 1,
                raw: "1050#150000#31340151,31340153,31340155,31340157#31340001#10#31340002#-50000",
                has_threshold_buff: true,
            },
        )
        .unwrap();

    assert_eq!(cast.skill_id, 31340153);
    assert_eq!(cast.trigger_value, 153);
    assert_eq!(cast.current, 53);
    assert_eq!(cast.consume_buff_id, Some(31340002));
}
