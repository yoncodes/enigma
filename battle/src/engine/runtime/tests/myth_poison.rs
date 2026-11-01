use super::*;

#[test]
fn myth_100m_three_poison_team_can_cast_across_rounds() {
    crate::test_support::init_config();
    use crate::dungeon::{BattleFighter, BattleRoster, FightOptions, HeroBuildInput};
    use crate::engine::manager::card::{CardCommand, pool::card_for};

    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let fighters = [(3142, 314201), (3098, 309801), (3104, 310401), (3127, 312701)]
        .into_iter().enumerate().map(|(i, (hero_id, skin))| BattleFighter {
            hero: HeroBuildInput { uid: 30_000_001 + i as i64, user_id: 1, hero_id, skin,
                level: 180, rank: 4, ex_skill_level: 5, talent: 15, ..Default::default() },
            equips: vec![],
        }).collect::<Vec<_>>();
    let group = sonettobuf::FightGroup {
        hero_list: fighters.iter().map(|fighter| fighter.hero.uid).collect(), ..Default::default()
    };
    let roster = BattleRoster { user_id: 1, fighters, ..Default::default() };
    let mut built = crate::dungeon::build_fight(catalog, &roster, 90001003, 9000103,
        &group, FightOptions::default(), None).unwrap();
    // Keep actual stats and effects, increasing only HP to make several turns observable.
    for hero in &mut built.fight.attacker.as_mut().unwrap().entitys {
        hero.current_hp = Some(10_000_000);
        if let Some(attr) = hero.attr.as_mut() { attr.hp = Some(10_000_000); }
    }
    for enemy in &mut built.fight.defender.as_mut().unwrap().entitys {
        enemy.current_hp = Some(10_000_000);
        if let Some(attr) = enemy.attr.as_mut() { attr.hp = Some(10_000_000); }
    }
    let rules = std::mem::take(&mut built.battle_rule_skills);
    let mut runtime = BattleRuntime::new_with_attributes(catalog, built.fight,
        built.ex_attributes, built.sp_attributes);
    runtime.extend_battle_rule_skills(rules);
    runtime.start_round_with_determinism(RoundDeterminism::with_seed(43))
        .unwrap_or_else(|e| panic!("opening failed: {e}"));

    for (turn, (owner, skill)) in [(30_000_002, 30980111), (30_000_001, 31420111),
        (30_000_003, 31040111), (30_000_004, 31270111), (30_000_003, 31040121)]
        .into_iter().enumerate()
    {
        let hero = runtime.fight.attacker.as_ref().unwrap().entitys.iter()
            .find(|hero| hero.uid == Some(owner)).unwrap();
        let card = card_for(hero, Some(skill)).unwrap();
        runtime.managers.execute_card(CardCommand::Setup(CardSetup {
            hand: vec![card], draw_pile: vec![], deck_num: 0,
        })).unwrap();
        let enemy = runtime.fight.defender.as_ref().unwrap().entitys.iter()
            .filter_map(|enemy| enemy.uid)
            .find(|uid| runtime.managers.hp.current(*uid) > 0).unwrap();
        runtime.build_begin_round_from_schedule(&BeginRoundRequest {
            opers: vec![BeginRoundOper { oper_type: Some(2), param1: Some(1),
                to_id: Some(enemy), ..Default::default() }], ..Default::default()
        }).unwrap_or_else(|e| panic!("turn {turn} skill {skill} failed: {e}"));
    }
}

fn configured_poison_fight(weak_enemies: bool, evolved: bool) -> BattleRuntime {
    crate::test_support::init_config();
    use crate::dungeon::{BattleFighter, BattleRoster, FightOptions, HeroBuildInput};
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let fighters = [(3142, 314201), (3098, 309801), (3104, 310401), (3127, 312701)]
        .into_iter().enumerate().map(|(i, (hero_id, skin))| BattleFighter {
            hero: HeroBuildInput { uid: 30_000_001 + i as i64, user_id: 1, hero_id, skin,
                level: 180, rank: 4, ex_skill_level: 5, talent: 15,
                destiny_stone: if evolved && matches!(hero_id, 3098 | 3104) { hero_id * 100 + 1 } else { 0 },
                destiny_rank: if evolved && matches!(hero_id, 3098 | 3104) { 4 } else { 0 },
                ..Default::default() },
            equips: vec![],
        }).collect::<Vec<_>>();
    let group = sonettobuf::FightGroup {
        hero_list: fighters.iter().map(|fighter| fighter.hero.uid).collect(), ..Default::default()
    };
    let roster = BattleRoster { user_id: 1, fighters, ..Default::default() };
    let mut built = crate::dungeon::build_fight(catalog, &roster, 90001003, 9000103,
        &group, FightOptions::default(), None).unwrap();
    for hero in &mut built.fight.attacker.as_mut().unwrap().entitys {
        hero.current_hp = Some(10_000_000);
        if let Some(attr) = hero.attr.as_mut() { attr.hp = Some(10_000_000); }
    }
    for (i, enemy) in built.fight.defender.as_mut().unwrap().entitys.iter_mut().enumerate() {
        if weak_enemies {
            let max_hp = enemy.attr.as_ref().and_then(|attr| attr.hp).unwrap();
            enemy.current_hp = Some(if i == 0 { 1 } else { max_hp / (i as i32 + 1) });
        } else {
            enemy.current_hp = Some(10_000_000);
            if let Some(attr) = enemy.attr.as_mut() { attr.hp = Some(10_000_000); }
        }
    }
    let rules = std::mem::take(&mut built.battle_rule_skills);
    let mut runtime = BattleRuntime::new_with_attributes(catalog, built.fight,
        built.ex_attributes, built.sp_attributes);
    runtime.extend_battle_rule_skills(rules);
    runtime.start_round_with_determinism(RoundDeterminism::with_seed(43)).unwrap();
    runtime
}

