use super::*;

#[test]
fn oracle_riddle_spends_its_last_stack_during_final_settlement() {
    init_config();
    let unit = |uid, team| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team),
        position: Some(1),
        current_hp: Some(10_000),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                passive_skill: vec![31572141],
                buffs: vec![BuffInfo {
                    uid: Some(100),
                    from_uid: Some(10),
                    buff_id: Some(31570003),
                    layer: Some(1),
                    ..Default::default()
                }],
                ..unit(10, 1)
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![unit(-1, 2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut determinism = RoundDeterminism::default();
    for expected in [1, 0] {
        let settled = run_round_end_final_settlement(
            &mut managers,
            &pool,
            &catalog,
            &mut determinism,
            TargetContext::default(),
            Vec::new(),
        )
        .unwrap();
        assert_eq!(
            settled
                .events
                .iter()
                .filter(|event| matches!(
                    event, BattleEvent::Hit(hit)
                        if hit.skill_id == 31570114 && hit.source_uid == 10 && hit.target_uid == -1
                ))
                .count(),
            expected
        );
        assert!(!managers.buff.has_active_buff_id(10, 31570003));
    }
}
