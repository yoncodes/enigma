use super::*;

#[test]
fn team_injury_change_projects_each_committed_counter_value() {
    let effects = project_change_for_test(&BattleChange::Injury(InjuryChange {
        origin: CommandOrigin {
            domain: RuleDomain::Skill,
            key: DefinitionKey::new(1, "TeamInjury"),
        },
        source_uid: 10,
        team_type: 1,
        counter_owner_uid: 11,
        before: 2,
        after: 4,
    }))
    .unwrap();

    assert_eq!(effects.len(), 2);
    assert!(effects.iter().all(|effect| {
        effect.target_id == Some(11)
            && effect.effect_type == Some(EffectType::Fightcounter as i32)
            && effect.effect_num
                == Some(crate::engine::manager::injury::InjuryCounterKind::TeamInjury.id())
            && effect.team_type == Some(1)
    }));
    assert_eq!(
        effects
            .iter()
            .map(|effect| effect.config_effect)
            .collect::<Vec<_>>(),
        vec![Some(3), Some(4)]
    );
}

#[test]
fn explicit_conduit_counter_change_projects_the_committed_absolute_value() {
    let effects = project_change_for_test(&BattleChange::Conduit(
        crate::engine::manager::conduit::ConduitChange::CounterChanged {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60297, "AddDeviceCounter"),
            },
            source_uid: -2,
            team: 1,
            kind: crate::engine::manager::conduit::ConduitCounterKind::Activation,
            requested_delta: 2,
            applied_delta: 2,
            after: 2,
        },
    ))
    .unwrap();

    let [effect] = effects.as_slice() else {
        panic!("expected one counter-change effect");
    };
    assert_eq!(effect.target_id, Some(-2));
    assert_eq!(effect.effect_type, Some(EffectType::Counterchange as i32));
    assert_eq!(effect.effect_num, Some(63));
    assert_eq!(effect.config_effect, Some(0));
    assert_eq!(effect.buff_act_id, Some(0));
    assert_eq!(effect.reserve_id, Some(0));
    assert_eq!(effect.reserve_str.as_deref(), Some("2"));
    assert_eq!(effect.team_type, Some(1));
    assert_eq!(effect.effect_num1, Some(0));
}

#[test]
fn zero_cost_conduit_activation_projects_zero_markers() {
    let frame = SemanticFrame {
        owner: FrameOwner::ConduitAction {
            source_uid: 10,
            group: 1,
            skill_position: 1,
            target_uid: Some(-1),
        },
        trigger: crate::engine::runtime::record::FrameTrigger::Active,
        items: vec![
            FrameItem::Change(Box::new(BattleChange::Conduit(
                crate::engine::manager::conduit::ConduitChange::SkillBegan {
                    source_uid: 10,
                    team: 1,
                    skill_id: 31490111,
                    power_id: 1,
                    activation_cost: 0,
                    spent: 0,
                },
            ))),
            FrameItem::Change(Box::new(BattleChange::Conduit(
                crate::engine::manager::conduit::ConduitChange::SkillCostCommitted {
                    source_uid: 10,
                    team: 1,
                    skill_id: 31490111,
                    power_id: 1,
                    activation_cost: 0,
                    consumed_this_round: 0,
                },
            ))),
            FrameItem::Child(Box::new(SemanticFrame {
                owner: FrameOwner::ConduitSkill {
                    source_uid: 10,
                    skill_id: 31490111,
                    card_index: 1,
                    target_uid: Some(-1),
                },
                trigger: crate::engine::runtime::record::FrameTrigger::Active,
                items: vec![FrameItem::Change(Box::new(BattleChange::Conduit(
                    crate::engine::manager::conduit::ConduitChange::SkillFinished {
                        source_uid: 10,
                        team: 1,
                        skill_id: 31490111,
                        uses_this_round: 1,
                    },
                )))],
            })),
        ],
    };

    let steps = project(&[frame]).unwrap();
    assert_eq!(steps.len(), 1);
    let effects = &steps[0].act_effect;
    assert_eq!(
        effects
            .iter()
            .map(|effect| effect.effect_type)
            .collect::<Vec<_>>(),
        vec![
            Some(EffectType::Devicepowerchange as i32),
            Some(EffectType::Counterchange as i32),
            Some(EffectType::Fightstep as i32),
        ]
    );

    let [power_change, counter_change, child] = effects.as_slice() else {
        panic!("expected zero-cost conduit markers before the child skill frame");
    };
    assert_eq!(power_change.target_id, Some(0));
    assert_eq!(power_change.effect_num, Some(-1));
    assert_eq!(power_change.config_effect, Some(0));
    assert_eq!(power_change.buff_act_id, Some(0));
    assert_eq!(power_change.reserve_id, Some(0));
    assert_eq!(power_change.reserve_str.as_deref(), Some("1#0"));
    assert_eq!(power_change.team_type, Some(1));
    assert_eq!(power_change.effect_num1, Some(0));

    assert_eq!(counter_change.target_id, Some(10));
    assert_eq!(counter_change.effect_num, Some(62));
    assert_eq!(counter_change.config_effect, Some(0));
    assert_eq!(counter_change.buff_act_id, Some(0));
    assert_eq!(counter_change.reserve_id, Some(0));
    assert_eq!(counter_change.reserve_str.as_deref(), Some("0"));
    assert_eq!(counter_change.team_type, Some(1));
    assert_eq!(counter_change.effect_num1, Some(0));
    assert_eq!(
        child.fight_step.as_ref().and_then(|step| step.act_type),
        Some(sonettobuf::fight_step::ActType::Device as i32)
    );
    assert_eq!(
        child.fight_step.as_ref().and_then(|step| step.from_id),
        Some(10)
    );
    assert_eq!(
        child.fight_step.as_ref().and_then(|step| step.to_id),
        Some(-1)
    );
    assert_eq!(
        child.fight_step.as_ref().and_then(|step| step.act_id),
        Some(31490111)
    );
    assert_eq!(
        child.fight_step.as_ref().and_then(|step| step.card_index),
        Some(1)
    );
}

