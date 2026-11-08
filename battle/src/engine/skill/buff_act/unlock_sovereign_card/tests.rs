use crate::engine::{
    manager::{
        BattleManagers,
        card::{CARD_PLAY_ORIGIN, CardCommand, CardSetup},
    },
    packet::timeline,
    runtime::{determinism::RoundDeterminism, schedule},
    skill::{
        effect::SkillEffectCatalog,
        target::{TargetContext, TargetPool},
    },
};
use sonettobuf::{
    BuffInfo, CardData, CardExtraInfo, CardInfo, Fight, FightEntityInfo, FightStep, FightTeam,
    card_data::CardDataKey, effect_type_enum::EffectType,
};

fn find_step(steps: &[FightStep], act_id: i32) -> Option<&FightStep> {
    for step in steps {
        if step.act_id == Some(act_id) {
            return Some(step);
        }
        for effect in &step.act_effect {
            if let Some(child) = &effect.fight_step
                && let Some(found) = find_step(std::slice::from_ref(child), act_id)
            {
                return Some(found);
            }
        }
    }
    None
}

fn fight() -> Fight {
    Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3147),
                entity_type: Some(1),
                team_type: Some(1),
                level: Some(150),
                current_hp: Some(1000),
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(1000),
                    ..Default::default()
                }),
                passive_skill: vec![31472151, 31471154],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[test]
fn opening_unlocks_rightmost() {
    crate::test_support::init_config();
    let definition = super::super::registry::find(1134, "UnnamedUnlockSpCard").unwrap();
    assert_eq!(
        definition.runtime.event_override,
        Some(crate::engine::event::kind::EventKind::SovereignUnlock)
    );
    let supports = definition.supports.unwrap();
    assert!(supports(&[31470163]));
    assert!(!supports(&[]) && !supports(&[114370006]));
    assert!(super::super::registry::find(1134, "None").is_none());
    let fight = fight();
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    schedule::run_start(
        managers.catalog(),
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        CardSetup {
            hand: vec![
                CardInfo {
                    uid: Some(10),
                    skill_id: Some(31470111),
                    ..Default::default()
                };
                4
            ],
            draw_pile: Vec::new(),
            deck_num: 0,
        },
        4,
    )
    .unwrap();
    let card = &managers.card.hand()[0];
    assert_eq!(card.skill_id, Some(31472131));
    assert!(managers.card.can_use_card(card));
    assert_eq!(managers.ex_point.get(10), 3);
    assert!(
        managers
            .buff
            .active_for(10)
            .any(|buff| buff.buff_id == Some(31470011))
    );
}

#[test]
fn handoff_unlocks_once() {
    crate::test_support::init_config();
    let mut fight = fight();
    let owner = &mut fight.attacker.as_mut().unwrap().entitys[0];
    owner.passive_skill.clear();
    owner.buffs = vec![BuffInfo {
        uid: Some(20),
        buff_id: Some(31470007),
        from_uid: Some(10),
        ..Default::default()
    }];
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let locked = CardInfo {
        uid: Some(10),
        skill_id: Some(31472131),
        energy: Some(7),
        card_dataes: vec![CardData {
            key: Some(CardDataKey::Unnamed as i32),
            value: Some(r#"{"lock":true,"strengthen":{"1":3,"2":4,"3":4,"4":0}}"#.to_owned()),
        }],
        extra_infos: [1, 4, 5]
            .into_iter()
            .map(|key| CardExtraInfo {
                key: Some(key),
                values: if key == 5 {
                    vec!["UnnamedSpCard".to_owned(), "Other".to_owned()]
                } else {
                    vec!["UnnamedSpCard".to_owned()]
                },
            })
            .collect(),
        ..Default::default()
    };
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![
                CardInfo {
                    uid: Some(10),
                    skill_id: Some(31470111),
                    ..Default::default()
                },
                locked.clone(),
            ],
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    let handoff = |managers: &mut BattleManagers| {
        schedule::run_before_ai_round_start(
            managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            1,
            &[],
            None,
        )
        .unwrap()
    };
    handoff(&mut managers);
    assert_eq!(managers.card.hand()[1], locked);
    assert_eq!(managers.ex_point.get(10), 0);
    managers
        .execute_card(CardCommand::Dissolve {
            origin: CARD_PLAY_ORIGIN,
            card_index: 0,
        })
        .unwrap();
    assert_eq!(managers.card.hand()[0], locked);
    let result = handoff(&mut managers);
    let mut expected = locked;
    expected.card_dataes[0].value =
        Some(r#"{"lock":false,"strengthen":{"1":3,"2":4,"3":4,"4":0}}"#.to_owned());
    expected.extra_infos[2].values = vec!["Other".to_owned()];
    assert_eq!(managers.card.hand(), &[expected.clone()]);
    assert!(!managers.card.can_use_card(&expected));
    assert_eq!(managers.ex_point.get(10), 3);
    let projected = timeline::project_for_version(&result.frames, 7).unwrap();
    let unlock = find_step(&projected, 31470007).unwrap();
    assert!(!unlock.fake_timeline.unwrap_or_default());
    assert_eq!(
        unlock.act_effect[0].effect_type,
        Some(EffectType::Updatecarddata as i32)
    );
    assert_eq!(
        unlock.act_effect[0].card_info.as_ref().unwrap().card_dataes,
        expected.card_dataes
    );
    let callback = unlock.act_effect[1].fight_step.as_ref().unwrap();
    assert_eq!(callback.act_id, Some(31470163));
    assert!(!callback.fake_timeline.unwrap_or_default());
    assert_eq!(
        callback.act_effect[0].effect_type,
        Some(EffectType::Buffadd as i32)
    );
    assert_eq!(callback.act_effect[1].effect_num, Some(3));
    let repeated = handoff(&mut managers);
    assert!(
        find_step(
            &timeline::project_for_version(&repeated.frames, 7).unwrap(),
            31470007
        )
        .is_none()
    );
    assert_eq!(managers.card.hand(), &[expected]);
    assert_eq!(managers.ex_point.get(10), 3);
}
