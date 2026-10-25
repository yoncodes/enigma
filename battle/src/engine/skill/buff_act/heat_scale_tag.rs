use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffCommand, BuffSetState},
    },
    mechanic::lingering_glow,
    skill::{
        buff_act::{BuffActRuleOp, registry::BuffActKind},
        effect::SkillEffectCatalog,
        rule::{
            SetupStage,
            output::{BattleCommand, RuleOp},
        },
        subscriber::BuffActSubscriber,
    },
};

pub fn setup_rule_ops(
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    feature: &ActiveBuffFeature,
    stage: SetupStage,
) -> Option<Vec<RuleOp>> {
    if !super::is_kind(feature, BuffActKind::HeatScaleTag)
        || !super::is_primary_team_feature(managers, feature, BuffActKind::HeatScaleTag)
    {
        return Some(Vec::new());
    }
    match stage {
        SetupStage::BattleStart => Some(
            lingering_glow::enable_rule_ops(
                &managers.gauge,
                &managers.buff.active_features(&managers.hp),
                catalog,
            )
            .into_iter()
            .filter(|enable| enable.create.team == feature.team_type)
            .map(|enable| enable.output)
            .collect(),
        ),
        SetupStage::OpeningBuffGate => {
            Some(lingering_glow::round_start_attribute_rule_ops_for_team(
                managers,
                catalog,
                feature.team_type,
            ))
        }
        SetupStage::RoundTransitionStart => Some(
            lingering_glow::round_transition_attribute_rule_ops_for_team(
                managers,
                catalog,
                feature.team_type,
            ),
        ),
        _ => Some(Vec::new()),
    }
}

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<BuffActRuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::HeatScaleTag)
        || !super::is_primary_team_subscriber(managers, subscriber, BuffActKind::HeatScaleTag)
    {
        return Some(Vec::new());
    }
    if let BattleEvent::GaugeChanged(change) = event {
        let gained_progress = change.progress_raw_delta > 0
            || (change.kind == crate::engine::manager::gauge::GaugeChangeKind::Accumulated
                && change.applied_delta > 0);
        if change.key != lingering_glow::key(subscriber.team_type) || !gained_progress {
            return Some(Vec::new());
        }
        let features = managers.buff.active_features(&managers.hp);
        let Some(counter) =
            lingering_glow::visible_counter_info(&managers.gauge, &features, subscriber.team_type)
        else {
            return Some(Vec::new());
        };
        let Some(origin) = managers
            .catalog()
            .buff_act_origin(counter.act_id, BuffActKind::HeatScaleUseSkill)
        else {
            return Some(Vec::new());
        };
        let Some(mut buff) = managers.buff.snapshot(counter.owner_uid, counter.buff_uid) else {
            return Some(Vec::new());
        };
        let Some(info) = buff
            .act_info
            .iter_mut()
            .find(|info| info.act_id == Some(counter.act_id))
        else {
            return Some(Vec::new());
        };
        info.param = vec![counter.current];
        info.str_param = Some(String::new());
        return Some(vec![
            BuffActRuleOp::causing(RuleOp::Command(BattleCommand::Buff(
                BuffCommand::SetInternalState(BuffSetState {
                    origin,
                    target_uid: counter.owner_uid,
                    buff_uid: counter.buff_uid,
                    ex_info: None,
                    params: None,
                    act_info: Some(buff.act_info),
                }),
            ))),
            BuffActRuleOp::causing(RuleOp::BuffActInfoMarker(
                crate::engine::manager::buff::BuffActInfoMarkerResult {
                    target_uid: counter.owner_uid,
                    buff_uid: counter.buff_uid,
                    act_id: counter.act_id,
                    params: vec![counter.current],
                    str_param: Some(String::new()),
                    team_type: 0,
                },
            )),
        ]);
    }
    let change = match event {
        BattleEvent::BuffAdded(change) | BattleEvent::BuffChanged(change)
            if change.after_amount > change.before_amount =>
        {
            change
        }
        _ => return Some(Vec::new()),
    };
    if managers.buff.team_type(change.source_uid) != Some(subscriber.team_type) {
        return Some(Vec::new());
    }
    let added_layers = change.after_amount - change.before_amount;
    let Some(target_team) = managers.buff.team_type(change.target_uid) else {
        return Some(Vec::new());
    };
    let Some(alive_enemy_count) = enemy_count_for_gain(managers, target_team, change.target_uid)
    else {
        return Some(Vec::new());
    };
    let features = managers.buff.active_features(&managers.hp);
    let ops = lingering_glow::burn_or_halo_rule_op(
        &managers.gauge,
        &features,
        crate::engine::mechanic::heat_scale::BurnOrHaloAdded {
            source_team: subscriber.team_type,
            target_uid: change.target_uid,
            buff_uid: change.buff_uid,
            added_layers,
            alive_enemy_count,
        },
    )
    .and_then(|input| match input.output {
        RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Gauge(command)) => {
            Some(lingering_glow::value_change_rule_ops(managers, command))
        }
        _ => None,
    })
    .unwrap_or_default()
    .into_iter()
    .map(BuffActRuleOp::causing)
    .collect();
    Some(ops)
}

