use super::*;

#[test]
fn self_target_does_not_need_fixture_choice() {
    let mut determinism = RoundDeterminism::default();
    let targets = TargetResolver::resolve(
        &TargetRequest::self_only(),
        1001,
        42,
        &TargetPool::default(),
        &mut determinism,
    );

    assert_eq!(targets, vec![42]);
}

#[test]
fn captured_choice_does_not_override_builtin_target_code() {
    let mut determinism = RoundDeterminism::default();
    determinism.skill_targets.push(SkillTargetChoice {
        skill_id: 1001,
        source_uid: 42,
        target_code: 0,
        targets: vec![7, 8],
        additional_targets: Vec::new(),
        crit_targets: Vec::new(),
        additional_crit_targets: Vec::new(),
    });

    let targets = TargetResolver::resolve(
        &TargetRequest::self_only(),
        1001,
        42,
        &TargetPool::default(),
        &mut determinism,
    );

    assert_eq!(targets, vec![42]);
    assert_eq!(determinism.skill_targets.len(), 1);
}

#[test]
fn logic_target_uses_the_parent_determinism_stream() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity_at(10, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity_at(-10, 1), entity_at(-11, 2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut determinism = RoundDeterminism::with_seed(7);
    let mut expected = RoundDeterminism::with_seed(7);
    let _ = expected.lua_random_index(2);

    let targets = TargetResolver::resolve_with_context(
        &TargetRequest {
            code: 0,
            raw: Vec::new(),
        },
        1001,
        10,
        &TargetPool::from_fight(&fight),
        &mut determinism,
        TargetContext {
            logic_target: 206,
            ..Default::default()
        },
    );

    assert_eq!(targets.len(), 1);
    assert_eq!(
        determinism.lua_random_index(100),
        expected.lua_random_index(100)
    );
}