#[test]
fn myth_100m_three_real_hands_resolve_multiple_poison_actions() {
    let mut runtime = configured_poison_fight(false, false);
    let mut observed = std::collections::HashSet::new();
    assert!(runtime.managers.card.hand().iter().any(|card| card.enchants.iter()
        .any(|enchant| enchant.enchant_id == Some(10002))), "stage must apply Scalding");
    for turn in 0..2 {
        let enemy = runtime.fight.defender.as_ref().unwrap().entitys.iter()
            .filter_map(|enemy| enemy.uid)
            .find(|uid| runtime.managers.hp.current(*uid) > 0).unwrap();
        let mut eligible = runtime.managers.card.hand().iter().enumerate()
            .filter(|(_, card)| card.uid.is_some_and(|uid| (30_000_001..=30_000_004).contains(&uid))
                && card.skill_id.is_some_and(|id| {
                    let base = id / 10;
                    [3142011, 3142012, 3098011, 3098012, 3104011, 3104012, 3127011].contains(&base)
                }))
            .map(|(index, card)| { observed.insert(card.uid.unwrap()); index }).take(4).collect::<Vec<_>>();
        assert!(!eligible.is_empty(), "no usable hand cards in turn {turn}");
        eligible.sort_unstable_by(|a, b| b.cmp(a));
        let opers = eligible.into_iter().map(|index| BeginRoundOper { oper_type: Some(2),
            param1: Some(index as i32 + 1), to_id: Some(enemy), ..Default::default() }).collect();
        runtime.build_begin_round_from_schedule(&BeginRoundRequest { opers, ..Default::default() })
            .unwrap_or_else(|e| panic!("natural hand turn {turn} failed: {e}"));
    }
    assert!(observed.len() >= 2, "test should exercise multiple poison-team members: {observed:?}");
}

#[test]
fn myth_100m_three_death_heals_survivors_once_for_25_percent_missing_hp() {
    use crate::engine::manager::card::{CardCommand, pool::card_for};
    let mut runtime = configured_poison_fight(true, false);
    let enemies = runtime.fight.defender.as_ref().unwrap().entitys.iter()
        .map(|enemy| enemy.uid.unwrap()).collect::<Vec<_>>();
    assert_eq!(enemies.len(), 3);
    let before = enemies.iter().map(|uid| runtime.managers.hp.get(*uid)).collect::<Vec<_>>();
    let hero = runtime.fight.attacker.as_ref().unwrap().entitys.iter()
        .find(|hero| hero.model_id == Some(3127)).unwrap();
    let card = card_for(hero, Some(31270111)).unwrap();
    runtime.managers.execute_card(CardCommand::Setup(CardSetup {
        hand: vec![card], draw_pile: vec![], deck_num: 0,
    })).unwrap();
    let round = runtime.build_begin_round_from_schedule(&BeginRoundRequest {
        opers: vec![BeginRoundOper { oper_type: Some(2), param1: Some(1),
            to_id: Some(enemies[0]), ..Default::default() }], ..Default::default()
    }).unwrap();
    assert_eq!(runtime.managers.hp.current(enemies[0]), 0);
    fn collect_heals(step: &sonettobuf::FightStep, heals: &mut Vec<(i64, i32)>) {
        for effect in &step.act_effect {
            if step.act_id == Some(5230471) && effect.effect_type == Some(4)
                && let (Some(target), Some(amount)) = (effect.target_id, effect.effect_num)
            {
                heals.push((target, amount));
            }
            if let Some(child) = &effect.fight_step { collect_heals(child, heals); }
        }
    }
    let mut heals = Vec::new();
    for step in &round.fight_step { collect_heals(step, &mut heals); }
    assert_eq!(heals.len(), 2, "only the two survivors receive one heal each: {heals:?}");
    for i in 1..3 {
        let amount = heals.iter().find(|(uid, _)| *uid == enemies[i]).unwrap().1;
        let unmodified_heal = (before[i].max - before[i].current) / 4;
        // This team inflicts Burn, which reduces healing received by 15%.
        assert_eq!(amount, unmodified_heal * 850 / 1000, "survivor {i}, before={:?}", before[i]);
    }
}

