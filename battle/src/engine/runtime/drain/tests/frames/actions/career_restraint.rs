use super::*;
use crate::engine::skill::action::SkillExecutionMode;

#[test]
fn career_restraint_lasts_through_the_own_effects_of_a_non_attack() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                model_id: Some(3134),
                current_hp: Some(100),
                passive_skill: vec![103374322],
                buffs: [31260211, 31450171, 31450143, 31440116, 31450111, 103374114]
                    .into_iter()
                    .enumerate()
                    .map(|(index, buff_id)| BuffInfo {
                        uid: Some(index as i64 + 2),
                        buff_id: Some(buff_id),
                        from_uid: Some(10),
                        duration: (buff_id == 31260211).then_some(1),
                        layer: (buff_id == 103374114).then_some(2),
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
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31340121, 103374322], []);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 31340121,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;

    let result = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    let effects = result
        .outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            RuleOutcome::Buff(changes) if !changes.change.removed.is_empty() => {
                Some((false, changes.change.removed.first()?.buff.buff_id?))
            }
            RuleOutcome::Buff(changes) => {
                Some((true, changes.change.added.as_ref()?.buff.buff_id?))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        effects,
        [
            (true, 103374322),
            (false, 31260211),
            (true, 31340002),
            (false, 103374322),
        ]
    );
    assert!(managers.buff.snapshot(10, 2).is_none());
    assert!(!managers.buff.has_buff_id(10, 103374322));
}

#[test]
fn prepared_restraint_observes_committed_purple_crystal_halo() {
    crate::test_support::init_config();
    let entity = |uid, team| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team),
        current_hp: Some(10_000),
        attr: Some(sonettobuf::HeroAttribute {
            hp: Some(10_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let actor = FightEntityInfo {
        model_id: Some(3134),
        passive_skill: vec![103374322],
        buffs: [31450171, 31450143, 31440116, 31450111, 103374114]
            .into_iter()
            .enumerate()
            .map(|(index, buff_id)| BuffInfo {
                uid: Some(index as i64 + 2),
                buff_id: Some(buff_id),
                from_uid: Some(10),
                layer: (buff_id == 103374114).then_some(5),
                ..Default::default()
            })
            .collect(),
        ..entity(10, 1)
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![actor],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    assert!(managers.emanation.select(10, 110));
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31340151, 103374322], []);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 31340151,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;

    let result = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    let grant = |target_uid, buff_id| {
        result
            .outcomes
            .iter()
            .position(|outcome| {
                matches!(outcome, RuleOutcome::Buff(changes)
                if changes.change.added.as_ref().is_some_and(|addition|
                    addition.target_uid == target_uid && addition.buff.buff_id == Some(buff_id)))
            })
            .unwrap()
    };
    let halo = grant(-1, 31340001);
    let restraint = grant(10, 103374322);
    let hit = result
        .outcomes
        .iter()
        .position(|outcome| matches!(outcome, RuleOutcome::HpBatch(_)))
        .unwrap();
    assert!(halo < restraint && restraint < hit);
    assert!(managers.buff.has_buff_id(-1, 31340001));
    assert!(managers.hp.current(-1) < 10_000);
}
