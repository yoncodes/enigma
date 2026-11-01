use super::*;

#[test]
fn myth_assist_configured_active_skills_execute_with_system_talents() {
    crate::test_support::init_config();
    use crate::dungeon::{BattleFighter, BattleRoster, FightOptions, HeroBuildInput};
    let db = crate::test_support::game_data();
    let catalog = crate::catalog::BattleCatalog::new(db);
    let fighters = [(3041, 304101), (3034, 303401), (3066, 306601), (3127, 312701)]
        .into_iter().enumerate().map(|(i, (hero_id, skin))| BattleFighter {
            hero: HeroBuildInput { uid: 30_000_001 + i as i64, user_id: 1, hero_id, skin,
                level: 180, rank: 4, ex_skill_level: 5, talent: 15, ..Default::default() },
            equips: vec![],
        }).collect::<Vec<_>>();
    let roster = BattleRoster { user_id: 1, fighters, ..Default::default() };
    let mut failures = Vec::new();
    for boss in db.tower_assist_boss.iter() {
        for plan in db.tower_talent_plan.iter().filter(|plan| plan.boss_id == boss.boss_id) {
            let talents = crate::tower::system_plan_talents(db, boss.boss_id, 10, &plan.talent_ids);
            let group = sonettobuf::FightGroup {
                hero_list: roster.fighters.iter().map(|fighter| fighter.hero.uid).collect(),
                assist_boss_id: Some(boss.boss_id), ..Default::default()
            };
            let skill_ids = boss.active_skills.split('|').filter_map(|entry|
                entry.split('#').next().and_then(|id| id.parse::<i32>().ok())).collect::<Vec<_>>();
            for skill_id in skill_ids {
              for mixed in [false, true] {
                let result = (|| -> anyhow::Result<()> {
                    let mut built = crate::dungeon::build_fight(catalog, &roster, 90000101, 9000011,
                        &group, FightOptions::default(), None)?;
                    crate::tower::apply_assist_boss(db, 1, boss.boss_id, 10, &talents, &mut built)?;
                    let team = built.fight.attacker.as_mut().unwrap();
                    let cost = team.assist_boss_info.as_ref().unwrap().skills.iter()
                        .find(|skill| skill.skill_id == Some(skill_id)).unwrap().need_power.unwrap_or_default();
                    let assist = team.assist_boss.as_mut().unwrap();
                    for power in &mut assist.power_infos {
                        power.num = if cost > 0 { Some(cost) } else { power.max };
                    }
                    for enemy in &mut built.fight.defender.as_mut().unwrap().entitys {
                        enemy.current_hp = Some(10_000_000);
                        if let Some(attr) = &mut enemy.attr { attr.hp = Some(10_000_000); }
                    }
                    let enemy = built.fight.defender.as_ref().unwrap().entitys[0].uid.unwrap();
                    let rules = std::mem::take(&mut built.battle_rule_skills);
                    let mut runtime = BattleRuntime::new_with_attributes(catalog, built.fight,
                        built.ex_attributes, built.sp_attributes);
                    runtime.extend_battle_rule_skills(rules);
                    runtime.start_round_with_determinism(RoundDeterminism::with_seed(43))
                        .map_err(|e| anyhow::anyhow!("opening: {e}"))?;
                    let mut opers = Vec::new();
                    let mut played_skill = None;
                    if mixed {
                        let (index, card) = runtime.managers.card.hand().iter().enumerate()
                            .find(|(_, card)| card.skill_id.is_some_and(|id| catalog.skill_is_attack(id)
                                && !catalog.skill_is_big(id)) && !card.temp_card.unwrap_or_default())
                            .ok_or_else(|| anyhow::anyhow!("no normal attack card"))?;
                        played_skill = card.skill_id;
                        opers.push(BeginRoundOper { oper_type: Some(2), param1: Some(index as i32 + 1),
                            to_id: Some(enemy), ..Default::default() });
                    }
                    opers.push(BeginRoundOper { oper_type: Some(4), param1: Some(skill_id),
                        to_id: Some(enemy), ..Default::default() });
                    let round = runtime.build_begin_round_from_schedule(&BeginRoundRequest {
                        opers, ..Default::default()
                    }).map_err(|e| anyhow::anyhow!("assist action: {e}"))?;
                    anyhow::ensure!(round.fight_step.iter().any(|step|
                        step.from_id == Some(-1) && step.act_id == Some(skill_id)),
                        "assist skill did not appear in the returned battle steps");
                    if boss.boss_id == 4 && !mixed {
                        anyhow::ensure!(runtime.managers.hp.current(enemy) < 10_000_000,
                            "Ashen Beast's incantation did not damage its target");
                    }
                    if let Some(played_skill) = played_skill {
                        anyhow::ensure!(round.fight_step.iter().any(|step| step.act_id == Some(played_skill)),
                            "mixed queue lost the played hand card");
                    }
                    Ok(())
                })();
                if let Err(error) = result { failures.push(format!(
                    "boss={} plan={} skill={} mixed={mixed}: {error:#}", boss.boss_id, plan.plan_id, skill_id)); }
              }
            }
        }
    }
    assert!(failures.is_empty(), "assist casts that failed:\n{}", failures.join("\n"));
}

