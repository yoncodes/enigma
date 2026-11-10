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
