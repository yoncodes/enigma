use super::*;

// Reproduce the reported four-character team with actual configured loadouts,
// including Semmelweis's original and upgraded ultimate variants.
#[test]
fn bloodtithe_team_ready_ultimates_survive_deal_and_next_round() {
    crate::test_support::init_config();
    use crate::dungeon::{BattleFighter, BattleRoster, FightOptions, HeroBuildInput};

    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    for destiny_rank in [0, 3, 4] {
        let specs = [(3120, 312001), (3088, 308801), (3126, 312601), (3125, 312501)];
        let fighters = specs.into_iter().enumerate().map(|(index, (hero_id, skin))| {
            BattleFighter {
                hero: HeroBuildInput {
                    uid: 30_000_001 + index as i64, user_id: 1, hero_id, skin,
                    level: 180, rank: 4, ex_skill_level: 5, talent: 15,
                    destiny_stone: if hero_id == 3088 && destiny_rank > 0 { 308801 } else { 0 },
                    destiny_rank: if hero_id == 3088 { destiny_rank } else { 0 },
                    ..Default::default()
                },
                equips: vec![],
            }
        }).collect::<Vec<_>>();
        let group = sonettobuf::FightGroup {
            hero_list: fighters.iter().map(|fighter| fighter.hero.uid).collect(),
            ..Default::default()
        };
        let roster = BattleRoster { user_id: 1, fighters, ..Default::default() };
        let mut built = crate::dungeon::build_fight(catalog, &roster, 20411, 2411,
            &group, FightOptions::default(), None).unwrap();
        let allies = &mut built.fight.attacker.as_mut().unwrap().entitys;
        for hero in allies.iter_mut() {
            hero.ex_point = Some(if hero.model_id == Some(3088) { 4 } else { 5 });
        }
        let semmel = allies.iter().find(|hero| hero.model_id == Some(3088)).unwrap();
        let owner_uid = semmel.uid.unwrap();
        let ultimate = semmel.ex_skill.unwrap();
        let other_ultimates = allies.iter().filter(|hero|
            matches!(hero.model_id, Some(3125 | 3126)))
            .map(|hero| (hero.uid.unwrap(), hero.ex_skill.unwrap()))
            .collect::<Vec<_>>();
        assert!(matches!(ultimate, 30880133 | 308801331 | 308801332),
            "unexpected Semmelweis ultimate {ultimate}, destiny {destiny_rank}");
        for enemy in &mut built.fight.defender.as_mut().unwrap().entitys {
            enemy.skill_group1.clear();
            enemy.skill_group2.clear();
            enemy.passive_skill.clear();
            enemy.ex_skill = Some(0);
            enemy.current_hp = Some(1_000_000_000);
            if let Some(attr) = enemy.attr.as_mut() { attr.hp = Some(1_000_000_000); }
            if let Some(attr) = enemy.base_attr.as_mut() { attr.hp = Some(1_000_000_000); }
        }
        let enemy_uid = built.fight.defender.as_ref().unwrap().entitys[0].uid.unwrap();
        let mut runtime = BattleRuntime::new_with_attributes(
            catalog, built.fight, built.ex_attributes, built.sp_attributes,
        );
        let opening = runtime.start_round_with_determinism(RoundDeterminism::with_seed(43)).unwrap();
        assert_eq!(runtime.managers.ex_point.get(owner_uid), 5, "destiny {destiny_rank}");
        let count = |cards: &[CardInfo]| cards.iter().filter(|card|
            card.uid == Some(owner_uid) && card.skill_id == Some(ultimate)).count();
        assert_eq!(count(runtime.managers.card.hand()), 1, "opening, destiny {destiny_rank}");
        assert_eq!(count(&opening.team_a_cards1), 1, "opening packet, destiny {destiny_rank}");
        for (uid, skill) in &other_ultimates {
            assert!(runtime.managers.card.hand().iter().any(|card|
                card.uid == Some(*uid) && card.skill_id == Some(*skill)),
                "other bloodtithe hero {uid} missing ultimate");
        }

        let next = runtime.build_begin_round_from_schedule(&BeginRoundRequest::default()).unwrap();
        assert_eq!(runtime.managers.ex_point.get(owner_uid), 5, "next round, destiny {destiny_rank}");
        assert_eq!(count(runtime.managers.card.hand()), 1, "next round, destiny {destiny_rank}");
        assert_eq!(count(&next.before_cards1) + count(&next.team_a_cards1), 1,
            "next-round packet, destiny {destiny_rank}");

        // Actually play other team members' cards. The client's final hand is
        // replaced by the CardsPush snapshot after each refill, so check that
        // snapshot as well as the authoritative hand.
        for turn in 0..3 {
            let index = runtime.managers.card.hand().iter().position(|card| {
                card.uid != Some(owner_uid)
                    && !card.temp_card.unwrap_or_default()
                    && card.skill_id.is_some_and(|id| !catalog.skill_is_big(id))
            }).expect("a teammate must have a normal card");
            let round = runtime.build_begin_round_from_schedule(&BeginRoundRequest {
                opers: vec![BeginRoundOper {
                    oper_type: Some(2), param1: Some(index as i32 + 1), to_id: Some(enemy_uid),
                    ..Default::default()
                }],
                ..Default::default()
            }).unwrap();
            assert_eq!(runtime.managers.ex_point.get(owner_uid), 5, "turn {turn}, destiny {destiny_rank}");
            assert_eq!(count(runtime.managers.card.hand()), 1, "hand after turn {turn}, destiny {destiny_rank}");
            let final_push = round.next_round_begin_step.iter().flat_map(|step| &step.act_effect)
                .filter(|effect| effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Cardspush as i32))
                .last().expect("the next round must send its final hand");
            assert_eq!(count(&final_push.card_info_list), 1, "client hand after turn {turn}, destiny {destiny_rank}");
            for (uid, skill) in &other_ultimates {
                assert!(runtime.managers.card.hand().iter().any(|card|
                    card.uid == Some(*uid) && card.skill_id == Some(*skill)),
                    "other bloodtithe hero {uid} lost ultimate after turn {turn}");
                assert!(final_push.card_info_list.iter().any(|card|
                    card.uid == Some(*uid) && card.skill_id == Some(*skill)),
                    "other bloodtithe hero {uid} missing from client hand after turn {turn}");
            }
        }
    }
}

