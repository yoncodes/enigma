use super::*;

fn joe_device_fight(burn: i32) -> Fight {
    let attr = HeroAttribute { hp: Some(100_000), attack: Some(1_000), ..Default::default() };
    let mut joe_buffs = vec![BuffInfo { uid: Some(40), buff_id: Some(30940001), from_uid: Some(10), ..Default::default() }];
    if burn > 0 {
        joe_buffs.push(BuffInfo { uid: Some(41), buff_id: Some(4150001), from_uid: Some(10),
            layer: Some(burn), count: Some(1), ..Default::default() });
    }
    Fight {
        attacker: Some(FightTeam { entitys: vec![
            FightEntityInfo { uid: Some(10), model_id: Some(3094), team_type: Some(1),
                current_hp: Some(100_000), attr: Some(attr), skill_group2: vec![30941124],
                buffs: joe_buffs, ..Default::default() },
            FightEntityInfo { uid: Some(20), model_id: Some(3149), team_type: Some(1),
                current_hp: Some(100_000), attr: Some(attr), skill_group1: vec![31446011],
                ..Default::default() },
        ], ..Default::default() }),
        defender: Some(FightTeam { entitys: vec![FightEntityInfo { uid: Some(-1),
            model_id: Some(1001), team_type: Some(2), current_hp: Some(100_000), attr: Some(attr),
            ..Default::default() }], ..Default::default() }), ..Default::default()
    }
}

fn play_joe_and_device(burn: i32, device_first: bool, joe_cards: usize) -> (BattleManagers, DrainResult) {
    init_config();
    let fight = joe_device_fight(burn);
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let joe = || CardInfo { uid: Some(10), hero_id: Some(3094), skill_id: Some(30941124), ..Default::default() };
    let device = || CardInfo { uid: Some(20), hero_id: Some(3149), skill_id: Some(31446011),
        temp_card: Some(false), card_type: Some(0), ..Default::default() };
    let mut hand = Vec::new();
    if device_first { hand.push(device()); }
    hand.extend((0..joe_cards).map(|_| joe()));
    if !device_first { hand.push(device()); }
    managers.execute_card(CardCommand::Setup(CardSetup { deck_num: hand.len() as i32,
        hand, draw_pile: vec![] })).unwrap();
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [30941124, 31446011], [30940001, 4150001]);
    let play = || CardPlay { origin: CARD_PLAY_ORIGIN, hand_index: 0, target_uid: None,
        chosen_skill_id: None, choice: None, recorded_skill: None };
    let result = run_player_action_queue(&mut managers, &pool, &catalog, &mut RoundDeterminism::default(),
        TargetContext::default(), (0..=joe_cards).map(|_| play()), 1,
        crate::engine::manager::emitter::UID).unwrap();
    (managers, result)
}

#[test]
fn joe_euphoria_shield_and_device_cards_work_in_either_order() {
    for first in [false, true] {
        let (managers, result) = play_joe_and_device(10, first, 1);
        assert_eq!(managers.hp.shield(10), 2_000, "J shield, device_first={first}");
        assert_eq!(managers.hp.shield(20), 2_000, "device ally shield, device_first={first}");
        assert!(result.events.iter().any(|event| matches!(event, BattleEvent::AllyAction(action)
            if action.skill_id == 30941124)));
        assert!(result.events.iter().any(|event| matches!(event, BattleEvent::AllyAction(action)
            if action.skill_id == 31446011
                && action.mode == crate::engine::skill::action::SkillExecutionMode::DeviceCard)));
        crate::engine::packet::timeline::project(&result.frames).expect("mixed action timeline must serialize");
    }
}

#[test]
fn joe_euphoria_shields_scale_by_burn_and_stack_to_six_times_attack() {
    let (unburned, _) = play_joe_and_device(0, false, 1);
    assert_eq!(unburned.hp.shield(10), 1_500);
    let (stacked, _) = play_joe_and_device(10, false, 4);
    assert_eq!(stacked.hp.shield(10), 6_000);
    assert_eq!(stacked.hp.shield(20), 6_000);
}

#[test]
fn joe_euphoria_device_ally_can_be_attacked_and_trigger_the_real_riposte() {
    let (mut managers, _) = play_joe_and_device(10, false, 1);
    let fight = joe_device_fight(10);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(),
        [102611, 30941124, 30941172, 31446011], [30940001, 4150001, 30941121, 30941182]);
    let mut attack: crate::engine::skill::action::SkillInvocation =
        crate::engine::skill::action::SkillRequest { source_uid: -1, skill_id: 102611 }.into();
    attack.mode = crate::engine::skill::action::SkillExecutionMode::Active;
    attack.target = crate::engine::skill::action::SkillTarget::Explicit(20);
    let result = drain::run_action(&mut managers, &pool, &catalog, &mut RoundDeterminism::default(),
        TargetContext::default(), [], attack).expect("enemy attack and J's riposte must complete");
    assert!(result.events.iter().any(|event| matches!(event, BattleEvent::SkillAction(action)
        if action.source_uid == 10 && action.skill_id == 30941172)));
    assert!(managers.hp.shield(20) > 1_200, "J's riposte should replenish the ally's shield");
    crate::engine::packet::timeline::project(&result.frames).expect("riposte timeline must serialize");
}

#[test]
fn joe_euphoria_burn_is_harmless_and_retains_layers_only_for_joe() {
    init_config();
    let mut fight = joe_device_fight(6);
    let ally = &mut fight.attacker.as_mut().unwrap().entitys[1];
    ally.buffs.push(BuffInfo { uid: Some(42), buff_id: Some(4150001), from_uid: Some(20),
        layer: Some(6), count: Some(1), ..Default::default() });
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    run_attacker_round_end(&mut managers, &pool, &catalog, &mut RoundDeterminism::default(),
        TargetContext::default()).unwrap();
    assert_eq!(managers.hp.current(10), 100_000);
    assert_eq!(managers.buff.buff_id_amount(10, 4150001), 6);
    assert_eq!(managers.hp.current(20), 99_760);
    assert_eq!(managers.buff.buff_id_amount(20, 4150001), 3);
}

#[test]
fn joe_euphoria_all_shield_ranks_and_riposte_have_no_blocking_definition_errors() {
    init_config();
    let skills = [30941121, 30941122, 30941123, 30941124, 30941125, 30941126, 30941171, 30941172];
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), skills, []);
    for skill in skills {
        let issues = catalog.issues(skill);
        assert!(issues.iter().all(|issue| !issue.reason.is_blocking()), "skill {skill}: {issues:?}");
    }
}
