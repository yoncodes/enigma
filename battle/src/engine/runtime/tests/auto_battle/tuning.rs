use super::*;

const SHUFFLE: i32 = 30010202;
const ATTACK: i32 = 30810481;
const DEFENSE: i32 = 30810226;

fn ready(hand: Vec<CardInfo>) -> BattleRuntime {
    crate::test_support::init_config();
    let cloth = Some(1);
    let mut runtime = runtime(Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![team_hero(10, 3081, 1_000, [ATTACK, DEFENSE])],
            player_entity: Some(FightEntityInfo {
                uid: Some(0),
                current_hp: Some(100),
                ..Default::default()
            }),
            power: Some(99),
            cloth_id: cloth,
            skill_infos: crate::engine::fight::team::Team::get_player_skills(cloth),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![enemy(-1, 100_000), enemy(-2, 100_000)],
            ..Default::default()
        }),
        ..Default::default()
    });
    runtime
        .managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand,
            draw_pile: vec![hand_card(10, ATTACK, false)],
            deck_num: 1,
        }))
        .unwrap();
    runtime.extend_current_card_skill_roots();
    runtime.round_state.act_point = 1;
    runtime
}

fn recommendation(runtime: &BattleRuntime) -> Option<i32> {
    runtime
        .auto_round_planner()
        .plan(&AutoRoundRequest::default())
        .unwrap()
        .cloth_skill
        .and_then(|skill| skill.skill_id)
        .filter(|&skill| skill > 0)
}

#[test]
fn recommends_shuffle_for_an_uncovered_defense_setup() {
    let runtime = ready(vec![hand_card(10, ATTACK, false)]);
    assert_eq!(recommendation(&runtime), Some(SHUFFLE));
    assert_eq!(recommendation(&runtime), Some(SHUFFLE));
    assert_eq!(runtime.round_state.power, 99);

    let mut covered = runtime.clone();
    grant_buff(&mut covered, 10, -2, 30810104);
    assert_eq!(recommendation(&covered), None);

    let in_hand = ready(vec![hand_card(10, DEFENSE, false)]);
    assert_eq!(recommendation(&in_hand), None);

    let mut unready = runtime;
    unready.round_state.power = 24;
    assert_eq!(recommendation(&unready), None);
}

#[test]
fn settles_the_shuffle_before_admitting_another() {
    let attack = hand_card(10, ATTACK, false);
    let mut runtime = ready(vec![attack.clone()]);
    runtime.determinism.enqueue_card_draws(vec![attack]);
    let selection = runtime
        .auto_round_planner()
        .plan(&AutoRoundRequest::default())
        .unwrap();
    assert!(selection.opers.is_empty());
    let shuffle = selection.cloth_skill.unwrap();
    assert_eq!(shuffle.skill_id, Some(SHUFFLE));
    runtime
        .use_cloth_skill(UseClothSkillRequest {
            skill_id: shuffle.skill_id,
            from_id: shuffle.from_id,
            to_id: shuffle.to_id,
            r#type: shuffle.r#type,
        })
        .unwrap();
    assert_eq!(runtime.round_state.power, 74);
    assert_eq!(runtime.card_hand()[0].skill_id, Some(ATTACK));
    assert_eq!(recommendation(&runtime), None);

    let cards = runtime
        .auto_round_planner()
        .plan(&AutoRoundRequest::default())
        .unwrap();
    assert_eq!(cards.opers.len(), 1);
    runtime
        .advance_round(BeginRoundRequest {
            opers: cards.opers,
            ..Default::default()
        })
        .unwrap();
    assert!(!runtime.cloth_redealt);
}
