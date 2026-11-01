use super::*;

fn cast_six_ultimate(power: i32, buffs: &[i32]) -> (i32, i32, i32) {
    init_config();
    let attr = HeroAttribute { hp: Some(100_000), attack: Some(1_000), ..Default::default() };
    let fight = Fight {
        attacker: Some(FightTeam { entitys: vec![
            FightEntityInfo { uid: Some(10), model_id: Some(3079), team_type: Some(1),
                current_hp: Some(100_000), ex_point: Some(5), attr: Some(attr),
                power_infos: vec![PowerInfo { power_id: Some(1), num: Some(power), max: Some(6) }],
                ..Default::default() },
            crate::engine::manager::emitter::activation_entity(),
        ], ..Default::default() }),
        defender: Some(FightTeam { entitys: vec![
            FightEntityInfo { uid: Some(-1), team_type: Some(2), current_hp: Some(100_000),
                attr: Some(attr), buffs: buffs.iter().enumerate().map(|(index, id)| BuffInfo {
                    uid: Some(100 + index as i64), buff_id: Some(*id), from_uid: Some(10),
                    duration: Some(2), ..Default::default()
                }).collect(), ..Default::default() },
        ], ..Default::default() }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let count = managers.buff.visible_buff_status_type_count(-1, &[2, 4, 6]);
    managers.execute_card(CardCommand::Setup(CardSetup { hand: vec![CardInfo {
        uid: Some(10), skill_id: Some(307901383), ..Default::default()
    }], draw_pile: Vec::new(), deck_num: 1 })).unwrap();
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [307901383], buffs.iter().copied());
    run_player_action_queue(&mut managers, &pool, &catalog, &mut RoundDeterminism::default(),
        TargetContext::default(), [CardPlay { origin: CARD_PLAY_ORIGIN, hand_index: 0,
            target_uid: Some(-1), chosen_skill_id: None, choice: None, recorded_skill: None }],
        1, 99998).unwrap();
    (100_000 - managers.hp.current(-1), managers.eureka.get(10, 1).current, count)
}

#[test]
fn six_ultimate_consumes_eureka_for_each_distinct_debuff_type_bonus() {
    let (base, base_power, _) = cast_six_ultimate(4, &[]);
    let (boosted, remaining_power, count) = cast_six_ultimate(4, &[30530103, 30530104]);
    let (insufficient, unspent, _) = cast_six_ultimate(3, &[30530103, 30530104]);
    assert_eq!((base_power, remaining_power, unspent, count), (0, 0, 3, 2));
    assert!((boosted * 7 - base * 12).abs() <= 7, "base={base}, boosted={boosted}");
    assert_eq!(insufficient, base, "insufficient eureka must not grant bonus damage");
}

fn six_round_end_power(first: &[i32], second: &[i32]) -> i32 {
    init_config();
    let attr = HeroAttribute { hp: Some(100_000), attack: Some(1_000), ..Default::default() };
    let buffs = |ids: &[i32], offset: i64| ids.iter().enumerate().map(|(index, id)| BuffInfo {
        uid: Some(offset + index as i64), buff_id: Some(*id), from_uid: Some(10),
        duration: Some(2), ..Default::default()
    }).collect();
    let entity = |uid, ids: &[i32]| FightEntityInfo { uid: Some(uid), team_type: Some(1),
        current_hp: Some(100_000), attr: Some(attr), buffs: buffs(ids, uid * 10), ..Default::default() };
    let fight = Fight {
        attacker: Some(FightTeam { entitys: vec![
            FightEntityInfo { model_id: Some(3079), passive_skill: vec![30790144],
                power_infos: vec![PowerInfo { power_id: Some(1), num: Some(1), max: Some(6) }],
                ..entity(10, &[]) }, entity(11, first), entity(12, second),
        ], ..Default::default() }),
        defender: Some(FightTeam { entitys: vec![FightEntityInfo { uid: Some(-1), team_type: Some(2),
            current_hp: Some(100_000), attr: Some(attr), ..Default::default() }], ..Default::default() }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    run_attacker_round_end(&mut managers, &pool, &catalog, &mut RoundDeterminism::default(),
        TargetContext::default()).unwrap();
    managers.eureka.get(10, 1).current
}

#[test]
fn six_passive_round_end_requires_one_ally_with_four_positive_statuses() {
    let positive = [307902011, 307902021, 307902031, 307902041];
    assert_eq!(six_round_end_power(&positive, &[]), 3);
    assert_eq!(six_round_end_power(&positive[..2], &positive[2..]), 1);
}

fn six_passive_attack_damage(with_passive: bool, negative_status: bool) -> i32 {
    init_config();
    let attr = HeroAttribute { hp: Some(100_000), attack: Some(1_000), ..Default::default() };
    let fight = Fight {
        attacker: Some(FightTeam { entitys: vec![FightEntityInfo {
            uid: Some(10), model_id: Some(3079), team_type: Some(1), current_hp: Some(100_000),
            attr: Some(attr), passive_skill: if with_passive { vec![30790144] } else { vec![] },
            skill_group1: vec![30610111], ..Default::default()
        }], ..Default::default() }),
        defender: Some(FightTeam { entitys: vec![FightEntityInfo {
            uid: Some(-1), team_type: Some(2), current_hp: Some(100_000), attr: Some(attr),
            buffs: if negative_status { vec![BuffInfo { uid: Some(20), buff_id: Some(30530103),
                from_uid: Some(10), duration: Some(2), ..Default::default() }] } else { vec![] },
            ..Default::default()
        }], ..Default::default() }), ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.execute_card(CardCommand::Setup(CardSetup { hand: vec![CardInfo {
        uid: Some(10), skill_id: Some(30610111), ..Default::default() }],
        draw_pile: vec![], deck_num: 1 })).unwrap();
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    run_player_action_queue(&mut managers, &pool, &catalog, &mut RoundDeterminism::default(),
        TargetContext::default(), [CardPlay { origin: CARD_PLAY_ORIGIN, hand_index: 0,
            target_uid: Some(-1), chosen_skill_id: None, choice: None, recorded_skill: None }],
        1, 99998).unwrap();
    100_000 - managers.hp.current(-1)
}

#[test]
fn six_passive_damage_bonus_requires_a_negative_status_on_the_target() {
    let baseline = six_passive_attack_damage(false, true);
    let boosted = six_passive_attack_damage(true, true);
    let no_status = six_passive_attack_damage(true, false);
    assert!((boosted * 100 - baseline * 124).abs() <= 100,
        "baseline={baseline}, boosted={boosted}");
    assert_eq!(no_status, baseline);
}

fn cast_duty(skill_id: i32, target_power: i32, target_max: i32, buffs: &[i32]) -> (BattleManagers, DrainResult) {
    init_config();
    let attr = HeroAttribute { hp: Some(100_000), attack: Some(1_000), ..Default::default() };
    let fight = Fight {
        attacker: Some(FightTeam { entitys: vec![
            FightEntityInfo { uid: Some(10), model_id: Some(3079), team_type: Some(1), current_hp: Some(100_000),
                attr: Some(attr), power_infos: vec![PowerInfo { power_id: Some(1), num: Some(5), max: Some(5) }], ..Default::default() },
            FightEntityInfo { uid: Some(11), model_id: Some(3141), team_type: Some(1), current_hp: Some(100_000),
                attr: Some(attr), power_infos: vec![PowerInfo { power_id: Some(1), num: Some(target_power), max: Some(target_max) }],
                buffs: buffs.iter().enumerate().map(|(index, id)| BuffInfo { uid: Some(100 + index as i64),
                    buff_id: Some(*id), from_uid: Some(10), duration: Some(2), ..Default::default() }).collect(), ..Default::default() },
            crate::engine::manager::emitter::activation_entity(),
        ], ..Default::default() }),
        defender: Some(FightTeam { entitys: vec![FightEntityInfo { uid: Some(-1), team_type: Some(2), current_hp: Some(100_000), attr: Some(attr), ..Default::default() }], ..Default::default() }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.execute_card(CardCommand::Setup(CardSetup { hand: vec![CardInfo { uid: Some(10), skill_id: Some(skill_id), ..Default::default() }],
        draw_pile: Vec::new(), deck_num: 1 })).unwrap();
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [skill_id], buffs.iter().copied());
    let mut determinism = RoundDeterminism::default();
    // If missing types exist, these captured choices must not displace them.
    determinism.enqueue_random_buffs([307902011, 307902021]);
    let result = run_player_action_queue(&mut managers, &pool, &catalog, &mut determinism, TargetContext::default(),
        [CardPlay { origin: CARD_PLAY_ORIGIN, hand_index: 0, target_uid: Some(11), chosen_skill_id: None, choice: None, recorded_skill: None }], 1, 99998).unwrap();
    (managers, result)
}

#[test]
fn six_euphoria_duty_grants_the_selected_ally_eureka_and_configured_buff_count() {
    for (skill, count) in [(307901213, 2), (307901223, 4), (307901233, 6),
        (307901243, 2), (307901253, 4), (307901263, 6)] {
        let (managers, result) = cast_duty(skill, 1, 10, &[]);
        assert_eq!(managers.eureka.get(11, 1).current, 10, "skill {skill}: events {:?}", result.events);
        assert_eq!(managers.eureka.get(10, 1).current, 5, "caster's eureka must not be spent");
        let types = [1014, 2140001, 1008, 1029, 1022, 307902061];
        let found = types.iter().filter(|id| managers.buff.has_buff_type(11, **id)).count();
        assert_eq!(found, count, "skill {skill}: expected {count} distinct buffs; events {:?}", result.events);
        fn has_change(step: &sonettobuf::FightStep, delta: i32) -> bool {
            step.act_effect.iter().any(|effect| {
                (effect.target_id == Some(11) && effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Powerchange as i32)
                    && effect.effect_num == Some(delta) && effect.config_effect == Some(1))
                    || effect.fight_step.as_ref().is_some_and(|child| has_change(child, delta))
            })
        }
        let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
        assert!(steps.iter().any(|step| has_change(step, 9)), "skill {skill} must notify the client's selected target");
        if count == 6 { assert!(managers.buff.has_buff_id(11, 5023), "skill {skill} should empower a complete set"); }
    }
    let (capped, _) = cast_duty(307901213, 2, 5, &[]);
    assert_eq!(capped.eureka.get(11, 1).current, 5);
}

#[test]
fn six_euphoria_duty_prefers_missing_buff_types_and_empowers_a_full_collection() {
    let existing = [307902011, 307902021, 307902031, 307902041, 307902051];
    let (managers, result) = cast_duty(307901213, 0, 10, &existing);
    assert!(managers.buff.has_buff_type(11, 307902061), "missing sixth buff should be selected first: {:?}", result.events);
    assert!(managers.buff.has_buff_id(11, 5022), "full collection should grant empowerment: {:?}", result.events);
    assert_eq!(managers.eureka.get(11, 1).current, 10);
}

#[test]
fn six_euphoria_duty_dispels_two_negative_types() {
    let debuffs = [30530104, 30530103, 300402];
    let (managers, _) = cast_duty(307901213, 0, 5, &debuffs);
    assert_eq!(debuffs.iter().filter(|id| managers.buff.has_buff_id(11, **id)).count(), 1);
}

#[test]
fn six_euphoria_debuff_card_applies_full_collection_poison_burn_and_eureka() {
    init_config();
    let attr = HeroAttribute { hp: Some(100_000), attack: Some(1_000), ..Default::default() };
    let fight = Fight {
        attacker: Some(FightTeam { entitys: vec![FightEntityInfo { uid: Some(10), model_id: Some(3079),
            team_type: Some(1), current_hp: Some(100_000), attr: Some(attr),
            power_infos: vec![PowerInfo { power_id: Some(1), num: Some(1), max: Some(5) }], ..Default::default() },
            crate::engine::manager::emitter::activation_entity()], ..Default::default() }),
        defender: Some(FightTeam { entitys: vec![FightEntityInfo { uid: Some(-1), team_type: Some(2),
            current_hp: Some(100_000), attr: Some(attr), ..Default::default() }], ..Default::default() }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.execute_card(CardCommand::Setup(CardSetup { hand: vec![CardInfo { uid: Some(10), skill_id: Some(307901133), ..Default::default() }],
        draw_pile: Vec::new(), deck_num: 1 })).unwrap();
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [307901133], []);
    let result = run_player_action_queue(&mut managers, &pool, &catalog, &mut RoundDeterminism::default(), TargetContext::default(),
        [CardPlay { origin: CARD_PLAY_ORIGIN, hand_index: 0, target_uid: Some(-1), chosen_skill_id: None, choice: None, recorded_skill: None }], 1, 99998).unwrap();
    assert_eq!(managers.eureka.get(10, 1).current, 4, "events {:?}", result.events);
    for kind in [2007, 2005, 2003, 2009, 2002, 2140003] {
        assert!(managers.buff.has_buff_type(-1, kind), "missing debuff {kind}: {:?}", result.events);
    }
    assert_eq!(managers.buff.buff_id_amount(-1, 307901071), 6, "poison {:?}", result.events);
    assert_eq!(managers.buff.buff_id_amount(-1, 4150001), 9, "burn {:?}", result.events);
}

#[test]
fn six_euphoria_duty_fills_the_real_evolved_voyager_eureka_pool() {
    init_config();
    use crate::engine::entity::{builder::EntityBuilder, input::HeroBuildInput};
    let mut target = EntityBuilder::new(HeroBuildInput { uid: 11, hero_id: 3048,
        level: 180, rank: 4, destiny_stone: 304801, destiny_rank: 4, ..Default::default() }, 2, 1, false).build().unwrap();
    assert_eq!((target.power_infos[0].num, target.power_infos[0].max), (Some(0), Some(5)));
    target.passive_skill.clear();
    let attr = HeroAttribute { hp: Some(100_000), attack: Some(1_000), ..Default::default() };
    let fight = Fight { attacker: Some(FightTeam { entitys: vec![
        FightEntityInfo { uid: Some(10), model_id: Some(3079), team_type: Some(1), current_hp: Some(100_000),
            attr: Some(attr), power_infos: vec![PowerInfo { power_id: Some(1), num: Some(5), max: Some(5) }], ..Default::default() },
        target, crate::engine::manager::emitter::activation_entity()], ..Default::default() }),
        defender: Some(FightTeam { entitys: vec![FightEntityInfo { uid: Some(-1), team_type: Some(2),
            current_hp: Some(100_000), attr: Some(attr), ..Default::default() }], ..Default::default() }), ..Default::default() };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.execute_card(CardCommand::Setup(CardSetup { hand: vec![CardInfo { uid: Some(10), skill_id: Some(307901213), ..Default::default() }],
        draw_pile: Vec::new(), deck_num: 1 })).unwrap();
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [307901213], []);
    let result = run_player_action_queue(&mut managers, &pool, &catalog, &mut RoundDeterminism::default(), TargetContext::default(),
        [CardPlay { origin: CARD_PLAY_ORIGIN, hand_index: 0, target_uid: Some(11), chosen_skill_id: None, choice: None, recorded_skill: None }], 1, 99998).unwrap();
    assert_eq!(managers.eureka.get(11, 1).current, 5);
    assert!(result.events.iter().any(|event| matches!(event, BattleEvent::EurekaChanged(change)
        if change.source_uid == 10 && change.target_uid == 11 && change.requested_delta == 10 && change.applied_delta == 5)));
}
