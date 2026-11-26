use super::*;
use sonettobuf::{BuffInfo, CardInfo, Fight, FightEntityInfo, FightTeam};

#[test]
fn post_action_refill_keeps_the_committed_ultimate_size_exemption() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3127),
                team_type: Some(1),
                current_hp: Some(1_000),
                ex_point: Some(5),
                ex_skill: Some(31270132),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31390190),
                    from_uid: Some(10),
                    duration: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(100_000),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: [31270132, 31270111, 31270121, 31270111, 31270121, 31270111]
                .into_iter()
                .map(|skill_id| CardInfo {
                    uid: Some(10),
                    skill_id: Some(skill_id),
                    ..Default::default()
                })
                .collect(),
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31270132, 31270111], []);
    let play = RoundCommand::PlayCard {
        card_index: 0,
        target_uid: Some(-1),
        chosen_skill_id: None,
        recorded_skill: None,
    };
    let result = run_player_phase(
        &fight,
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [play.clone(), play],
        &[],
        1,
        crate::engine::manager::emitter::UID,
    )
    .unwrap();

    assert!(managers.buff.snapshot(10, 20).is_none());
    assert!(
        managers
            .card
            .played()
            .iter()
            .all(|play| play.uses_action_point)
    );
    assert_eq!(result.refill_slots.replacements(&managers), 1);
    assert_eq!(result.refill_slots.target(&managers, &pool, 5), 5);
    managers.card.add_to_hand(CardInfo {
        uid: Some(10),
        skill_id: Some(31270111),
        ..Default::default()
    });
    assert_eq!(result.refill_slots.target(&managers, &pool, 6), 7);
}
