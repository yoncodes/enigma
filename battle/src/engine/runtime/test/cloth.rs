use super::*;

#[test]
fn rejects_unknown_cloth_skill_type_at_runtime_boundary() {
    crate::test_support::init_config();
    let mut runtime = BattleRuntime::new(Fight::default());
    runtime.pending_redeal = Some(RedealCardInfoPush::default());

    assert!(
        runtime
            .use_cloth_skill(UseClothSkillRequest {
                r#type: Some(-1),
                ..Default::default()
            })
            .is_none()
    );
    assert!(runtime.pending_redeal.is_some());
}

#[test]
fn configured_cloth_skills_drive_universal_and_redeal_card_rules() {
    crate::test_support::init_config();
    let card = |skill_id| CardInfo {
        uid: Some(10),
        skill_id: Some(skill_id),
        card_effect: Some(1),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(6),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                skill_group1: vec![101, 102],
                skill_group2: vec![201, 202],
                ..Default::default()
            }],
            player_entity: Some(FightEntityInfo {
                uid: Some(0),
                current_hp: Some(100),
                ..Default::default()
            }),
            power: Some(99),
            cloth_id: Some(1),
            skill_infos: crate::engine::fight::team::Team::get_player_skills(Some(1)),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = BattleRuntime::new(fight);
    runtime.catalog =
        SkillEffectCatalog::from_roots(config::configs::get(), [30010201, 30010202], []);
    runtime
        .managers
        .execute_card(crate::engine::manager::card::CardCommand::Setup(
            crate::engine::manager::card::CardSetup {
                hand: vec![card(101)],
                draw_pile: vec![card(201)],
                deck_num: 16,
            },
        ))
        .unwrap();

    let universal = runtime
        .use_cloth_skill(UseClothSkillRequest {
            skill_id: Some(30010201),
            from_id: Some(0),
            to_id: Some(0),
            r#type: Some(ClothSkillType::ClothSkill as i32),
        })
        .unwrap();
    assert_eq!(
        runtime.card_hand().last().unwrap().skill_id,
        Some(30_000_001)
    );
    let universal = universal.round.unwrap();
    assert_eq!(universal.power, Some(59));
    assert_eq!(
        universal
            .skill_infos
            .iter()
            .find(|skill| skill.skill_id == Some(30010201))
            .and_then(|skill| skill.need_power),
        Some(50)
    );
    assert!(universal.fight_step.iter().any(|step| {
        step.act_effect.iter().any(|effect| {
            effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Universalcard as i32)
        })
    }));

    let redeal = runtime
        .use_cloth_skill(UseClothSkillRequest {
            skill_id: Some(30010202),
            from_id: Some(0),
            to_id: Some(0),
            r#type: Some(ClothSkillType::ClothSkill as i32),
        })
        .unwrap();
    let redeal = redeal.round.unwrap();
    assert_eq!(redeal.power, Some(34));
    let v6_effect = &redeal.fight_step[0].act_effect[0];
    assert_eq!(
        v6_effect.effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Redealcard as i32)
    );
    assert_eq!(v6_effect.config_effect, Some(60012));
    assert_eq!(v6_effect.team_type, Some(0));
    assert!(v6_effect.card_info_list.is_empty());
    assert_eq!(runtime.card_hand()[0].skill_id, Some(201));
    assert_eq!(
        runtime.take_redeal_card_push().unwrap().card_group,
        runtime.card_hand()
    );

    runtime.fight.version = Some(7);
    let redeal = runtime
        .use_cloth_skill(UseClothSkillRequest {
            skill_id: Some(30010202),
            from_id: Some(0),
            to_id: Some(0),
            r#type: Some(ClothSkillType::ClothSkill as i32),
        })
        .unwrap()
        .round
        .unwrap();
    assert_eq!(redeal.power, Some(9));
    assert_eq!(redeal.fight_step.len(), 2);
    assert_eq!(
        redeal.fight_step[0].act_effect[0].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Afterredealcard as i32)
    );
    assert_eq!(redeal.fight_step[0].act_effect[0].team_type, Some(1));
    assert!(
        redeal.fight_step[0].act_effect[0]
            .card_info_list
            .iter()
            .map(|card| (card.uid, card.skill_id, card.temp_card.unwrap_or_default()))
            .eq(runtime.card_hand().iter().map(|card| (
                card.uid,
                card.skill_id,
                card.temp_card.unwrap_or_default()
            )))
    );
    assert_eq!(
        redeal.fight_step[1].act_effect[0].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Cardspush as i32)
    );
    assert_eq!(redeal.fight_step[1].act_effect[0].effect_num, Some(0));
    assert_eq!(redeal.fight_step[1].act_effect[0].effect_num1, Some(0));
    assert_eq!(redeal.fight_step[1].act_effect[0].team_type, Some(0));
    assert!(
        redeal.fight_step[1].act_effect[0]
            .card_info_list
            .iter()
            .map(|card| (card.uid, card.skill_id, card.temp_card.unwrap_or_default()))
            .eq(runtime.card_hand().iter().map(|card| (
                card.uid,
                card.skill_id,
                card.temp_card.unwrap_or_default()
            )))
    );
    assert!(runtime.take_redeal_card_push().is_none());
}

#[test]
fn conduit_selection_adds_the_configured_precast_and_commits_the_choice() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                current_hp: Some(100),
                buffs: vec![sonettobuf::BuffInfo {
                    uid: Some(1146),
                    buff_id: Some(31490013),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = BattleRuntime::new(fight);
    let reply = runtime
        .use_cloth_skill(UseClothSkillRequest {
            skill_id: Some(0),
            from_id: Some(10),
            to_id: Some(4),
            r#type: Some(ClothSkillType::TwinsSelect as i32),
        })
        .unwrap()
        .round
        .unwrap();

    let card = runtime.card_hand().last().unwrap();
    assert_eq!(card.skill_id, Some(31446012));
    assert_eq!(card.temp_card, Some(true));
    assert_eq!(card.hero_id, Some(3149));
    assert_eq!(runtime.managers.card.normal_hand_len(), 0);
    let effects = reply
        .fight_step
        .iter()
        .flat_map(|step| &step.act_effect)
        .collect::<Vec<_>>();
    assert!(effects.iter().any(|effect| {
        effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Addhandcard as i32)
            && effect.card_info.as_ref().and_then(|card| card.skill_id) == Some(31446012)
    }));
    assert!(effects.iter().any(|effect| {
        effect.effect_type
            == Some(sonettobuf::effect_type_enum::EffectType::Buffactinfoupdate as i32)
            && effect.buff_act_info.as_ref().is_some_and(|info| {
                info.act_id == Some(10030)
                    && info.param
                        == [
                            4, 1, 31495201, 31495211, 31446012, 31446022, 31446013, 31446023,
                        ]
            })
    }));
    assert!(
        runtime
            .use_cloth_skill(UseClothSkillRequest {
                skill_id: Some(0),
                from_id: Some(10),
                to_id: Some(2),
                r#type: Some(ClothSkillType::TwinsSelect as i32),
            })
            .is_none()
    );
}
