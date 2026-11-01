use super::*;

#[test]
fn compose_default_research_applies_its_level_bonus_once_to_each_ally() {
    crate::test_support::init_config();
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let mut built = crate::dungeon::BuiltFight {
        fight: Fight {
            battle_id: Some(0), episode_id: Some(90400199), version: Some(7), cur_round: Some(1),
            attacker: Some(FightTeam {
                entitys: (1..=4).map(|uid| FightEntityInfo {
                    uid: Some(uid), model_id: Some(3088), team_type: Some(1), position: Some(uid as i32),
                    current_hp: Some(1000),
                    attr: Some(sonettobuf::HeroAttribute { hp: Some(1000), attack: Some(100), ..Default::default() }),
                    skill_group1: vec![30880111, 30880112, 30880113],
                    skill_group2: vec![30880121, 30880122, 30880123],
                    ex_skill: Some(30880131), ..Default::default()
                }).collect(), ..Default::default()
            }),
            defender: Some(FightTeam { entitys: vec![FightEntityInfo {
                uid: Some(-1), team_type: Some(2), current_hp: Some(1000), ..Default::default()
            }], ..Default::default() }), ..Default::default()
        },
        ex_attributes: vec![], sp_attributes: vec![], battle_rule_skills: vec![],
    };
    let research = crate::dungeon::compose::ComposeFightData {
        theme_id: 1, layer_id: 3, plane_id: 1, max_plane_id: 1, research_id: 1400,
        mods: vec![sonettobuf::TowerComposeMods { r#type: Some(3), mods: vec![
            sonettobuf::TowerComposeMod { slot: Some(1), mod_id: Some(11601) }
        ] }],
    };
    crate::dungeon::compose::apply_to_built(catalog, &mut built, &research).unwrap();
    let mut runtime = BattleRuntime::new(catalog, built.fight);
    runtime.start_round_with_determinism(RoundDeterminism::with_seed(29)).unwrap();
    for uid in 1..=4 {
        assert_eq!(runtime.managers.buff.buff_type_amount(uid, 23390261), 1, "ally {uid}");
        assert_eq!(runtime.managers.buff.active_for(uid)
            .filter(|buff| buff.buff_id == Some(23390265)).count(), 1, "ally {uid}");
    }
}

#[test]
fn compose_portrait_research_distinguishes_new_and_original_max_levels() {
    crate::test_support::init_config();
    use crate::dungeon::{BattleFighter, BattleRoster, FightOptions, HeroBuildInput};
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let specs = [(3088, 308801, 4), (3125, 312501, 5), (3126, 312601, 1)];
    let fighters = specs.into_iter().enumerate().map(|(index, (hero_id, skin, ex_skill_level))| {
        BattleFighter { hero: HeroBuildInput { uid: 30_000_001 + index as i64, user_id: 1,
            hero_id, skin, level: 180, rank: 4, talent: 15, ex_skill_level, ..Default::default()
        }, equips: vec![] }
    }).collect::<Vec<_>>();
    let group = sonettobuf::FightGroup {
        hero_list: fighters.iter().map(|fighter| fighter.hero.uid).collect(), ..Default::default()
    };
    let roster = BattleRoster { user_id: 1, fighters, ..Default::default() };
    let params = r#"{"themeId":1,"layerId":6,"planeId":1,"supportId":0,"researchId":1403}"#;
    let mut built = crate::dungeon::build_fight(catalog, &roster, 90400199, 9001001,
        &group, FightOptions::default(), Some(params)).unwrap();
    let data = crate::dungeon::compose::ComposeFightData {
        theme_id: 1, layer_id: 6, plane_id: 1, max_plane_id: 2, research_id: 1403,
        mods: vec![],
    };
    crate::dungeon::compose::apply_to_built(catalog, &mut built, &data).unwrap();
    for enemy in &mut built.fight.defender.as_mut().unwrap().entitys {
        enemy.skill_group1.clear(); enemy.skill_group2.clear(); enemy.passive_skill.clear();
        enemy.ex_skill = Some(0);
    }
    let mut runtime = BattleRuntime::new_with_attributes(catalog, built.fight,
        built.ex_attributes, built.sp_attributes);
    runtime.start_round_with_determinism(RoundDeterminism::with_seed(29)).unwrap();
    let has_buff = |uid, id| runtime.managers.buff.active_for(uid).any(|buff| buff.buff_id == Some(id));
    assert!(has_buff(30_000_001, 113330011));
    assert!(!has_buff(30_000_001, 113330012), "newly upgraded hero must not receive the original-max bonus");
    assert!(!has_buff(30_000_002, 113330011));
    assert!(has_buff(30_000_002, 113330012), "originally maxed hero receives the configured bonus");
    assert!(!has_buff(30_000_003, 113330012));
}

