use super::*;

#[test]
fn bullet_bonus_counts_active_types_on_round_start() {
    init_config();
    let bullet = |uid, buff_id, count| BuffInfo {
        uid: Some(uid),
        buff_id: Some(buff_id),
        count: Some(count),
        from_uid: Some(10),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                position: Some(1),
                current_hp: Some(100),
                passive_skill: vec![436215],
                buffs: vec![
                    bullet(100, 722501, 2),
                    bullet(101, 722502, 3),
                    bullet(102, 31020111, 1),
                    bullet(103, 31020113, 0),
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let pool = TargetPool::from_fight(&fight);
    crate::engine::runtime::drain::run_setup_stage(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        SetupStage::RoundStart,
        1,
    )
    .unwrap();

    assert_eq!(managers.buff.buff_id_amount(10, 436225), 2);
}
