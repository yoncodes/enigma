use super::*;

fn cast_with_balance(career: i32, target_career: i32, passive: bool) -> (i32, bool) {
    init_config();
    let attr = HeroAttribute { hp: Some(100_000), attack: Some(1_000), ..Default::default() };
    let fight = Fight {
        attacker: Some(FightTeam { entitys: vec![FightEntityInfo {
            uid: Some(10), model_id: Some(3079), career: Some(career), team_type: Some(1),
            current_hp: Some(100_000), attr: Some(attr), skill_group1: vec![30610111],
            passive_skill: if passive { vec![431415] } else { vec![] }, ..Default::default()
        }], ..Default::default() }),
        defender: Some(FightTeam { entitys: vec![FightEntityInfo {
            uid: Some(-1), career: Some(target_career), team_type: Some(2),
            current_hp: Some(100_000), attr: Some(attr), ..Default::default()
        }], ..Default::default() }), ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.execute_card(CardCommand::Setup(CardSetup { hand: vec![CardInfo {
        uid: Some(10), skill_id: Some(30610111), ..Default::default()
    }], draw_pile: vec![], deck_num: 1 })).unwrap();
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [30610111, 431415], [43141111]);
    let blocking = catalog.issues(431415).into_iter().filter(|issue| issue.reason.is_blocking()).collect::<Vec<_>>();
    assert!(blocking.is_empty(), "Balance, Please has unsupported rules: {blocking:?}");
    run_player_action_queue(&mut managers, &pool, &catalog, &mut RoundDeterminism::default(),
        TargetContext::default(), [CardPlay { origin: CARD_PLAY_ORIGIN, hand_index: 0,
            target_uid: Some(-1), chosen_skill_id: None, choice: None, recorded_skill: None }],
        1, 99998).unwrap();
    (100_000 - managers.hp.current(-1), managers.buff.has_buff_id(-1, 43141111))
}

#[test]
fn balance_psychube_applies_for_neutral_afflatus_and_doubles_for_spirit_or_intellect() {
    for (career, expected) in [(1, 112), (5, 124), (6, 124)] {
        let (base, _) = cast_with_balance(career, 1, false);
        let (boosted, marker_left) = cast_with_balance(career, 1, true);
        assert!((boosted * 100 - base * expected).abs() <= 100,
            "career={career}, base={base}, boosted={boosted}");
        assert!(!marker_left, "temporary target marker must be cleaned up");
    }
}

#[test]
fn balance_psychube_does_not_boost_an_afflatus_advantage_hit() {
    for (career, target) in [(1, 4), (5, 6), (6, 5)] {
        let (base, _) = cast_with_balance(career, target, false);
        let (boosted, _) = cast_with_balance(career, target, true);
        assert_eq!(boosted, base, "career={career}, target={target}");
    }
}

fn cast_balance_area_attack(passive: bool) -> (i32, i32) {
    init_config();
    let attr = HeroAttribute { hp: Some(100_000), attack: Some(1_000), ..Default::default() };
    let fight = Fight {
        attacker: Some(FightTeam { entitys: vec![FightEntityInfo {
            uid: Some(10), model_id: Some(3079), career: Some(5), team_type: Some(1),
            current_hp: Some(100_000), attr: Some(attr), skill_group1: vec![30790111],
            passive_skill: if passive { vec![431415] } else { vec![] }, ..Default::default()
        }], ..Default::default() }),
        defender: Some(FightTeam { entitys: vec![
            FightEntityInfo { uid: Some(-1), career: Some(1), team_type: Some(2), position: Some(1),
                current_hp: Some(100_000), attr: Some(attr), ..Default::default() },
            FightEntityInfo { uid: Some(-2), career: Some(6), team_type: Some(2), position: Some(2),
                current_hp: Some(100_000), attr: Some(attr), ..Default::default() },
        ], ..Default::default() }), ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.execute_card(CardCommand::Setup(CardSetup { hand: vec![CardInfo {
        uid: Some(10), skill_id: Some(30790111), ..Default::default()
    }], draw_pile: vec![], deck_num: 1 })).unwrap();
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [30790111, 431415], [43141111]);
    run_player_action_queue(&mut managers, &pool, &catalog, &mut RoundDeterminism::default(),
        TargetContext::default(), [CardPlay { origin: CARD_PLAY_ORIGIN, hand_index: 0,
            target_uid: Some(-1), chosen_skill_id: None, choice: None, recorded_skill: None }],
        1, 99998).unwrap();
    (100_000 - managers.hp.current(-1), 100_000 - managers.hp.current(-2))
}

#[test]
fn balance_psychube_checks_afflatus_per_target_on_area_attacks() {
    let (base_neutral, base_advantage) = cast_balance_area_attack(false);
    let (boosted_neutral, boosted_advantage) = cast_balance_area_attack(true);
    assert!(base_neutral > 0 && base_advantage > 0);
    assert!((boosted_neutral * 100 - base_neutral * 124).abs() <= 100,
        "neutral: {base_neutral} -> {boosted_neutral}");
    assert_eq!(boosted_advantage, base_advantage);
}
