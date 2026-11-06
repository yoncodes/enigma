use super::*;

fn entity(uid: i64, ex_point: i32, max: i32, buffs: Vec<BuffInfo>) -> FightEntityInfo {
    FightEntityInfo {
        uid: Some(uid),
        ex_point: Some(ex_point),
        ex_point_max: Some(max),
        current_hp: Some(100),
        attr: Some(HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        buffs,
        ..Default::default()
    }
}

fn command() -> RuleOp {
    RuleOp::Command(BattleCommand::LowestExPoint(LowestExPointChange {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(60174, "ConsumeExPointAddAttr"),
        },
        source_uid: 10,
        candidates: vec![(20, 2), (30, 3)],
        delta: 1,
        config_effect: 0,
        effect_type: 0,
    }))
}

fn run(fight: Fight, count: usize) -> (BattleManagers, DrainResult) {
    crate::test_support::init_config();
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let result = run_command_group(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        (0..count).map(|_| command()),
    )
    .unwrap();
    (managers, result)
}

fn changes(result: &DrainResult) -> Vec<(i64, i32)> {
    result
        .outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            RuleOutcome::ExPoint(crate::engine::manager::ex_point::ExPointChanges::Value {
                change,
                ..
            }) => Some((change.target_uid, change.applied_delta)),
            _ => None,
        })
        .collect()
}

#[test]
fn lowest_moxie_commands_reselect_from_committed_state() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(10, 5, 10, Vec::new()),
                entity(20, 0, 10, Vec::new()),
                entity(30, 2, 10, Vec::new()),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let (_, result) = run(fight, 4);

    assert_eq!(changes(&result), [(20, 1), (20, 1), (20, 1), (30, 1)]);
}

#[test]
fn lowest_moxie_commands_observe_redirects_blocks_and_caps() {
    let redirected = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(10, 0, 10, Vec::new()),
                entity(
                    20,
                    0,
                    10,
                    vec![BuffInfo {
                        uid: Some(1),
                        buff_id: Some(31_000_171),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                ),
                entity(30, 1, 10, Vec::new()),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let (managers, result) = run(redirected, 3);
    assert_eq!(changes(&result), [(10, 1), (10, 1), (10, 1)]);
    assert_eq!(managers.ex_point.get(20), 0);

    let blocked = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(10, 0, 10, Vec::new()),
                entity(
                    20,
                    0,
                    10,
                    vec![BuffInfo {
                        uid: Some(1),
                        buff_id: Some(31_050_132),
                        from_uid: Some(20),
                        ..Default::default()
                    }],
                ),
                entity(30, 1, 10, Vec::new()),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let (_, result) = run(blocked, 3);
    assert_eq!(changes(&result), [(20, 0), (20, 0), (20, 0)]);

    let capped = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(10, 0, 10, Vec::new()),
                entity(20, 5, 5, Vec::new()),
                entity(30, 5, 10, Vec::new()),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let (_, result) = run(capped, 3);
    assert_eq!(changes(&result), [(20, 0), (20, 0), (20, 0)]);
}