#[test]
fn bloodtithe_semmelweis_ultimate_survives_becoming_ready_during_battle() {
    crate::test_support::init_config();
    use crate::dungeon::{BattleFighter, BattleRoster, FightOptions, HeroBuildInput};
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    for destiny_rank in [0, 3, 4] {
        let specs = [(3120, 312001), (3088, 308801), (3126, 312601), (3125, 312501)];
        let fighters = specs.into_iter().enumerate().map(|(index, (hero_id, skin))| BattleFighter {
            hero: HeroBuildInput {
                uid: 30_000_001 + index as i64, user_id: 1, hero_id, skin,
                level: 180, rank: 4, ex_skill_level: 5, talent: 15,
                destiny_stone: if hero_id == 3088 && destiny_rank > 0 { 308801 } else { 0 },
                destiny_rank: if hero_id == 3088 { destiny_rank } else { 0 },
                ..Default::default()
            }, equips: vec![],
        }).collect::<Vec<_>>();
        let group = sonettobuf::FightGroup {
            hero_list: fighters.iter().map(|fighter| fighter.hero.uid).collect(), ..Default::default()
        };
        let roster = BattleRoster { user_id: 1, fighters, ..Default::default() };
        let mut built = crate::dungeon::build_fight(catalog, &roster, 20411, 2411,
            &group, FightOptions::default(), None).unwrap();
        let allies = &mut built.fight.attacker.as_mut().unwrap().entitys;
        for hero in allies.iter_mut() { hero.ex_point = Some(0); }
        let semmel = allies.iter().find(|hero| hero.model_id == Some(3088)).unwrap();
        let owner = semmel.uid.unwrap();
        let ultimate = semmel.ex_skill.unwrap();
        for enemy in &mut built.fight.defender.as_mut().unwrap().entitys {
            enemy.skill_group1.clear(); enemy.skill_group2.clear(); enemy.passive_skill.clear();
            enemy.ex_skill = Some(0); enemy.current_hp = Some(1_000_000_000);
            if let Some(attr) = enemy.attr.as_mut() { attr.hp = Some(1_000_000_000); }
            if let Some(attr) = enemy.base_attr.as_mut() { attr.hp = Some(1_000_000_000); }
        }
        let enemy = built.fight.defender.as_ref().unwrap().entitys[0].uid.unwrap();
        let mut runtime = BattleRuntime::new_with_attributes(catalog, built.fight,
            built.ex_attributes, built.sp_attributes);
        runtime.start_round_with_determinism(RoundDeterminism::with_seed(43)).unwrap();
        // This team may already generate the ultimate at battle entry. Prepare
        // the same state as a legitimate later Moxie loss, then exercise real
        // card plays, refill and client snapshots when it becomes ready again.
        let starting_moxie = runtime.managers.ex_point.get(owner);
        if starting_moxie >= 5 {
            use crate::engine::manager::{
                card::{CardCommand, CardSetUltimateAvailability},
                ex_point::{ExPointChange, ExPointCommand},
            };
            use crate::engine::skill::rule::{CommandOrigin, DefinitionKey, RuleDomain};
            let origin = CommandOrigin { domain: RuleDomain::Lifecycle,
                key: DefinitionKey::new(0, "TestMoxieLoss") };
            runtime.managers.execute_ex_point(ExPointCommand::Change(ExPointChange {
                origin, source_uid: owner, target_uid: owner, delta: 3 - starting_moxie,
                config_effect: 0, effect_type: 0,
            })).unwrap();
            if let Some(card) = runtime.managers.card.hand().iter().find(|card|
                card.uid == Some(owner) && card.skill_id == Some(ultimate)).cloned() {
                runtime.managers.execute_card(CardCommand::SetUltimateAvailability(
                    CardSetUltimateAvailability { origin, card, available: false }
                )).unwrap();
            }
        }
        assert!(runtime.managers.ex_point.get(owner) < 5, "fixture should start below readiness");
        let mut seen_ready = false;
        for turn in 0..12 {
            let hand = runtime.managers.card.hand();
            let index = hand.iter().position(|card| card.uid == Some(owner)
                && card.skill_id.is_some_and(|id| (30880111..=30880116).contains(&id)))
                .or_else(|| hand.iter().position(|card| card.uid != Some(owner)
                    && card.skill_id.is_some_and(|id| !catalog.skill_is_big(id))
                    && !card.temp_card.unwrap_or_default()))
                .expect("the fixture must offer a playable normal card");
            let round = runtime.build_begin_round_from_schedule(&BeginRoundRequest {
                opers: vec![BeginRoundOper { oper_type: Some(2), param1: Some(index as i32 + 1),
                    to_id: Some(enemy), ..Default::default() }], ..Default::default()
            }).unwrap();
            if runtime.managers.ex_point.get(owner) < 5 { continue; }
            let count = |cards: &[CardInfo]| cards.iter()
                .filter(|card| card.uid == Some(owner) && card.skill_id == Some(ultimate)).count();
            assert_eq!(count(runtime.managers.card.hand()), 1, "hand at turn {turn}, destiny {destiny_rank}");
            let push = round.next_round_begin_step.iter().flat_map(|step| &step.act_effect)
                .filter(|effect| effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Cardspush as i32))
                .last().expect("the next round must send its final hand");
            assert_eq!(count(&push.card_info_list), 1, "client at turn {turn}, destiny {destiny_rank}");
            if seen_ready { break; }
            seen_ready = true;
        }
        assert!(seen_ready, "Semmelweis never reached five moxie, destiny {destiny_rank}");
    }
}
