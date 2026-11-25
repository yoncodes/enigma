use super::*;
use crate::engine::{
    manager::{
        card::CardSetTeamCards,
        ex_point::{ExPointChange, ExPointCommand},
    },
    runtime::schedule::start::{deal_round_start, settle_round_start},
};

#[test]
fn action_completion_carries_an_unplayed_draw_exempt_ultimate() {
    init_config();
    let holder = |uid, model_id, ultimate, buff_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        team_type: Some(1),
        current_hp: Some(100),
        ex_skill: Some(ultimate),
        buffs: vec![BuffInfo {
            uid: Some(uid + 10),
            from_uid: Some(uid),
            buff_id: Some(buff_id),
            ..Default::default()
        }],
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                holder(10, 3120, 31200133, 31200146),
                holder(11, 3140, 31402141, 31400010),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let origin = CommandOrigin {
        domain: RuleDomain::Lifecycle,
        key: DefinitionKey::new(0, "TestTeamCards"),
    };
    let card = |uid, skill_id| CardInfo {
        uid: Some(uid),
        skill_id: Some(skill_id),
        ..Default::default()
    };
    let existing = card(10, 31200114);
    let drawn = card(10, 312001215);
    let spare = card(11, 31402121);
    let mut unplayed = card(10, 31200133);
    unplayed.hero_id = Some(3120);
    let other = card(11, 31402141);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![existing.clone()],
            draw_pile: vec![drawn.clone(), spare.clone()],
            deck_num: 2,
        }))
        .unwrap();
    managers
        .execute_card(CardCommand::SetTeamCards(CardSetTeamCards {
            origin,
            cards: vec![unplayed.clone(), other.clone()],
        }))
        .unwrap();

    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_card_draws(vec![drawn.clone()]);
    let context = TargetContext {
        current_round: 2,
        ..Default::default()
    };
    run_action_phase_end(&mut managers, &pool, &catalog, &mut determinism, context, 1).unwrap();
    assert_eq!(managers.card.hand(), &[existing.clone(), unplayed.clone()]);
    assert_eq!(managers.card.team_cards(), &[other]);

    run_post_action_refill(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        context,
        2,
        1,
    )
    .unwrap();
    assert_eq!(
        managers.card.post_action_refill(),
        Some((
            [existing.clone(), unplayed.clone()].as_slice(),
            [drawn.clone()].as_slice(),
        ))
    );
    assert_eq!(
        managers.card.hand(),
        &[existing.clone(), unplayed, drawn.clone()]
    );
    assert_eq!(managers.card.deck_num(), 1);

    let started = settle_round_start(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        context,
        &[],
    )
    .unwrap();
    deal_round_start(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        context,
        started,
        2,
    )
    .unwrap();
    assert!(managers.card.team_cards().is_empty());
    managers.begin_action_round();
    managers
        .execute_card(CardCommand::Play(CardPlay {
            origin: CARD_PLAY_ORIGIN,
            hand_index: 1,
            target_uid: None,
            chosen_skill_id: None,
            choice: None,
            recorded_skill: None,
        }))
        .unwrap();
    managers.card.record_action_point_use(1, true);
    let refill = RefillSlots::capture(&managers, &pool);
    assert_eq!(refill.replacements(&managers), 0);
    assert_eq!(refill.target(&managers, &pool, 2), 2);
    assert!(managers.card.played()[0].uses_action_point);

    run_action_phase_end(&mut managers, &pool, &catalog, &mut determinism, context, 1).unwrap();
    run_post_action_refill(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        context,
        2,
        1,
    )
    .unwrap();
    assert_eq!(managers.card.hand(), &[existing.clone(), drawn.clone()]);
    assert_eq!(managers.card.draw_pile(), &[spare]);
    assert_eq!(managers.card.deck_num(), 1);

    managers
        .execute_ex_point(ExPointCommand::Change(ExPointChange {
            origin,
            source_uid: 11,
            target_uid: 11,
            delta: 5,
            config_effect: 0,
            effect_type: 0,
        }))
        .unwrap();
    let context = TargetContext {
        current_round: 3,
        ..context
    };
    let started = settle_round_start(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        context,
        &[],
    )
    .unwrap();
    let (_, _, before, dealt) = deal_round_start(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        context,
        started,
        2,
    )
    .unwrap();
    assert_eq!(before, [existing.clone(), drawn.clone()]);
    assert_eq!(managers.card.hand(), &[existing, drawn]);
    assert_eq!(
        dealt
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        [31402141]
    );
    assert_eq!(managers.card.team_cards(), dealt);
    assert_eq!(managers.card.deck_num(), 1);
}