#[test]
fn tides_boss_configured_components_reach_the_opening_round() {
    crate::test_support::init_config();
    use crate::dungeon::{BattleFighter, BattleRoster, FightOptions, HeroBuildInput};
    use sonettobuf::{TowerComposeMod, TowerComposeMods};
    let db = crate::test_support::game_data();
    let catalog = crate::catalog::BattleCatalog::new(db);
    let fighters = [(3041, 304101), (3034, 303401), (3066, 306601), (3127, 312701)]
        .into_iter().enumerate().map(|(i, (hero_id, skin))| BattleFighter {
            hero: HeroBuildInput { uid: 30_000_001 + i as i64, user_id: 1, hero_id, skin,
                level: 180, rank: 4, ex_skill_level: 5, talent: 15, ..Default::default() },
            equips: vec![],
        }).collect::<Vec<_>>();
    let group = sonettobuf::FightGroup {
        hero_list: fighters.iter().map(|fighter| fighter.hero.uid).collect(), ..Default::default()
    };
    let roster = BattleRoster { user_id: 1, fighters, ..Default::default() };
    let params = r#"{"themeId":1,"layerId":6,"planeId":1,"supportId":0,"researchId":1400}"#;
    let mut failures = Vec::new();
    for component in db.tower_compose_mod.iter().filter(|row| row.theme_id == 1) {
        let mut mods = vec![TowerComposeMods { r#type: Some(component.r#type), mods: vec![
            TowerComposeMod { slot: Some(component.slot.max(1)), mod_id: Some(component.id) }
        ] }];
        if component.r#type != 3 {
            mods.push(TowerComposeMods { r#type: Some(3), mods: vec![
                TowerComposeMod { slot: Some(1), mod_id: Some(11601) }
            ] });
        }
        let data = crate::dungeon::compose::ComposeFightData {
            theme_id: 1, layer_id: 6, plane_id: 1, max_plane_id: 2, research_id: 1400, mods,
        };
        let result = (|| -> anyhow::Result<()> {
            let mut built = crate::dungeon::build_fight(catalog, &roster, 90400199, 9001001,
                &group, FightOptions::default(), Some(params))?;
            crate::dungeon::compose::apply_to_built(catalog, &mut built, &data)?;
            let mut runtime = BattleRuntime::new_with_attributes(catalog, built.fight,
                built.ex_attributes, built.sp_attributes);
            runtime.start_round_with_determinism(RoundDeterminism::with_seed(29))
                .map_err(anyhow::Error::msg)?;
            Ok(())
        })();
        if let Err(error) = result { failures.push(format!("{}: {error:#}", component.id)); }
    }
    assert!(failures.is_empty(), "components that failed battle opening:\n{}", failures.join("\n"));
}

