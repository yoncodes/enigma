use super::*;

#[test]
fn final_settlement_advances_final_stage_durations() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(109320002),
                    duration: Some(1),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);

    let result = run_round_end_final_settlement(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        Vec::new(),
    )
    .unwrap();

    assert!(managers.buff.snapshot(10, 20).is_none());
    let settled = result
        .events
        .iter()
        .position(|event| matches!(event, BattleEvent::BuffsSettled(_)))
        .unwrap();
    let (removed, duration_expired) = result
        .events
        .iter()
        .enumerate()
        .find_map(|(index, event)| match event {
            BattleEvent::BuffRemoved(change) if change.buff_id == 109320002 => {
                Some((index, change.duration_expired))
            }
            _ => None,
        })
        .unwrap();
    assert!(settled < removed);
    assert!(duration_expired);
}
