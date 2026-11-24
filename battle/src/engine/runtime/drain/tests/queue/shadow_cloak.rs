use super::*;

#[test]
fn shadow_sentry_heals_allies_without_shadow_friend() {
    crate::test_support::init_config();
    let entity = |uid, team_type| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(10_000),
        attr: Some(HeroAttribute {
            hp: Some(20_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    buffs: vec![BuffInfo {
                        uid: Some(77),
                        buff_id: Some(31250151),
                        from_uid: Some(10),
                        act_common_params: Some("0#5000".to_owned()),
                        ..Default::default()
                    }],
                    ..entity(10, 1)
                },
                entity(11, 1),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31250121], [31250151]);
    let mut managers = BattleManagers::seeded(&fight);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 31250121,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;

    run_action(
        &mut managers,
        &TargetPool::from_fight(&fight),
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [],
        invocation,
    )
    .unwrap();

    assert_eq!(managers.hp.current(11), 16_500);
    assert_eq!(managers.hp.max(11), 20_000);
    assert_eq!(managers.hp.max(10), 20_660);
    assert_eq!(
        managers
            .buff
            .snapshot(10, 77)
            .unwrap()
            .act_common_params
            .as_deref(),
        Some("660#5000")
    );
}