#[test]
fn ashen_beast_two_casts_upgrade_the_attack_in_the_same_round() {
    crate::test_support::init_config();
    use crate::dungeon::{BattleFighter, BattleRoster, FightOptions, HeroBuildInput};
    let db = crate::test_support::game_data();
    let catalog = crate::catalog::BattleCatalog::new(db);
    let fighters = [(3041, 304101), (3034, 303401), (3066, 306601), (3127, 312701)]
        .into_iter().enumerate().map(|(i, (hero_id, skin))| BattleFighter {
            hero: HeroBuildInput { uid: 30_000_001 + i as i64, user_id: 1, hero_id, skin,
                level: 180, rank: 4, ex_skill_level: 5, talent: 15, ..Default::default() },
            equips: vec![],
        }).collect::<Vec<_>>();
    let group = sonettobuf::FightGroup {
        hero_list: fighters.iter().map(|fighter| fighter.hero.uid).collect(),
        assist_boss_id: Some(4), ..Default::default()
    };
    let roster = BattleRoster { user_id: 1, fighters, ..Default::default() };
    let mut built = crate::dungeon::build_fight(catalog, &roster, 90000101, 9000011,
        &group, FightOptions::default(), None).unwrap();
    crate::tower::apply_assist_boss(db, 1, 4, 10, &[], &mut built).unwrap();
    let assist = built.fight.attacker.as_mut().unwrap().assist_boss.as_mut().unwrap();
    for power in &mut assist.power_infos { power.num = power.max; }
    let enemy = built.fight.defender.as_ref().unwrap().entitys[0].uid.unwrap();
    let mut runtime = BattleRuntime::new_with_attributes(catalog, built.fight,
        built.ex_attributes, built.sp_attributes);
    runtime.start_round_with_determinism(RoundDeterminism::with_seed(43)).unwrap();
    let action = || BeginRoundOper { oper_type: Some(4), param1: Some(12410010),
        to_id: Some(enemy), ..Default::default() };
    let round = runtime.build_begin_round_from_schedule(&BeginRoundRequest {
        opers: vec![action(), action()], ..Default::default()
    }).unwrap();
    fn collect(step: &sonettobuf::FightStep, ids: &mut Vec<i32>) {
        if step.from_id == Some(-1) { ids.extend(step.act_id); }
        for nested in step.act_effect.iter().filter_map(|effect| effect.fight_step.as_ref()) {
            collect(nested, ids);
        }
    }
    let mut ids = Vec::new();
    for step in &round.fight_step { collect(step, &mut ids); }
    assert!(ids.contains(&12410011) && ids.contains(&12410012), "actual skills: {ids:?}");
}

#[test]
fn primitive_urge_extends_its_array_for_each_extra_power_spent() {
    crate::test_support::init_config();
    use crate::dungeon::{BattleFighter, BattleRoster, FightOptions, HeroBuildInput};
    let db = crate::test_support::game_data();
    let catalog = crate::catalog::BattleCatalog::new(db);
    let fighters = [(3041, 304101), (3034, 303401), (3066, 306601), (3127, 312701)]
        .into_iter().enumerate().map(|(i, (hero_id, skin))| BattleFighter {
            hero: HeroBuildInput { uid: 30_000_001 + i as i64, user_id: 1, hero_id, skin,
                level: 180, rank: 4, ex_skill_level: 5, talent: 15, ..Default::default() },
            equips: vec![],
        }).collect::<Vec<_>>();
    let group = sonettobuf::FightGroup {
        hero_list: fighters.iter().map(|fighter| fighter.hero.uid).collect(),
        assist_boss_id: Some(5), ..Default::default()
    };
    let roster = BattleRoster { user_id: 1, fighters, ..Default::default() };
    let mut durations = Vec::new();
    for power_value in 3..=5 {
        let mut built = crate::dungeon::build_fight(catalog, &roster, 90000101, 9000011,
            &group, FightOptions::default(), None).unwrap();
        crate::tower::apply_assist_boss(db, 1, 5, 10, &[], &mut built).unwrap();
        let assist = built.fight.attacker.as_mut().unwrap().assist_boss.as_mut().unwrap();
        assist.power_infos[0].num = Some(power_value);
        for enemy in &mut built.fight.defender.as_mut().unwrap().entitys {
            enemy.current_hp = Some(10_000_000);
            if let Some(attr) = &mut enemy.attr { attr.hp = Some(10_000_000); }
        }
        let enemy = built.fight.defender.as_ref().unwrap().entitys[0].uid.unwrap();
        let mut runtime = BattleRuntime::new_with_attributes(catalog, built.fight,
            built.ex_attributes, built.sp_attributes);
        runtime.start_round_with_determinism(RoundDeterminism::with_seed(43)).unwrap();
        runtime.build_begin_round_from_schedule(&BeginRoundRequest {
            opers: vec![BeginRoundOper { oper_type: Some(4), param1: Some(1251001),
                to_id: Some(enemy), ..Default::default() }], ..Default::default()
        }).unwrap();
        assert_eq!(runtime.managers.eureka.get(-1, 4).current, 0);
        durations.push(runtime.managers.field.get(1).expect("array deployed").definition.duration);
        fn collect(step: &sonettobuf::FightStep, ids: &mut Vec<i32>) {
            if step.from_id == Some(-1) { ids.extend(step.act_id); }
            for child in step.act_effect.iter().filter_map(|effect| effect.fight_step.as_ref()) {
                collect(child, ids);
            }
        }
        let mut followups = Vec::new();
        for _ in 0..5 {
            if runtime.managers.field.get(1).is_none() { break; }
            let round = runtime.build_begin_round_from_schedule(&BeginRoundRequest::default()).unwrap();
            for step in &round.fight_step { collect(step, &mut followups); }
        }
        assert!(runtime.managers.field.get(1).is_none(), "array did not expire");
        assert!(followups.contains(&12510011), "array expiry did not cast its ultimate: {followups:?}");
        assert!(runtime.managers.hp.current(enemy) < 10_000_000, "array ultimate dealt no damage");
    }
    assert_eq!(durations[1] - durations[0], 1, "durations: {durations:?}");
    assert_eq!(durations[2] - durations[0], 2, "durations: {durations:?}");
}
