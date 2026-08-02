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
fn zero_cost_conduit_activation_has_no_cost_projection() {
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
