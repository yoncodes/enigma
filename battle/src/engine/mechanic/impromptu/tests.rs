use super::*;
use crate::engine::manager::BattleManagers;

#[test]
fn active_impromptu_without_team_energy_is_an_error() {
    crate::test_support::init_config();
    let fight = sonettobuf::Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                team_type: Some(1),
                buffs: vec![sonettobuf::BuffInfo {
                    uid: Some(20),
                    buff_id: Some(2_240_000),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };

    assert_eq!(
        allocate_team_energy_rule_ops(
            &BattleManagers::seeded(&fight),
            &SkillEffectCatalog::default(),
            &mut crate::engine::runtime::determinism::RoundDeterminism::default(),
            1,
        ),
        Err(ImpromptuError::MissingTeamEnergy(1))
    );
}

#[test]
fn team_energy_and_inspiration_have_distinct_lifetimes() {
    crate::test_support::init_config();
    let tag = feature(10, 1, 20, "EmitterTag", vec![875]);
    let gain = feature(10, 1, 21, "UseSkillTeamAddEmitterEnergy", vec![881, 1, 2]);
    let mut managers = BattleManagers::default();
    let enable = enable_rule_ops(
        crate::catalog::impromptu_definition(crate::test_support::game_data()),
        &managers.gauge,
        &[tag.clone(), gain.clone()],
        99998,
    )
    .pop()
    .unwrap();
    for output in [enable.team_energy, enable.inspiration] {
        let RuleOp::Command(BattleCommand::Gauge(command)) = output else {
            panic!("gauge command");
        };
        managers.execute_gauge(command).unwrap();
    }

    let RuleOp::Command(BattleCommand::Gauge(command)) =
        team_energy_gain_rule_op(&managers.gauge, &gain, 2).unwrap()
    else {
        panic!("gauge command");
    };
    managers.execute_gauge(command).unwrap();
    let cards = [CardInfo {
        energy: Some(3),
        ..Default::default()
    }];
    let RuleOp::Command(BattleCommand::Gauge(command)) =
        collect_inspiration_rule_op(&managers.gauge, &tag, 99998, &cards).unwrap()
    else {
        panic!("gauge command");
    };
    managers.execute_gauge(command).unwrap();

    let RuleOp::Command(BattleCommand::Gauge(command)) =
        reset_inspiration_rule_op(&managers.gauge, &tag, 99998).unwrap()
    else {
        panic!("gauge command");
    };
    managers.execute_gauge(command).unwrap();

    assert_eq!(managers.gauge.get(team_energy_key(1)).unwrap().current, 2);
    assert_eq!(
        managers.gauge.get(inspiration_key(99998)).unwrap().current,
        0
    );
}

#[test]
fn finalization_snapshots_only_after_inspiration_participates() {
    crate::test_support::init_config();
    let fight = sonettobuf::Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                team_type: Some(1),
                buffs: vec![sonettobuf::BuffInfo {
                    uid: Some(20),
                    buff_id: Some(2_240_000),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let key = inspiration_key(99_998);
    let origin = buff_act::configured_command_origin(875, BuffActKind::EmitterTag).unwrap();
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            key,
            GaugeOperation::Enable { max: None },
        ))
        .unwrap();
    managers.begin_round();

    let idle = finalize_action_queue_rule_ops(&managers, 1, 99_998);
    assert!(matches!(
        idle.as_slice(),
        [
            RuleOp::Command(BattleCommand::Card(CardCommand::ClearEnergy { .. })),
            RuleOp::Command(BattleCommand::Gauge(GaugeCommand {
                operation: GaugeOperation::Snapshot,
                ..
            }))
        ]
    ));

    for delta in [1, -1] {
        managers
            .execute_gauge(GaugeCommand::new(
                origin,
                key,
                GaugeOperation::ChangeValue { delta },
            ))
            .unwrap();
    }
    let participated = finalize_action_queue_rule_ops(&managers, 1, 99_998);
    assert!(matches!(
        participated.as_slice(),
        [
            RuleOp::Command(BattleCommand::Card(CardCommand::ClearEnergy { .. })),
            RuleOp::Command(BattleCommand::Gauge(GaugeCommand {
                operation: GaugeOperation::Snapshot,
                ..
            }))
        ]
    ));
}

#[test]
fn configured_emitter_skill_drives_the_queue_and_plan() {
    crate::test_support::init_config();
    let emitter_uid = crate::engine::manager::emitter::UID;

    for replacement_owner in [None, Some(10)] {
        let managers = impromptu_managers(replacement_owner);
        assert_eq!(queued_skill(&managers), Some(2_240_001));
        assert_eq!(
            build_plan(&managers, 1, emitter_uid).unwrap().skill_id,
            2_240_001
        );
    }

    let managers = impromptu_managers(Some(emitter_uid));
    assert_eq!(queued_skill(&managers), Some(31_411_001));
    assert_eq!(
        build_plan(&managers, 1, emitter_uid).unwrap().skill_id,
        31_411_001
    );
}

#[test]
fn inputs_fail_closed_without_the_exact_feature_or_enabled_gauge() {
    let tag = feature(10, 1, 20, "EmitterTag", vec![875]);
    let wrong = feature(10, 1, 21, "EmitterTag", vec![875]);
    let managers = BattleManagers::default();

    assert!(team_energy_gain_rule_op(&managers.gauge, &wrong, 2).is_none());
    assert!(collect_inspiration_rule_op(&managers.gauge, &tag, 99998, &[]).is_none());
    assert!(reset_inspiration_rule_op(&managers.gauge, &tag, 99998).is_none());
}

fn impromptu_managers(replacement_owner: Option<i64>) -> BattleManagers {
    let emitter_uid = crate::engine::manager::emitter::UID;
    let replacement = || sonettobuf::BuffInfo {
        uid: Some(30),
        buff_id: Some(31_411_402),
        from_uid: Some(3_141),
        ..Default::default()
    };
    let mut hero_buffs = vec![sonettobuf::BuffInfo {
        uid: Some(20),
        buff_id: Some(2_240_000),
        from_uid: Some(10),
        ..Default::default()
    }];
    if replacement_owner == Some(10) {
        hero_buffs.push(replacement());
    }
    let emitter_buffs = (replacement_owner == Some(emitter_uid))
        .then(replacement)
        .into_iter()
        .collect();
    let fight = sonettobuf::Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![
                sonettobuf::FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    team_type: Some(1),
                    buffs: hero_buffs,
                    ..Default::default()
                },
                sonettobuf::FightEntityInfo {
                    uid: Some(emitter_uid),
                    current_hp: Some(100),
                    team_type: Some(1),
                    buffs: emitter_buffs,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let origin = buff_act::configured_command_origin(875, BuffActKind::EmitterTag).unwrap();
    for operation in [
        GaugeOperation::Enable { max: None },
        GaugeOperation::ChangeValue { delta: 1 },
    ] {
        managers
            .execute_gauge(GaugeCommand::new(
                origin,
                inspiration_key(emitter_uid),
                operation,
            ))
            .unwrap();
    }
    managers
}

fn queued_skill(managers: &BattleManagers) -> Option<i32> {
    action_queue_committed_rule_ops(
        managers,
        1,
        crate::engine::manager::emitter::UID,
        &[CardInfo {
            energy: Some(1),
            ..Default::default()
        }],
    )
    .into_iter()
    .find_map(|op| match op {
        RuleOp::Command(BattleCommand::Card(CardCommand::QueueUseCard(queued))) => {
            queued.card.skill_id
        }
        _ => None,
    })
}

fn feature(
    owner_uid: i64,
    team_type: i32,
    buff_uid: i64,
    act_type: &str,
    values: Vec<i32>,
) -> ActiveBuffFeature {
    ActiveBuffFeature {
        owner_uid,
        source_uid: owner_uid,
        buff_uid,
        buff_id: 2240008,
        amount: 1,
        team_type,
        owner_alive: true,
        act_type: act_type.to_owned(),
        effect_time: 0,
        effect_condition: 0,
        raw: values
            .iter()
            .map(i32::to_string)
            .collect::<Vec<_>>()
            .join("#"),
        values,
    }
}