#[test]
fn positive_cost_conduit_activation_keeps_markers() {
    let effects = [
        crate::engine::manager::conduit::ConduitChange::SkillBegan {
            source_uid: 10,
            team: 1,
            skill_id: 31490121,
            power_id: 1,
            activation_cost: 3,
            spent: 3,
        },
        crate::engine::manager::conduit::ConduitChange::SkillCostCommitted {
            source_uid: 10,
            team: 1,
            skill_id: 31490121,
            power_id: 1,
            activation_cost: 3,
            consumed_this_round: 3,
        },
    ]
    .into_iter()
    .flat_map(|change| project_change_for_test(&BattleChange::Conduit(change)).unwrap())
    .collect::<Vec<_>>();

    assert_eq!(effects.len(), 2);
    assert_eq!(effects[0].reserve_str.as_deref(), Some("1#-3"));
    assert_eq!(effects[1].effect_num, Some(62));
    assert_eq!(effects[1].reserve_str.as_deref(), Some("3"));
}

#[test]
fn unique_conduit_activation_has_no_cost_projection() {
    for change in [
        crate::engine::manager::conduit::ConduitChange::SkillBegan {
            source_uid: 10,
            team: 1,
            skill_id: 31490151,
            power_id: 999,
            activation_cost: 0,
            spent: 0,
        },
        crate::engine::manager::conduit::ConduitChange::SkillCostCommitted {
            source_uid: 10,
            team: 1,
            skill_id: 31490151,
            power_id: 999,
            activation_cost: 0,
            consumed_this_round: 0,
        },
    ] {
        assert!(
            project_change_for_test(&BattleChange::Conduit(change))
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn positive_cost_reduced_to_zero_suppresses_only_the_begin_marker() {
    let began = project_change_for_test(&BattleChange::Conduit(
        crate::engine::manager::conduit::ConduitChange::SkillBegan {
            source_uid: 10,
            team: 1,
            skill_id: 31490121,
            power_id: 1,
            activation_cost: 3,
            spent: 0,
        },
    ))
    .unwrap();
    assert!(began.is_empty());

    let committed = project_change_for_test(&BattleChange::Conduit(
        crate::engine::manager::conduit::ConduitChange::SkillCostCommitted {
            source_uid: 10,
            team: 1,
            skill_id: 31490121,
            power_id: 1,
            activation_cost: 3,
            consumed_this_round: 3,
        },
    ))
    .unwrap();
    assert_eq!(committed.len(), 1);
    assert_eq!(committed[0].effect_num, Some(62));
}

#[test]
fn client_conduit_group_selection_projects_two_top_level_confirmations() {
    let frames = [
        SemanticFrame {
            owner: FrameOwner::Command,
            trigger: crate::engine::runtime::record::FrameTrigger::Active,
            items: vec![FrameItem::Change(Box::new(BattleChange::Conduit(
                crate::engine::manager::conduit::ConduitChange::GroupSelected {
                    source_uid: 263_811_366,
                    team: 1,
                    group: 3,
                },
            )))],
        },
        SemanticFrame {
            owner: FrameOwner::Command,
            trigger: crate::engine::runtime::record::FrameTrigger::Active,
            items: vec![FrameItem::Cue(RoundCue::ClientConduitSelectionConfirmed {
                source_uid: 263_811_366,
                team: 1,
                group: 3,
            })],
        },
    ];

    let steps = project(&frames).unwrap();
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0], steps[1]);
    assert!(steps.iter().all(|step| step.act_effect.len() == 1));
    let effect = &steps[0].act_effect[0];
    assert_eq!(effect.target_id, Some(263_811_366));
    assert_eq!(
        effect.effect_type,
        Some(EffectType::Deviceskillindex as i32)
    );
    assert_eq!(effect.effect_num, Some(3));
    assert_eq!(effect.team_type, Some(1));
    assert_eq!(effect.config_effect, Some(0));
}

#[test]
fn behavior_conduit_group_change_projects_one_keyed_confirmation() {
    let effects = project_change_for_test(&BattleChange::Conduit(
        crate::engine::manager::conduit::ConduitChange::SkillGroupChanged {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60293, "SetDeviceSkillIndex"),
            },
            source_uid: 263_811_366,
            team: 1,
            group: 1,
        },
    ))
    .unwrap();

    assert_eq!(effects.len(), 1);
    let [effect] = effects.as_slice() else {
        panic!("expected one behavior-owned conduit selection effect");
    };
    assert_eq!(effect.target_id, Some(263_811_366));
    assert_eq!(
        effect.effect_type,
        Some(EffectType::Deviceskillindex as i32)
    );
    assert_eq!(effect.effect_num, Some(1));
    assert_eq!(effect.team_type, Some(1));
    assert_eq!(effect.config_effect, Some(60293));
}
