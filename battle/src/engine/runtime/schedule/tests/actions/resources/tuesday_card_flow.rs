use super::*;

#[test]
fn tuesday_field_rolls_for_status_inflicted_by_a_played_card() {
    init_config();
    let entity = |uid, model_id, team| FightEntityInfo {
        uid: Some(uid), model_id: Some(model_id), team_type: Some(team), current_hp: Some(100_000),
        attr: Some(HeroAttribute { hp: Some(100_000), attack: Some(1_000), ..Default::default() }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam { entitys: vec![
            FightEntityInfo { skill_group1: vec![31420111], buffs: vec![BuffInfo {
                uid: Some(20), buff_id: Some(30980142), from_uid: Some(11), ..Default::default()
            }], ..entity(10, 3142, 1) }, entity(11, 3098, 1),
        ], ..Default::default() }),
        defender: Some(FightTeam { entitys: vec![entity(-1, 1001, 2)], ..Default::default() }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.execute_card(CardCommand::Setup(CardSetup {
        hand: vec![CardInfo { uid: Some(10), skill_id: Some(31420111), ..Default::default() }],
        draw_pile: Vec::new(), deck_num: 0,
    })).unwrap();
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut random = RoundDeterminism::default();
    random.enqueue_permille_rolls([0, 0, 0, 0, 0]);
    run_player_action_queue(&mut managers, &pool, &catalog, &mut random, TargetContext::default(),
        [CardPlay { origin: CARD_PLAY_ORIGIN, hand_index: 0, target_uid: Some(-1),
            chosen_skill_id: None, choice: None, recorded_skill: None }],
        1, crate::engine::manager::emitter::UID).unwrap();
    assert_eq!(managers.buff.buff_id_amount(-1, 31420000), 1);
    assert_eq!(managers.buff.buff_id_amount(-1, 30980145), 1);
}
