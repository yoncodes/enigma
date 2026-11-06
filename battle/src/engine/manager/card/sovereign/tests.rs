use super::*;
use crate::engine::{
    event::payload::BattleEvent,
    manager::card::{CARD_PLAY_ORIGIN, CardCommand, CardPlay, CardPlayChoice},
    packet::card::CardPacket,
};

#[test]
fn moxie_cleanup_keeps_sovereign() {
    use crate::engine::{
        manager::BattleManagers,
        runtime::{determinism::RoundDeterminism, drain},
        skill::{
            action::{SkillInvocation, SkillRequest},
            effect::SkillEffectCatalog,
            rule::output::RuleOp,
            target::{TargetContext, TargetPool},
        },
    };
    use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute};

    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3147),
                entity_type: Some(1),
                team_type: Some(1),
                level: Some(150),
                current_hp: Some(1000),
                attr: Some(HeroAttribute {
                    hp: Some(1000),
                    ..Default::default()
                }),
                ex_skill: Some(31473141),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.card = CardManager::new(vec![CardInfo {
        uid: Some(10),
        skill_id: Some(31473141),
        ..Default::default()
    }]);
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
    let sovereign = managers.card.hand()[0].clone();
    let physical_id = managers.card.deck.hand_id(0);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31470163], []);
    drain::run_command_group(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(SkillInvocation::from(SkillRequest {
            source_uid: 10,
            skill_id: 31470163,
        }))],
    )
    .unwrap();
    assert_eq!(managers.ex_point.get(10), 3);
    assert_eq!(managers.card.hand(), &[sovereign]);
    assert_eq!(managers.card.deck.hand_id(0), physical_id);
}

