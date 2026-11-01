use super::*;

#[test]
fn playing_a_marked_ally_card_triggers_ramona_follow_up() {
    init_config();
    let entity = |uid, model_id, team| FightEntityInfo {
        uid: Some(uid), model_id: Some(model_id), team_type: Some(team), current_hp: Some(100_000),
        attr: Some(HeroAttribute { hp: Some(100_000), attack: Some(1_000), ..Default::default() }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam { entitys: vec![
            FightEntityInfo { skill_group1: vec![30230111], ..entity(10, 3023, 1) },
            FightEntityInfo { passive_skill: vec![31420151], ..entity(30, 3142, 1) },
        ], ..Default::default() }),
        defender: Some(FightTeam { entitys: vec![entity(-1, 1001, 2)], ..Default::default() }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.execute_card(CardCommand::Setup(CardSetup {
        hand: vec![CardInfo { uid: Some(10), skill_id: Some(30230111),
            enchants: vec![sonettobuf::CardEnchant { enchant_id: Some(10011), duration: Some(1), ..Default::default() }],
            ..Default::default() }], draw_pile: Vec::new(), deck_num: 0,
    })).unwrap();
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let result = run_player_action_queue(&mut managers, &pool, &catalog,
        &mut RoundDeterminism::default(), TargetContext::default(),
        [CardPlay { origin: CARD_PLAY_ORIGIN, hand_index: 0, target_uid: Some(-1),
            chosen_skill_id: None, choice: None, recorded_skill: None }],
        1, crate::engine::manager::emitter::UID).unwrap();
    fn contains(step: &sonettobuf::FightStep) -> bool {
        step.act_id == Some(31420141) || step.act_effect.iter()
            .filter_map(|effect| effect.fight_step.as_ref()).any(contains)
    }
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    assert!(steps.iter().any(contains), "marked hand card did not trigger Ramona's follow-up");
}