fn enemy_count_for_gain(
    managers: &BattleManagers,
    team_type: i32,
    target_uid: i64,
) -> Option<usize> {
    if !managers.entity.is_main_combatant(target_uid) {
        return None;
    }
    let count = managers
        .entity
        .alive_combatants(team_type, &managers.hp)
        .len();
    (count > 0).then_some(count)
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffActInfo, BuffInfo, Fight, FightEntityInfo, FightTeam};

    use super::*;
    use crate::engine::{
        event::kind::EventKind,
        manager::gauge::{GaugeBatchId, GaugeCommand, GaugeOperation},
        skill::buff_act,
    };

    #[test]
    fn gauge_change_persists_the_visible_accumulation_counter() {
        crate::test_support::init_config();
        let mut managers = BattleManagers::seeded(&Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(100),
                    buffs: vec![
                        BuffInfo {
                            buff_id: Some(31340003),
                            uid: Some(20),
                            from_uid: Some(10),
                            ..Default::default()
                        },
                        BuffInfo {
                            buff_id: Some(31340005),
                            uid: Some(21),
                            from_uid: Some(10),
                            act_info: vec![BuffActInfo {
                                act_id: Some(1050),
                                param: vec![0],
                                str_param: Some(String::new()),
                            }],
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        });
        let tag = managers
            .buff
            .active_features(&managers.hp)
            .into_iter()
            .find(|feature| buff_act::is_kind(feature, BuffActKind::HeatScaleTag))
            .unwrap();
        let origin = buff_act::feature_command_origin(&tag).unwrap();
        let subscriber = buff_act::subscriber_from_feature(tag, EventKind::GaugeChanged).unwrap();
        managers
            .execute_gauge(GaugeCommand::new(
                origin,
                lingering_glow::key(1),
                GaugeOperation::Enable { max: Some(150) },
            ))
            .unwrap();
        let change = managers
            .execute_gauge(GaugeCommand::new(
                origin,
                lingering_glow::key(1),
                GaugeOperation::AccumulateProgress { raw_amount: 40_000 },
            ))
            .unwrap();
        let event = change.events().into_iter().next().unwrap();

        let ops = rule_ops(&managers, &subscriber, &event).unwrap();

        assert!(matches!(
            &ops[0].op,
            RuleOp::Command(BattleCommand::Buff(BuffCommand::SetInternalState(
                BuffSetState { act_info: Some(info), .. }
            ))) if info[0].param == [40]
        ));
        assert!(matches!(
            &ops[1].op,
            RuleOp::BuffActInfoMarker(marker) if marker.act_id == 1050 && marker.params == [40]
        ));
        let RuleOp::Command(BattleCommand::Buff(command)) = &ops[0].op else {
            unreachable!();
        };
        managers.execute_buff(command.clone()).unwrap();
        assert_eq!(
            managers.buff.snapshot(10, 21).unwrap().act_info[0].param,
            [40]
        );
    }

    #[test]
    fn contribution_events_advance_the_visible_counter_in_order() {
        crate::test_support::init_config();
        let mut managers = BattleManagers::seeded(&Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(100),
                    buffs: vec![
                        BuffInfo {
                            buff_id: Some(31340003),
                            uid: Some(20),
                            from_uid: Some(10),
                            ..Default::default()
                        },
                        BuffInfo {
                            buff_id: Some(31340005),
                            uid: Some(21),
                            from_uid: Some(10),
                            act_info: vec![BuffActInfo {
                                act_id: Some(1050),
                                param: vec![86],
                                str_param: Some(String::new()),
                            }],
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        });
        let tag = managers
            .buff
            .active_features(&managers.hp)
            .into_iter()
            .find(|feature| buff_act::is_kind(feature, BuffActKind::HeatScaleTag))
            .unwrap();
        let origin = buff_act::feature_command_origin(&tag).unwrap();
        let subscriber = buff_act::subscriber_from_feature(tag, EventKind::GaugeChanged).unwrap();
        managers
            .execute_gauge(GaugeCommand::new(
                origin,
                lingering_glow::key(1),
                GaugeOperation::Enable { max: Some(150) },
            ))
            .unwrap();
        managers
            .execute_gauge(GaugeCommand::new(
                origin,
                lingering_glow::key(1),
                GaugeOperation::AccumulateRawValue {
                    amount: 86_000,
                    stream: 1062,
                },
            ))
            .unwrap();
        let batch = GaugeBatchId::new(origin, lingering_glow::key(1), 0, 21);
        let first = managers
            .start_raw_gauge_batch(
                batch,
                vec![
                    GaugeCommand::new(
                        origin,
                        lingering_glow::key(1),
                        GaugeOperation::AccumulateRawValue {
                            amount: 10_000,
                            stream: 1062,
                        },
                    ),
                    GaugeCommand::new(
                        origin,
                        lingering_glow::key(1),
                        GaugeOperation::AccumulateRawValue {
                            amount: 7_000,
                            stream: 1062,
                        },
                    ),
                ],
            )
            .unwrap();

        let mut counters = Vec::new();
        let mut change = first;
        for index in 0..2 {
            let event = change.events().into_iter().next().unwrap();
            let ops = rule_ops(&managers, &subscriber, &event).unwrap();
            let RuleOp::Command(BattleCommand::Buff(command)) = &ops[0].op else {
                unreachable!();
            };
            let RuleOp::BuffActInfoMarker(marker) = &ops[1].op else {
                unreachable!();
            };
            counters.push(marker.params[0]);
            managers.execute_buff(command.clone()).unwrap();
            if index == 0 {
                change = managers.continue_raw_gauge_batch(batch).unwrap();
            }
        }
        managers.commit_raw_gauge_batch(batch).unwrap();

        assert_eq!(counters, [96, 103]);
    }

    #[test]
    fn defeated_main_target_uses_the_current_alive_enemy_count() {
        crate::test_support::init_config();
        let managers = BattleManagers::seeded(&Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    position: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![
                    FightEntityInfo {
                        uid: Some(-1),
                        team_type: Some(2),
                        position: Some(1),
                        current_hp: Some(0),
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(-2),
                        team_type: Some(2),
                        position: Some(2),
                        current_hp: Some(0),
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(-3),
                        team_type: Some(2),
                        position: Some(3),
                        current_hp: Some(100),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        });

        assert_eq!(enemy_count_for_gain(&managers, 2, -1), Some(1));
    }
}
