use super::*;

#[test]
fn exempt_spell_keeps_ultimate_classification() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3147),
                team_type: Some(1),
                current_hp: Some(1000),
                ex_point: Some(3),
                ex_skill: Some(31473141),
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
                model_id: Some(1),
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
        .execute_card(CardCommand::InsertSovereign(
            crate::engine::manager::card::CardInsertSovereign {
                origin: CARD_PLAY_ORIGIN,
                target_uid: 10,
                skill_id: 31472131,
                hero_id: 3147,
                max_position: 9,
                team_type: 1,
            },
        ))
        .unwrap();
    managers
        .execute_card(CardCommand::UnlockSovereign {
            origin: CARD_PLAY_ORIGIN,
            owner_uid: 10,
            team_type: 1,
        })
        .unwrap();
    managers
        .execute_card(CardCommand::Play(CardPlay {
            origin: CARD_PLAY_ORIGIN,
            hand_index: 0,
            target_uid: Some(-1),
            chosen_skill_id: None,
            choice: None,
            recorded_skill: None,
        }))
        .unwrap();
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31472131, 31473141], []);
    let card = crate::engine::mechanic::card::CardMechanic;
    let entity = pool.entity(10).unwrap();
    assert!(card.is_ultimate_skill(&managers, 31472131, entity));
    assert!(!card.ultimate_ready(&managers, entity));
    let cast = |managers: &mut BattleManagers, skill_id| {
        let mut skill = crate::engine::skill::action::SkillInvocation::from(
            crate::engine::skill::action::SkillRequest {
                source_uid: 10,
                skill_id,
            },
        );
        skill.card_index = 1;
        run_active_action(
            managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            ActiveActionRequest {
                skill,
                grants_ex_point: false,
                grant_after_action: false,
                composition_rewards: Vec::new(),
                prelude: Vec::new(),
            },
        )
    };
    cast(&mut managers, 31472131).unwrap();
    assert_eq!(managers.ex_point.get(10), 3);
    assert!(!card.ultimate_ready(&managers, entity));
    assert!(matches!(
        cast(&mut managers, 31473141),
        Err(DrainError::InsufficientUltimateResource {
            required: 5,
            current: 3,
            ..
        })
    ));
    assert_eq!(managers.ex_point.get(10), 3);
}