#[test]
fn tides_boss_real_cards_can_advance_multiple_rounds() {
    crate::test_support::init_config();
    use crate::dungeon::{BattleFighter, BattleRoster, FightOptions, HeroBuildInput};
    use sonettobuf::{TowerComposeMod, TowerComposeMods};
    let db = crate::test_support::game_data();
    let catalog = crate::catalog::BattleCatalog::new(db);
    let fighters = [(3041, 304101), (3034, 303401), (3066, 306601), (3127, 312701)]
        .into_iter().enumerate().map(|(i, (hero_id, skin))| BattleFighter {
            hero: HeroBuildInput { uid: 30_000_001 + i as i64, user_id: 1, hero_id, skin,
                level: 180, rank: 4, ex_skill_level: 5, talent: 15, ..Default::default() },
            equips: vec![],
        }).collect::<Vec<_>>();
    let group = sonettobuf::FightGroup {
        hero_list: fighters.iter().map(|fighter| fighter.hero.uid).collect(), ..Default::default()
    };
    let roster = BattleRoster { user_id: 1, fighters, ..Default::default() };
    let cases: &[(i32, i32, &[i32])] = &[
        (3, 1, &[11601]),
        (6, 1, &[11101, 11201, 11301, 11415, 11501, 11502, 11503, 11504, 11505, 11601]),
        (6, 2, &[11109, 11601]),
    ];
    for &(layer_id, plane_id, ids) in cases {
        let params = serde_json::json!({"themeId":1,"layerId":layer_id,"planeId":plane_id,
            "supportId":0,"researchId":1400}).to_string();
        let mut built = crate::dungeon::build_fight(catalog, &roster, 90400199, 9001001,
            &group, FightOptions::default(), Some(&params)).unwrap();
        let mods = ids.iter().enumerate().map(|(index, id)| TowerComposeMod {
            slot: Some(index as i32 + 1), mod_id: Some(*id),
        }).collect();
        let data = crate::dungeon::compose::ComposeFightData {
            theme_id: 1, layer_id, plane_id, max_plane_id: if layer_id == 3 { 1 } else { 2 },
            research_id: 1400, mods: vec![TowerComposeMods { r#type: Some(1), mods }],
        };
        crate::dungeon::compose::apply_to_built(catalog, &mut built, &data).unwrap();
        for hero in &mut built.fight.attacker.as_mut().unwrap().entitys {
            hero.current_hp = Some(10_000_000);
            if let Some(attr) = hero.attr.as_mut() { attr.hp = Some(10_000_000); }
            if let Some(attr) = hero.base_attr.as_mut() { attr.hp = Some(10_000_000); }
        }
        // Keep the real first-wave roster and make it quick to clear so the
        // same scenario exercises the actual boss wave and its skills.
        for enemy in &mut built.fight.defender.as_mut().unwrap().entitys {
            enemy.current_hp = Some(1);
        }
        let mut runtime = BattleRuntime::new_with_attributes(catalog, built.fight,
            built.ex_attributes, built.sp_attributes);
        runtime.start_round_with_determinism(RoundDeterminism::with_seed(43))
            .unwrap_or_else(|e| panic!("opening failed layer={layer_id} plane={plane_id}: {e}"));
        let mut boss_turns = 0;
        let mut boss_actions = Vec::new();
        fn collect_boss_actions(step: &sonettobuf::FightStep, actions: &mut Vec<i32>) {
            if step.from_id.is_some_and(|uid| uid < 0)
                && step.act_id.is_some_and(|id| (23_310_111..=23_310_442).contains(&id))
            {
                actions.push(step.act_id.unwrap());
            }
            for child in step.act_effect.iter().filter_map(|effect| effect.fight_step.as_ref()) {
                collect_boss_actions(child, actions);
            }
        }
        for turn in 0..6 {
            let current_wave = runtime.fight.cur_wave.unwrap_or_default();
            if current_wave == 2 { boss_turns += 1; }
            let enemy = runtime.fight.defender.as_ref().unwrap().entitys.iter()
                .filter_map(|enemy| enemy.uid)
                .find(|uid| runtime.managers.hp.current(*uid) > 0)
                .expect("a living enemy should be present");
            let index = runtime.managers.card.hand().iter().position(|card| {
                card.skill_id.is_some_and(|id| !catalog.skill_is_big(id) && catalog.skill_is_attack(id))
                    && !card.temp_card.unwrap_or_default()
            }).expect("a normal card should be available");
            let result = runtime.build_begin_round_from_schedule(&BeginRoundRequest {
                opers: vec![BeginRoundOper { oper_type: Some(2), param1: Some(index as i32 + 1),
                    to_id: Some(enemy), ..Default::default() }], ..Default::default()
            }).unwrap_or_else(|e| panic!("action failed layer={layer_id} plane={plane_id} turn={turn}: {e}"));
            for step in result.fight_step.iter().chain(&result.next_round_begin_step) {
                collect_boss_actions(step, &mut boss_actions);
            }
        }
        assert!(boss_turns >= 3, "the test must exercise actual boss turns");
        assert!(boss_actions.len() >= 3, "boss must use its configured skills: layer={layer_id} plane={plane_id} actions={boss_actions:?}");
        if layer_id == 3 {
            assert!(boss_actions.contains(&23310112), "boss did not use its power-gated skill: {boss_actions:?}");
        }
    }
}