#[test]
fn sovereign_insertion_keeps_physical_identity_and_denies_the_resolved_locked_source() {
    let normal = CardInfo {
        uid: Some(10),
        skill_id: Some(100),
        temp_card: Some(false),
        ..Default::default()
    };
    for (hand_size, position) in [(4, 4), (6, 6), (10, 8)] {
        let mut manager = CardManager::new(vec![normal.clone(); hand_size]);
        let original_ids: Vec<_> = (0..hand_size)
            .map(|index| manager.deck.hand_id(index).unwrap())
            .collect();
        let insert = CardCommand::InsertSovereign(CardInsertSovereign {
            origin: CARD_PLAY_ORIGIN,
            target_uid: 10,
            skill_id: 31472131,
            hero_id: 3147,
            max_position: 9,
            team_type: 1,
        });
        let CardCommand::InsertSovereign(mut opposing) = insert.clone() else {
            unreachable!();
        };
        opposing.team_type = 2;
        let before = manager.clone();
        assert_eq!(
            manager.execute_command(CardCommand::InsertSovereign(opposing)),
            Err(CardCommandError::InvalidCommand)
        );
        assert_eq!(manager, before);
        let changes = manager.execute_command(insert.clone()).unwrap();
        let card = manager.hand()[position].clone();
        let physical_id = manager.deck.hand_id(position).unwrap();
        assert!(!original_ids.contains(&physical_id));
        let remaining_ids: Vec<_> = (0..manager.hand().len())
            .filter(|index| *index != position)
            .map(|index| manager.deck.hand_id(index).unwrap())
            .collect();
        assert_eq!(remaining_ids, original_ids);
        assert_eq!(
            (card.uid, card.hero_id, card.card_type, card.temp_card),
            (
                Some(10),
                Some(3147),
                Some(CardType::Skill3 as i32),
                Some(false)
            )
        );
        let value: serde_json::Value =
            serde_json::from_str(card.card_dataes[0].value.as_ref().unwrap()).unwrap();
        assert_eq!(
            value,
            serde_json::json!({"lock":true,"strengthen":{"1":0,"2":0,"3":0,"4":0}})
        );
        assert_eq!(
            card.extra_infos
                .iter()
                .map(|info| info.key.unwrap())
                .collect::<Vec<_>>(),
            vec![1, 4, 5]
        );
        assert!(
            card.extra_infos
                .iter()
                .all(|info| info.values == ["UnnamedSpCard"])
        );
        assert!(!manager.can_use_card(&card));
        let mut payload_only = card.clone();
        payload_only.extra_infos.clear();
        assert!(!manager.can_use_card(&payload_only));
        let mut metadata_only = normal.clone();
        metadata_only.extra_infos = vec![CardExtraInfo {
            key: Some(5),
            values: Vec::new(),
        }];
        assert!(!manager.can_use_card(&metadata_only));
        assert!(manager.generated().is_empty() && manager.draw_pile().is_empty());
        let projected = CardPacket::from_change(changes.operation.clone().unwrap());
        assert_eq!(
            projected.effect_type,
            Some(sonettobuf::effect_type_enum::EffectType::Inserthandcard as i32)
        );
        assert_eq!(
            (
                projected.effect_num,
                projected.target_id,
                projected.team_type
            ),
            (Some(position as i32 + 1), Some(10), Some(1))
        );
        assert_eq!(projected.card_info, Some(card.clone()));
        assert!(changes.events().iter().any(|event| matches!(event,
            BattleEvent::SovereignCardCreated(created) if created.owner_uid == 10 && created.index == position
                && created.team_type == 1 && event.target_uid() == Some(10))));
        let before = manager.clone();
        assert!(manager.execute_command(insert).unwrap().operation.is_none());
        assert_eq!(manager, before);
        assert!(!manager.move_card(position, 0));
        assert!(manager.compose(&[0, position], normal.clone()).is_none());
        assert_eq!(manager, before);
        let mut source = card.clone();
        source.card_dataes.clear();
        source.extra_infos.clear();
        let play = CardPlay {
            origin: CARD_PLAY_ORIGIN,
            hand_index: 0,
            target_uid: None,
            chosen_skill_id: None,
            choice: Some(CardPlayChoice {
                source: source.clone(),
                played: normal.clone(),
            }),
            recorded_skill: None,
        };
        assert_eq!(
            manager.check_play_permission(play.hand_index, play.choice.as_ref()),
            Err(CardCommandError::PlayForbidden)
        );
        let mut control = manager.clone();
        control.deck.hand_mut()[0] = source.clone();
        let mut stale = play.clone();
        stale.hand_index = 1;
        let before_control = control.clone();
        assert_eq!(
            control.execute_command(CardCommand::Play(stale)),
            Err(CardCommandError::PlayForbidden)
        );
        assert_eq!(control, before_control);
        let chosen = control
            .execute_command(CardCommand::Play(play.clone()))
            .unwrap();
        assert_eq!(chosen.played.unwrap().card, source);
        assert_eq!(control.deck.hand_id(position - 1), Some(physical_id));
        assert_eq!(
            manager.execute_command(CardCommand::Play(play)),
            Err(CardCommandError::PlayForbidden)
        );
        assert_eq!(manager, before);
        assert!(manager.move_card(0, position));
        assert_eq!(manager.deck.hand_id(position - 1), Some(physical_id));
        let playable: Vec<_> = manager
            .hand()
            .iter()
            .enumerate()
            .filter(|(_, card)| manager.can_use_card(card))
            .map(|(index, _)| index)
            .collect();
        assert!(!playable.contains(&(position - 1)) && playable.contains(&position));
        let played = manager
            .execute_command(CardCommand::Play(CardPlay {
                origin: CARD_PLAY_ORIGIN,
                hand_index: position,
                target_uid: None,
                chosen_skill_id: None,
                choice: None,
                recorded_skill: None,
            }))
            .unwrap();
        assert_eq!(played.played.unwrap().card, normal);
        assert_eq!(manager.deck.hand_id(position - 1), Some(physical_id));
    }
}
