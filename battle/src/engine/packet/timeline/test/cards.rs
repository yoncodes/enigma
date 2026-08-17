use super::*;

#[test]
fn card_consumption_projects_its_owned_wire_effect() {
    let mut cards = CardManager::new(vec![sonettobuf::CardInfo {
        uid: Some(10),
        skill_id: Some(100),
        card_effect: Some(1),
        ..Default::default()
    }]);
    cards.seed(&sonettobuf::Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                uid: Some(10),
                skill_group1: vec![100, 101, 102],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let changes = cards
        .execute_command(CardCommand::ConsumeForEffect(
            crate::engine::manager::card::CardConsumeForEffect {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(60222, "ConsumeCardAddBuff"),
                },
                owner_uid: 10,
                indices: vec![0],
            },
        ))
        .unwrap();

    let effects = project_change_for_test(&BattleChange::Card(Box::new(changes))).unwrap();

    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].effect_type, Some(EffectType::Cardremove as i32));
    assert_eq!(effects[0].reserve_str.as_deref(), Some("1"));
}

#[test]
fn hand_rank_change_projects_the_committed_card_and_resource_state() {
    use crate::engine::manager::{
        card::{CardSetup, HandCardRankUp},
        eureka::{EUREKA_RESOURCE_ID, EurekaChange, EurekaCommand},
    };

    crate::test_support::init_config();
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(50034, "ConsumePowerUpgradeSkillCard"),
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                skill_group1: vec![30650221, 30650222, 30650223],
                power_infos: vec![sonettobuf::PowerInfo {
                    power_id: Some(EUREKA_RESOURCE_ID),
                    num: Some(2),
                    max: Some(6),
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![CardInfo {
                uid: Some(10),
                skill_id: Some(30650221),
                ..Default::default()
            }],
            draw_pile: Vec::new(),
            deck_num: 1,
        }))
        .unwrap();
    managers
        .execute_eureka(EurekaCommand::Change(EurekaChange {
            origin,
            source_uid: 10,
            target_uid: 10,
            power_id: EUREKA_RESOURCE_ID,
            delta: -2,
            effect_type: EffectType::Powerchange as i32,
        }))
        .unwrap();
    let changes = managers
        .execute_card(CardCommand::RankUpHand(HandCardRankUp {
            origin,
            owner_uid: 10,
            hand_index: 0,
        }))
        .unwrap();

    let effects = project_change_for_test(&BattleChange::Card(Box::new(changes))).unwrap();

    assert_eq!(effects.len(), 1);
    let effect = &effects[0];
    assert_eq!(effect.effect_type, Some(EffectType::Cardlevelchange as i32));
    assert_eq!(effect.target_id, Some(1));
    assert_eq!(effect.effect_num, Some(30650222));
    assert_eq!(effect.config_effect, Some(50034));
    assert_eq!(
        effect
            .entity
            .as_ref()
            .and_then(|entity| entity.power_infos.first())
            .and_then(|power| power.num),
        Some(0)
    );
}

#[test]
fn owner_skill_group_replacement_commits_state_and_projects_changed_hand_cards_first() {
    use crate::engine::manager::card::{CardReplaceOwnerSkills, CardSetup};

    crate::test_support::init_config();
    let base_group1 = vec![31460114, 31460115, 31460116];
    let base_group2 = vec![31460127, 31460128, 31460129];
    let replacement_group1 = vec![31460214, 31460215, 31460216];
    let replacement_group2 = vec![31460227, 31460228, 31460229];
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3146),
                team_type: Some(1),
                current_hp: Some(100),
                skill_group1: base_group1.clone(),
                skill_group2: base_group2.clone(),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let origin = CommandOrigin {
        domain: RuleDomain::BuffAct,
        key: DefinitionKey::new(1138, "ReplaceEntitySkillGroup"),
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![CardInfo {
                uid: Some(10),
                hero_id: Some(3146),
                skill_id: Some(31460114),
                ..Default::default()
            }],
            draw_pile: vec![CardInfo {
                uid: Some(10),
                hero_id: Some(3146),
                skill_id: Some(31460127),
                ..Default::default()
            }],
            deck_num: 2,
        }))
        .unwrap();

    let changes = managers
        .execute_card(CardCommand::ReplaceOwnerSkills(CardReplaceOwnerSkills {
            origin,
            owner_uid: 10,
            base_group1: base_group1.clone(),
            base_group2: base_group2.clone(),
            replacement_group1: replacement_group1.clone(),
            replacement_group2: replacement_group2.clone(),
        }))
        .unwrap();

    assert_eq!(managers.card.hand()[0].skill_id, Some(31460214));
    let entity = managers.entity_snapshot(10).unwrap();
    assert_eq!(entity.skill_group1, replacement_group1);
    assert_eq!(entity.skill_group2, replacement_group2);
    let effects = project_change_for_test(&BattleChange::Card(Box::new(changes))).unwrap();
    assert_eq!(
        effects
            .iter()
            .map(|effect| effect.effect_type)
            .collect::<Vec<_>>(),
        vec![
            Some(EffectType::Cardaconvertcardb as i32),
            Some(EffectType::Heroupgrade as i32),
        ]
    );
    let converted = &effects[0];
    assert_eq!(converted.target_id, Some(10));
    assert_eq!(converted.effect_num, Some(1));
    assert_eq!(converted.config_effect, Some(31460214));
    assert_eq!(converted.reserve_id, Some(1));
    assert_eq!(converted.team_type, Some(1));
    assert_eq!(
        converted.card_info.as_ref().and_then(|card| card.skill_id),
        Some(31460214)
    );
    assert_eq!(effects[1].target_id, Some(10));
    assert_eq!(effects[1].effect_num, Some(0));
    assert_eq!(
        effects[1]
            .entity
            .as_ref()
            .map(|entity| entity.skill_group1.as_slice()),
        Some([31460214, 31460215, 31460216].as_slice())
    );

    let restored = managers
        .execute_card(CardCommand::ReplaceOwnerSkills(CardReplaceOwnerSkills {
            origin,
            owner_uid: 10,
            base_group1: vec![31460214, 31460215, 31460216],
            base_group2: vec![31460227, 31460228, 31460229],
            replacement_group1: base_group1.clone(),
            replacement_group2: base_group2.clone(),
        }))
        .unwrap();
    assert_eq!(managers.card.hand()[0].skill_id, Some(31460114));
    let entity = managers.entity_snapshot(10).unwrap();
    assert_eq!(entity.skill_group1, base_group1);
    assert_eq!(entity.skill_group2, base_group2);
    assert_eq!(
        project_change_for_test(&BattleChange::Card(Box::new(restored)))
            .unwrap()
            .iter()
            .map(|effect| effect.effect_type)
            .collect::<Vec<_>>(),
        vec![
            Some(EffectType::Cardaconvertcardb as i32),
            Some(EffectType::Heroupgrade as i32),
        ]
    );
}