#[test]
fn myth_100m_three_poison_team_ultimates_execute_in_a_shared_fight() {
    use crate::engine::manager::card::{CardCommand, pool::card_for};
    let mut runtime = configured_poison_fight(false, false);
    for (owner, skill) in [(30_000_002, 30980131), (30_000_001, 31420131),
        (30_000_003, 31040131), (30_000_004, 31270131)]
    {
        runtime.managers.ex_point.set(owner, owner, 5, 0);
        let hero = runtime.fight.attacker.as_ref().unwrap().entitys.iter()
            .find(|hero| hero.uid == Some(owner)).unwrap();
        let card = card_for(hero, Some(skill)).unwrap();
        runtime.managers.execute_card(CardCommand::Setup(CardSetup {
            hand: vec![card], draw_pile: vec![], deck_num: 0,
        })).unwrap();
        let enemy = runtime.fight.defender.as_ref().unwrap().entitys.iter()
            .filter_map(|enemy| enemy.uid)
            .find(|uid| runtime.managers.hp.current(*uid) > 0).unwrap();
        let target = if skill == 31040131 { owner } else { enemy };
        let round = runtime.build_begin_round_from_schedule(&BeginRoundRequest {
            opers: vec![BeginRoundOper { oper_type: Some(2), param1: Some(1),
                to_id: Some(target), ..Default::default() }], ..Default::default()
        }).unwrap_or_else(|e| panic!("ultimate {skill} failed: {e}"));
        fn contains(step: &sonettobuf::FightStep, skill: i32) -> bool {
            step.act_id == Some(skill) || step.act_effect.iter()
                .filter_map(|effect| effect.fight_step.as_ref()).any(|step| contains(step, skill))
        }
        assert!(round.fight_step.iter().any(|step| contains(step, skill)), "ultimate {skill} was skipped");
    }
}

#[test]
fn myth_100m_three_evolved_poison_team_casts_normal_and_ultimate_skills() {
    use crate::engine::manager::card::{CardCommand, pool::card_for};
    let mut runtime = configured_poison_fight(false, true);
    for (owner, group) in [(30_000_002, 1), (30_000_001, 1), (30_000_003, 1),
        (30_000_002, 2), (30_000_003, 2), (30_000_004, 1), (30_000_002, 3), (30_000_003, 3)]
    {
        let hero = runtime.fight.attacker.as_ref().unwrap().entitys.iter()
            .find(|hero| hero.uid == Some(owner)).unwrap();
        let skill = match group {
            1 => *hero.skill_group1.first().unwrap(),
            2 => *hero.skill_group2.first().unwrap(),
            _ => hero.ex_skill.unwrap(),
        };
        let mut card = card_for(hero, Some(skill)).unwrap();
        card.enchants.push(sonettobuf::CardEnchant { enchant_id: Some(10002), duration: Some(-1),
            ..Default::default() });
        if group == 3 { runtime.managers.ex_point.set(owner, owner, 5, 0); }
        runtime.managers.execute_card(CardCommand::Setup(CardSetup {
            hand: vec![card], draw_pile: vec![], deck_num: 0,
        })).unwrap();
        let enemy = runtime.fight.defender.as_ref().unwrap().entitys.iter()
            .filter_map(|enemy| enemy.uid)
            .find(|uid| runtime.managers.hp.current(*uid) > 0).unwrap();
        let target = if owner == 30_000_003 && group == 3 { owner } else { enemy };
        let round = runtime.build_begin_round_from_schedule(&BeginRoundRequest {
            opers: vec![BeginRoundOper { oper_type: Some(2), param1: Some(1),
                to_id: Some(target), ..Default::default() }], ..Default::default()
        }).unwrap_or_else(|e| panic!("evolved owner={owner} skill={skill} failed: {e}"));
        fn contains(step: &sonettobuf::FightStep, skill: i32) -> bool {
            step.act_id == Some(skill) || step.act_effect.iter()
                .filter_map(|effect| effect.fight_step.as_ref()).any(|child| contains(child, skill))
        }
        assert!(round.fight_step.iter().any(|step| contains(step, skill)), "evolved skill {skill} was skipped");
    }
}
