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

fn command(candidates: Vec<(i64, i32)>) -> RuleOp {
    RuleOp::Command(BattleCommand::LowestExPoint(LowestExPointChange {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(60174, "ConsumeExPointAddAttr"),
        },
        source_uid: 10,
        candidates,
        delta: 1,
        config_effect: 0,
        effect_type: 0,
    }))
}

fn run(fight: Fight, count: usize, candidates: &[(i64, i32)]) -> (BattleManagers, DrainResult) {
    crate::test_support::init_config();
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let result = run_command_group(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        (0..count).map(|_| command(candidates.to_vec())),
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
fn a_moxie_grant_publishes_cost_after_its_committed_cap() {
    use crate::engine::{
        manager::ex_point::ExPointChanges, skill::buff_act::registry::BuffActKind,
    };

    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 5, 5, Vec::new())],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    for (buff_id, cap, cost_delta, order) in [
        (31400000, 4, -1, &["cap", "cost"][..]),
        (823100107, 3, -2, &["cost", "cap"][..]),
    ] {
        let mut managers = BattleManagers::seeded(&fight);
        let result = run_command_group(
            &mut managers,
            &pool,
            &SkillEffectCatalog::default(),
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            [RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
                BuffGrant {
                    origin: CommandOrigin {
                        domain: RuleDomain::Behavior,
                        key: DefinitionKey::new(1, "AddBuff"),
                    },
                    source_uid: 10,
                    target_uid: 10,
                    buff_id,
                    amount: None,
                    occurrences: 1,
                    child_uid_reservations: 0,
                },
            )))],
        )
        .unwrap();
        let outcomes = result.outcomes.as_slice();
        let [RuleOutcome::Buff(grant), reactions @ ..] = outcomes else {
            panic!("expected one grant followed by Moxie reactions");
        };
        assert!(grant.change.added.as_ref().unwrap().markers.is_empty());
        let observed = reactions
            .iter()
            .map(|outcome| match outcome {
                RuleOutcome::ExPoint(ExPointChanges::Max { change, .. }) => {
                    assert_eq!((change.target_uid, change.after), (10, cap));
                    "cap"
                }
                RuleOutcome::BuffFeatureMarker(cost) => {
                    assert_eq!(
                        cost.effect_type,
                        sonettobuf::effect_type_enum::EffectType::Exskillpointchange as i32
                    );
                    assert_eq!((cost.target_uid, cost.effect_num), (10, cost_delta));
                    "cost"
                }
                _ => panic!("unexpected Moxie grant reaction"),
            })
            .collect::<Vec<_>>();
        assert_eq!(observed, order);
        assert_eq!(managers.ex_point.maximum(10), Some(cap));
        assert_eq!(managers.ex_point.get(10), cap);
        assert_eq!(
            managers
                .buff
                .buff_act_scalar(10, BuffActKind::ExSkillPointChange),
            cost_delta
        );
    }
}

#[test]
fn steady_composure_grants_cost_in_the_entry_skill_frame() {
    use crate::engine::runtime::record::FrameItem;
    use sonettobuf::effect_type_enum::EffectType;

    crate::test_support::init_config();
    let mut artillerist = entity(10, 5, 5, Vec::new());
    artillerist.passive_skill.push(222001142);
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![artillerist],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let result = run_setup_stage(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        SetupStage::EnterFight,
        0,
    )
    .unwrap();
    let [FrameItem::Child(entrant)] = result.frames[0].items.as_slice() else {
        panic!("expected one entrant");
    };
    let [FrameItem::Child(skill)] = entrant.items.as_slice() else {
        panic!("expected one entry skill");
    };
    assert!(matches!(
        skill.owner,
        FrameOwner::Skill {
            skill_id: 222001142,
            ..
        }
    ));
    let [FrameItem::Change(grant), FrameItem::Change(cost)] = skill.items.as_slice() else {
        panic!("expected the grant and cost change on the entry skill");
    };
    assert!(matches!(
        grant.as_ref(), BattleChange::Buff(grant)
            if grant.change.added.as_ref().is_some_and(|added| added.buff.buff_id == Some(2220012))
    ));
    assert!(matches!(
        cost.as_ref(), BattleChange::BuffFeatureMarker(change)
            if change.target_uid == 10 && change.effect_type == EffectType::Exskillpointchange as i32
                && change.effect_num == -4
    ));
    assert_eq!(managers.ex_point.maximum(10), Some(5));
}

#[test]
fn lowest_moxie_commands_reselect_from_committed_state() {
    let mut everecho = entity(40, 0, 100, Vec::new());
    everecho.ex_point_type =
        Some(crate::engine::manager::ex_point::ExPointKind::DevicePower.as_wire());
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(10, 5, 10, Vec::new()),
                entity(20, 0, 10, Vec::new()),
                entity(30, 3, 10, Vec::new()),
                everecho,
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let (_, result) = run(fight, 7, &[(20, 2), (30, 3), (40, 4)]);

    assert_eq!(
        changes(&result),
        [
            (20, 1),
            (20, 1),
            (20, 1),
            (20, 1),
            (30, 1),
            (20, 1),
            (30, 1)
        ]
    );
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
    let (managers, result) = run(redirected, 3, &[(20, 2), (30, 3)]);
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
    let (_, result) = run(blocked, 3, &[(20, 2), (30, 3)]);
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
    let (_, result) = run(capped, 3, &[(20, 2), (30, 3)]);
    assert_eq!(changes(&result), [(20, 0), (20, 0), (20, 0)]);
}
