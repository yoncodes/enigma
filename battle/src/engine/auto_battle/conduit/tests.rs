use super::*;

fn began(source_uid: i64, skill_id: i32, spent: i32) -> RuleOutcome {
    RuleOutcome::Conduit(ConduitChange::SkillBegan {
        source_uid,
        team: 1,
        skill_id,
        power_id: 1,
        activation_cost: 1,
        spent,
        generic_spent: 0,
    })
}

#[test]
fn distinct_skills_keep_each_committed_cost_after_one_use_reduction() {
    let skills = [
        ConduitSkill {
            skill_id: 10,
            cost_type: 1,
            cost_value: 1,
            is_stopped: false,
        },
        ConduitSkill {
            skill_id: 20,
            cost_type: 1,
            cost_value: 1,
            is_stopped: false,
        },
    ];
    let outcomes = [began(7, 10, 0), began(7, 20, 1), began(7, 10, 0)];

    assert_eq!(
        executed_skills(&outcomes, 7, &skills),
        [(skills[0], 0), (skills[1], 1)]
    );
}

#[test]
fn rejects_a_two_state_group_cycle() {
    let mut seen = HashSet::new();
    let groups = |left, right| {
        [
            FightDeviceOper {
                uid: Some(10),
                index: Some(left),
            },
            FightDeviceOper {
                uid: Some(11),
                index: Some(right),
            },
        ]
    };

    assert_eq!(record_groups(&mut seen, &groups(1, 2)), Ok(()));
    assert_eq!(record_groups(&mut seen, &groups(2, 1)), Ok(()));
    assert_eq!(
        record_groups(&mut seen, &groups(1, 2)),
        Err(AutoBattleError::OscillatingConduitGroups)
    );
}

#[test]
fn current_team_vector_is_simulated_once_for_all_devices() {
    let skill = ConduitSkill {
        skill_id: 10,
        cost_type: 1,
        cost_value: 1,
        is_stopped: false,
    };
    let devices = [10, 11].map(|uid| ConduitDevice {
        uid,
        selected_group: 1,
        skill_groups: vec![vec![skill], vec![skill]],
    });
    let operations = devices
        .iter()
        .map(|device| FightDeviceOper {
            uid: Some(device.uid),
            index: Some(1),
        })
        .collect::<Vec<_>>();

    let available = devices
        .iter()
        .flat_map(|device| (1..=device.skill_groups.len()).map(move |group| (device.uid, group)))
        .collect();
    let jobs = group_jobs(&devices, &operations, &available);

    assert_eq!(jobs.len(), 3);
    assert_eq!(
        jobs.iter()
            .find(|job| job.operations == operations)
            .unwrap()
            .candidates
            .len(),
        2
    );
}

#[test]
fn unavailable_groups_are_not_simulation_jobs() {
    let skill = ConduitSkill {
        skill_id: 10,
        cost_type: 1,
        cost_value: 1,
        is_stopped: false,
    };
    let devices = [ConduitDevice {
        uid: 10,
        selected_group: 1,
        skill_groups: vec![vec![skill], vec![skill], vec![skill]],
    }];
    let operations = vec![FightDeviceOper {
        uid: Some(10),
        index: Some(1),
    }];
    let available = [(10, 1), (10, 2)].into_iter().collect();

    let jobs = group_jobs(&devices, &operations, &available);

    assert_eq!(jobs.len(), 2);
    assert!(jobs.iter().all(|job| job.operations[0].index != Some(3)));
}

#[test]
fn only_strictly_lower_priority_groups_are_pruned() {
    assert!(!priority_can_beat(Some((1, 0, 0)), 0));
    assert!(priority_can_beat(Some((1, 0, 0)), 1));
    assert!(priority_can_beat(None, 0));
}
