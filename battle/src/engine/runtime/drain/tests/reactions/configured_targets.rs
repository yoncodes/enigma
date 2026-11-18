use super::*;

#[test]
fn passive_keeps_its_configured_recipient() {
    crate::test_support::init_config();
    let entity = |uid, position| FightEntityInfo {
        uid: Some(uid),
        position: Some(position),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut caster = entity(10, 2);
    caster.passive_skill = vec![30630161];
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![entity(11, 1), caster],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 1), entity(-2, 2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 30630122,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    run(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();
    assert!(managers.buff.has_buff_id(11, 301));
    for uid in [10, -1, -2] {
        assert!(!managers.buff.has_buff_id(uid, 301));
    }
}
