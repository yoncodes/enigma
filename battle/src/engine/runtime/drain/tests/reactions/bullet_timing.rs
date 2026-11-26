use super::*;

#[test]
fn bullet_reactions_wait_for_the_attacking_skills_stat_grants() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut source = entity(10);
    source.passive_skill = vec![31020151, 433711];
    source.buffs = vec![BuffInfo {
        uid: Some(40),
        buff_id: Some(31020111),
        from_uid: Some(10),
        count: Some(2),
        ..Default::default()
    }];
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![source, entity(11)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.attribute.override_ex(
        10,
        &HeroExAttribute {
            cri_dmg: Some(2_000),
            ..Default::default()
        },
    );
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 31020141,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    run(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(invocation.clone())],
    )
    .unwrap();
    assert!(managers.buff.has_buff_id(10, 433711));
    assert_eq!(managers.ex_point.get(10), 1);
    assert_eq!(
        managers.buff.fixed_attribute_delta(10, AttrId::CriticalDmg),
        200
    );
    assert_eq!(
        managers.buff.fixed_attribute_delta(11, AttrId::CriticalDmg),
        200
    );

    run(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();
    for uid in [10, 11] {
        assert_eq!(
            managers
                .buff
                .fixed_attribute_delta(uid, AttrId::CriticalDmg),
            216
        );
    }
}
