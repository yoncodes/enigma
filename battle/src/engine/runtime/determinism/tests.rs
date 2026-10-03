use sonettobuf::CardInfo;

use super::{AiSkillChoice, ConditionRandomChoice, RoundDeterminism};

#[test]
fn crit_roll_respects_impossible_and_guaranteed_chances() {
    let mut determinism = RoundDeterminism::default();
    assert!(!determinism.roll_crit(1, 2, 3, 0));
    assert!(determinism.roll_crit(1, 2, 3, 1000));
}

#[test]
fn ai_skill_choices_are_separate_from_the_card_envelope() {
    let mut determinism = RoundDeterminism::default();
    let choice = AiSkillChoice {
        source_uid: -2,
        skill_id: 530000511,
        target_uid: -2,
    };

    determinism.enqueue_ai_skills(vec![choice]);

    assert_eq!(determinism.take_ai_skills(), Some(vec![choice]));
}

#[test]
fn empty_ai_observations_do_not_override_owned_runtime_state() {
    let mut determinism = RoundDeterminism::default();

    determinism.enqueue_next_ai_card_snapshot(Vec::new());
    determinism.enqueue_ai_skills(Vec::new());

    assert_eq!(determinism.take_next_ai_card_snapshot(), None);
    assert_eq!(determinism.take_ai_skills(), None);
}

#[test]
fn captured_random_skill_only_consumes_a_matching_choice() {
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_random_skills([20]);

    assert_eq!(determinism.take_random_skill(&[10]), None);
    assert!(!determinism.has_scripted_random_skill(&[10]));
    assert_eq!(determinism.take_random_skill(&[10, 20]), Some(20));
    assert!(determinism.has_scripted_random_skill(&[10, 20]));
}

#[test]
fn captured_random_buff_only_consumes_a_matching_choice() {
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_random_buffs([20]);

    assert_eq!(determinism.take_random_buff(&[10]), None);
    assert_eq!(determinism.take_random_buff(&[10, 20]), Some(20));
}

#[test]
fn target_preview_does_not_consume_the_damage_decision() {
    let mut determinism = RoundDeterminism::default();
    determinism.skill_targets.push(super::SkillTargetChoice {
        skill_id: 10,
        source_uid: 1,
        target_code: 202,
        targets: vec![-1],
        additional_targets: Vec::new(),
        crit_targets: vec![-1],
        additional_crit_targets: Vec::new(),
    });

    assert_eq!(determinism.take_skill_targets(10, 1, 202), Some(vec![-1]));
    assert_eq!(
        determinism
            .take_skill_target_choice(10, 1, 202)
            .unwrap()
            .crit_targets,
        vec![-1]
    );
}

#[test]
fn zero_seed_lua_pool_order_matches_start_battle_rng() {
    let mut determinism = RoundDeterminism::default();

    assert_eq!(determinism.lua_random_index(4), Some(1));
    assert_eq!(determinism.lua_random_index(3), Some(0));
}

#[test]
fn card_draws_are_seeded_but_preview_can_supply_the_decision() {
    let candidates = vec![CardInfo {
        uid: Some(7),
        skill_id: Some(1),
        card_effect: Some(2),
        ..Default::default()
    }];
    let scripted = CardInfo {
        uid: Some(7),
        skill_id: Some(1),
        card_effect: Some(99),
        ..Default::default()
    };
    let mut determinism = RoundDeterminism::with_seed(7);
    determinism.enqueue_card_draws(vec![scripted]);

    assert_eq!(
        determinism.draw_cards(&candidates, 2),
        vec![candidates[0].clone(), candidates[0].clone()]
    );
}

#[test]
fn captured_card_energy_only_applies_to_the_matching_hand() {
    let card = |uid, skill_id, energy| CardInfo {
        uid: Some(uid),
        skill_id: Some(skill_id),
        energy: Some(energy),
        ..Default::default()
    };
    let hand = vec![card(1, 10, 0), card(2, 20, 0)];
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_card_energy_snapshot(vec![card(1, 10, 3), card(2, 20, 1)]);

    assert_eq!(
        determinism
            .take_card_energy_snapshot(&hand, 4)
            .unwrap()
            .iter()
            .map(|card| card.energy)
            .collect::<Vec<_>>(),
        vec![Some(3), Some(1)]
    );
}

#[test]
fn captured_card_energy_validates_the_added_energy_not_the_total() {
    let card = |uid, skill_id, energy| CardInfo {
        uid: Some(uid),
        skill_id: Some(skill_id),
        energy: Some(energy),
        ..Default::default()
    };
    let hand = vec![card(1, 10, 3), card(2, 20, 1)];
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_card_energy_snapshot(vec![card(1, 10, 5), card(2, 20, 3)]);

    assert!(determinism.take_card_energy_snapshot(&hand, 4).is_some());
}

#[test]
fn captured_condition_roll_is_owned_by_skill_and_opcode() {
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_condition_random_choices(vec![ConditionRandomChoice {
        skill_id: 31260161,
        opcode: 552210,
        roll: 499,
    }]);

    assert_eq!(determinism.condition_random_roll(31260161, 552210), 499);
    assert_eq!(determinism.condition_random_roll(31260161, 552210), 999);
    assert!((0..1000).contains(&determinism.condition_random_roll(1, 552210)));
}

#[test]
fn observed_additional_crits_are_consumed_per_target_until_replaced() {
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_additional_crits(100, 10, -1, [false, true]);
    determinism.enqueue_additional_crits(100, 10, -2, [true]);

    assert!(!determinism.roll_additional_crit(100, 10, 20, -1, 1000));
    assert!(determinism.roll_additional_crit(100, 10, 20, -2, 0));
    assert!(determinism.roll_additional_crit(100, 10, 20, -1, 0));

    determinism.enqueue_hidden_crits(100, 10, [true]);
    determinism.enqueue_additional_crits(100, 10, -1, [true]);
    determinism.clear_crit_choices();
    assert!(!determinism.roll_hidden_crit(100, 10, -1, 0));
    assert!(!determinism.roll_additional_crit(100, 10, 20, -1, 0));
}
