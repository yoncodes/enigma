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

#[test]
fn obscurity_break_enters_the_witness_round_on_the_next_round() {
    init_config();
    let entity = |uid, buffs, passive_skill| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(10_000),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            ..Default::default()
        }),
        buffs,
        passive_skill,
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, Vec::new(), Vec::new())],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(
                -1,
                vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(109320001),
                    duration: Some(1),
                    from_uid: Some(-1),
                    ..Default::default()
                }],
                vec![109320102, 109320105],
            )],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);

    run_round_end_final_settlement(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        Vec::new(),
    )
    .unwrap();

    // "When Obscurity reaches zero, you'll enter a bonus Witness Round during the following
    // round", in which "enemies cannot take active actions".
    assert!(managers.buff.has_active_buff_id(-1, 109320002));
    assert!(managers.buff.has_active_buff_id(-1, 109320103));
}
