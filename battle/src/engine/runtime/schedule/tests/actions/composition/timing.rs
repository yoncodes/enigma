use super::*;
use crate::engine::event::payload::BattleEvent;

#[test]
fn other_owner_composition_precedes_paid_source_opening() {
    init_config();
    let entity = |uid, model_id, moxie, group| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        team_type: Some(1),
        current_hp: Some(1000),
        ex_point: Some(moxie),
        ex_point_max: Some(5),
        skill_group1: group,
        attr: Some(HeroAttribute {
            hp: Some(1000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    ex_skill: Some(31473141),
                    ..entity(10, 3147, 5, vec![])
                },
                entity(20, 3023, 0, vec![30230111, 30230112, 30230113]),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(1000),
                attr: Some(HeroAttribute {
                    hp: Some(1000),
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
            hand: [(20, 30230111), (10, 31473141), (20, 30230111)]
                .into_iter()
                .map(|(uid, skill_id)| CardInfo {
                    uid: Some(uid),
                    skill_id: Some(skill_id),
                    ..Default::default()
                })
                .collect(),
            draw_pile: vec![],
            deck_num: 16,
        }))
        .unwrap();
    let mut catalog =
        SkillEffectCatalog::from_roots(config::configs::get(), [31473141, 30230111], []);
    let mut skill = catalog.get(31473141).unwrap().clone();
    let mut observer = SkillEffectSlot::new(
        ParsedBehavior::new(1, "AddBuff", vec![205]),
        TargetRequest::self_only(),
    );
    observer.conditions =
        crate::engine::skill::condition::parse_conditions(config::configs::get(), "201");
    observer.compiled_route = ConditionRoute::compile(&observer.conditions);
    skill.slots.push(observer);
    catalog.insert(skill);
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

    let reward = result
        .events
        .iter()
        .position(
            |event| matches!(event, BattleEvent::ExPointChanged(change) if change.target_uid == 20),
        )
        .unwrap();
    let opening = result
        .events
        .iter()
        .position(|event| matches!(event, BattleEvent::BuffAdded(change) if change.target_uid == 10 && change.buff_id == 205))
        .unwrap();
    assert!(reward < opening);
    assert_eq!(managers.ex_point.get(20), 1);
    assert_eq!(managers.ex_point.get(10), 0);
}
