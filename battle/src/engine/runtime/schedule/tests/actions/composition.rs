use super::*;
use crate::engine::manager::card::{CardInsertSovereign, CardSetup};

mod timing;

fn setup(
    hand: &[i32],
    moxie: i32,
    enemy_hp: i32,
) -> (BattleManagers, TargetPool, SkillEffectCatalog) {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3147),
                team_type: Some(1),
                current_hp: Some(1000),
                ex_point: Some(moxie),
                ex_point_max: Some(5),
                ex_skill: Some(31473141),
                passive_skill: vec![999900001],
                skill_group1: vec![31470111, 31470112, 31470113],
                skill_group2: vec![31473121, 31473122, 31473123],
                attr: Some(HeroAttribute {
                    hp: Some(1000),
                    attack: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(enemy_hp),
                attr: Some(HeroAttribute {
                    hp: Some(enemy_hp),
                    ..Default::default()
                }),
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
            hand: hand
                .iter()
                .map(|id| CardInfo {
                    uid: Some(10),
                    skill_id: Some(*id),
                    ..Default::default()
                })
                .collect(),
            draw_pile: Vec::new(),
            deck_num: 16,
        }))
        .unwrap();
    let mut catalog = SkillEffectCatalog::from_roots(
        config::configs::get(),
        [31473141, 31472131, 31470111, 31473121],
        [],
    );
    let mut observer = SkillEffectSlot::new(
        ParsedBehavior::new(1, "AddBuff", vec![205]),
        TargetRequest::self_only(),
    );
    observer.conditions =
        crate::engine::skill::condition::parse_conditions(config::configs::get(), "40#0&526203#0");
    observer.compiled_route = ConditionRoute::compile(&observer.conditions);
    catalog.insert(ParsedSkillEffect {
        skill_id: 999900001,
        slots: vec![observer],
    });
    (managers, pool, catalog)
}

fn play(card_index: usize, target_uid: i64) -> RoundCommand {
    RoundCommand::PlayCard {
        card_index,
        target_uid: Some(target_uid),
        chosen_skill_id: None,
        recorded_skill: None,
    }
}

#[test]
fn played_composition_reward_survives_ultimate_payment_at_cap() {
    let (mut managers, pool, catalog) = setup(&[31470111, 31473141, 31470111], 5, 1000);
    let result = run_player_commands(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [play(1, 10)],
        1,
        crate::engine::manager::emitter::UID,
    )
    .unwrap();
    assert_eq!(managers.card.hand()[0].skill_id, Some(31470112));
    assert_eq!(managers.ex_point.get(10), 1);
    assert!(managers.buff.has_active_buff_id(10, 205));
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let ultimate = steps
        .iter()
        .find(|step| step.act_id == Some(31473141))
        .unwrap();
    let deltas = ultimate
        .act_effect
        .iter()
        .filter(|effect| effect.effect_type == Some(111))
        .map(|effect| effect.effect_num.unwrap())
        .collect::<Vec<_>>();
    assert_eq!(deltas, [-5, 1]);
}

#[test]
fn invalidated_card_keeps_composition_and_play_rewards_separate() {
    let (mut managers, pool, catalog) = setup(&[31473121, 31470111, 31473121, 31470111], 3, 100);
    managers
        .execute_card(CardCommand::InsertSovereign(CardInsertSovereign {
            origin: CARD_PLAY_ORIGIN,
            target_uid: 10,
            skill_id: 31472131,
            hero_id: 3147,
            max_position: 1,
            team_type: 1,
        }))
        .unwrap();
    managers
        .execute_card(CardCommand::UnlockSovereign {
            origin: CARD_PLAY_ORIGIN,
            owner_uid: 10,
            team_type: 1,
        })
        .unwrap();
    let result = run_player_commands(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [play(0, -1), play(1, -1)],
        1,
        crate::engine::manager::emitter::UID,
    )
    .unwrap();
    assert_eq!(managers.hp.current(-1), 0);
    assert_eq!(managers.ex_point.get(10), 5);
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let rewards = steps
        .iter()
        .flat_map(|step| &step.act_effect)
        .filter_map(|effect| match effect.effect_type {
            Some(111) if effect.target_id == Some(10) => Some((111, effect.effect_num.unwrap())),
            Some(160) => Some((160, effect.effect_num.unwrap())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(rewards, [(111, 1), (160, 2), (111, 1)]);
}
