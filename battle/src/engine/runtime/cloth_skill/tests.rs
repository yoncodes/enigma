use super::*;
use crate::engine::{
    manager::card::{CardCommand, CardSetTeamCards, CardSetup},
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};
use sonettobuf::{
    BuffInfo, CardInfo, Fight, FightEntityInfo, FightTeam, effect_type_enum::EffectType,
};

#[test]
fn values_match_lua_fight_enum() {
    assert_eq!(ClothSkillType::try_from(1), Ok(ClothSkillType::HeroUpgrade));
    assert_eq!(ClothSkillType::try_from(2), Ok(ClothSkillType::Rouge));
    assert_eq!(
        ClothSkillType::try_from(8),
        Ok(ClothSkillType::BattleSelection)
    );
    assert_eq!(ClothSkillType::try_from(9), Ok(ClothSkillType::ExtraRound));
    assert_eq!(
        ClothSkillType::try_from(10),
        Ok(ClothSkillType::TwinsSelect)
    );
}

#[test]
fn crystal_selection_confirms_choice_and_reveals_existing_hand() {
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                model_id: Some(3134),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31340008),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = BattleRuntime::new(
        crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
        fight,
    )
    .unwrap();
    let card = CardInfo {
        uid: Some(10),
        skill_id: Some(31340111),
        ..Default::default()
    };
    let ultimate = CardInfo {
        uid: Some(10),
        skill_id: Some(31340131),
        hero_id: Some(3134),
        ..Default::default()
    };
    runtime
        .managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![card.clone()],
            draw_pile: vec![card.clone()],
            deck_num: 64,
        }))
        .unwrap();
    runtime
        .managers
        .execute_card(CardCommand::SetTeamCards(CardSetTeamCards {
            origin: CommandOrigin {
                domain: RuleDomain::Lifecycle,
                key: DefinitionKey::new(0, "TestTeamCards"),
            },
            cards: vec![ultimate.clone()],
        }))
        .unwrap();
    let round = runtime
        .use_cloth_skill(UseClothSkillRequest {
            r#type: Some(ClothSkillType::SelectCrystal as i32),
            skill_id: Some(0),
            from_id: Some(10),
            to_id: Some(110),
        })
        .unwrap()
        .round
        .unwrap();
    assert_eq!(round.fight_step.len(), 2);
    let selection = &round.fight_step[0];
    assert_eq!(selection.from_id, Some(10));
    let [choice, marker] = selection.act_effect.as_slice() else {
        panic!("expected selection and updated crystal state");
    };
    assert_eq!(choice.effect_type, Some(EffectType::Crystalselect as i32));
    assert_eq!(choice.effect_num, Some(110));
    assert_eq!(choice.reserve_str.as_deref(), Some("110"));
    assert_eq!(
        marker.effect_type,
        Some(EffectType::Buffactinfoupdate as i32)
    );
    assert_eq!(marker.reserve_id, Some(20));
    assert_eq!(marker.buff_act_info.as_ref().unwrap().param, [2, 2, 1]);
    let [hand, deck] = round.fight_step[1].act_effect.as_slice() else {
        panic!("expected existing hand and deck count");
    };
    assert_eq!(hand.effect_type, Some(EffectType::Cardspush as i32));
    assert!(
        hand.card_info_list
            .iter()
            .map(|visible| (visible.uid, visible.skill_id, visible.hero_id))
            .eq([&card, &ultimate].into_iter().map(|visible| (
                visible.uid,
                visible.skill_id,
                visible.hero_id
            )))
    );
    assert_eq!(deck.effect_type, Some(EffectType::Carddecknum as i32));
    assert_eq!(deck.effect_num, Some(64));
    assert_eq!(runtime.managers.emanation.counts(10), [1, 1, 0]);
    assert_eq!(runtime.card_hand(), &[card]);
    assert_eq!(runtime.managers.card.team_cards(), &[ultimate]);
    assert_eq!(runtime.managers.card.deck_num(), 64);
}
