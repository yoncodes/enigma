use crate::engine::{
    manager::{
        BattleManagers,
        card::{
            CARD_PLAY_ORIGIN, CardCommand, CardSetAiQueue, CardSetup,
            start::{CapturedDeckSeed, configured_start_decks},
        },
    },
    runtime::{determinism::RoundDeterminism, drain, schedule},
    skill::{
        effect::SkillEffectCatalog,
        rule::SetupStage,
        target::{TargetContext, TargetPool},
    },
};
use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute, PowerInfo};

fn fight() -> Fight {
    let entity = |uid, model_id, team_type, position| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        team_type: Some(team_type),
        entity_type: Some(if team_type == 1 { 1 } else { 2 }),
        position: Some(position),
        current_hp: Some(10_000),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    Fight {
        version: Some(7),
        cur_round: Some(4),
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 3023, 1, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    skill_group1: vec![40411011],
                    ..entity(-6, 900100601, 2, 1)
                },
                FightEntityInfo {
                    passive_skill: vec![23310413],
                    power_infos: vec![PowerInfo {
                        power_id: Some(1),
                        num: Some(0),
                        max: Some(3),
                    }],
                    ..entity(-7, 900100304, 2, 2)
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[test]
fn prepared_cast_is_deferred() {
    crate::test_support::init_config();
    assert!(
        super::super::registry::find_key(50015, "UseExtraSkill")
            .unwrap()
            .destination
    );
    let fight = fight();
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut determinism = RoundDeterminism::default();
    let context = TargetContext {
        current_round: 4,
        ..Default::default()
    };

    drain::run_setup_stage(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        context,
        SetupStage::CardSetup,
        0,
    )
    .unwrap();

    assert_eq!(managers.hp.current(10), 10_000);
    assert_eq!(managers.eureka.get(-7, 1).current, 1);
    let decks =
        configured_start_decks(&fight, &managers, &catalog, &determinism, 0, 4, None).unwrap();
    assert_eq!(
        decks
            .ai
            .iter()
            .map(|card| (card.uid, card.skill_id))
            .collect::<Vec<_>>(),
        vec![(Some(-6), Some(40411011)), (Some(-7), Some(23310411))]
    );
    schedule::run_ai_queue_refresh(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        context,
        decks.ai,
    )
    .unwrap();
    let next =
        configured_start_decks(&fight, &managers, &catalog, &determinism, 0, 4, None).unwrap();
    assert_eq!(next.ai.len(), 1);
}

#[test]
fn prepared_duplicates_constrain_capture_seed() {
    crate::test_support::init_config();
    let mut fight = fight();
    let caster = &mut fight.defender.as_mut().unwrap().entitys[1];
    caster.model_id = Some(900100202);
    caster.passive_skill = vec![23310222];
    caster.power_infos[0].num = Some(3);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut determinism = RoundDeterminism::default();
    drain::run_setup_stage(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext {
            current_round: 4,
            ..Default::default()
        },
        SetupStage::CardSetup,
        0,
    )
    .unwrap();
    let generated =
        configured_start_decks(&fight, &managers, &catalog, &determinism, 0, 4, None).unwrap();
    assert_eq!(
        generated
            .ai
            .iter()
            .filter(|card| card.skill_id == Some(23310221))
            .count(),
        2
    );
    assert_eq!(managers.eureka.get(-7, 1).current, 1);
    let accepted = configured_start_decks(
        &fight,
        &managers,
        &catalog,
        &determinism,
        0,
        4,
        Some(CapturedDeckSeed::NextAi(generated.ai.clone())),
    )
    .unwrap();
    assert!(accepted.used_capture);
    assert_eq!(accepted.ai, generated.ai);
    let missing = vec![generated.ai[0].clone(); 3];
    let rejected = configured_start_decks(
        &fight,
        &managers,
        &catalog,
        &determinism,
        0,
        4,
        Some(CapturedDeckSeed::NextAi(missing)),
    )
    .unwrap();
    assert!(!rejected.used_capture);
    assert_eq!(rejected.ai, generated.ai);
}

#[test]
fn opening_commits_prepared_cards() {
    crate::test_support::init_config();
    let fight = fight();
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut determinism = RoundDeterminism::default();
    let ordinary =
        configured_start_decks(&fight, &managers, &catalog, &determinism, 0, 4, None).unwrap();
    managers
        .execute_card(CardCommand::SetAiQueue(CardSetAiQueue {
            origin: CARD_PLAY_ORIGIN,
            cards: ordinary.ai.clone(),
        }))
        .unwrap();
    schedule::run_start(
        managers.catalog(),
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext {
            current_round: 4,
            ..Default::default()
        },
        CardSetup {
            hand: Vec::new(),
            draw_pile: Vec::new(),
            deck_num: 0,
        },
        0,
    )
    .unwrap();
    assert_eq!(managers.hp.current(10), 10_000);
    assert_eq!(managers.card.ai_queue().len(), ordinary.ai.len() + 1);
    let prepared = managers.card.ai_queue().last().unwrap();
    assert_eq!(
        (prepared.uid, prepared.skill_id, prepared.target_uid),
        (Some(-7), Some(23310411), Some(10))
    );
    assert!(managers.card.prepared_ai_skills().is_empty());
}
