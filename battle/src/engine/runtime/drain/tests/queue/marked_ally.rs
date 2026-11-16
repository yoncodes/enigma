use super::*;

#[test]
fn moldirs_ultimate_grants_moxie_to_the_exact_marked_ally() {
    crate::test_support::init_config();
    let entity = |uid, position| FightEntityInfo {
        uid: Some(uid),
        position: Some(position),
        current_hp: Some(10_000),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let marked = |uid, position, id| FightEntityInfo {
        buffs: vec![BuffInfo {
            buff_id: Some(id),
            r#type: Some(30920104),
            uid: Some(uid + 100),
            duration: Some(1),
            from_uid: Some(uid),
            ..Default::default()
        }],
        ..entity(uid, position)
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    model_id: Some(3121),
                    passive_skill: vec![30920221],
                    ex_skill: Some(31210131),
                    ..entity(10, 1)
                },
                marked(11, 2, 30920104),
                marked(12, 3, 30920105),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![marked(-1, 1, 30920104)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31210131, 30920221], []);
    let mut managers = BattleManagers::seeded(&fight);
    let mut cast: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 31210131,
    }
    .into();
    cast.mode = SkillExecutionMode::Direct;
    run(
        &mut managers,
        &TargetPool::from_fight(&fight),
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 2,
            ..Default::default()
        },
        [RuleOp::Skill(cast)],
    )
    .unwrap();
    assert_eq!(
        [10, 11, 12, -1].map(|uid| managers.ex_point.get(uid)),
        [1, 2, 0, 0]
    );
}
