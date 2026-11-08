use sonettobuf::{
    ActEffect, BuffActInfo, BuffInfo, CardExtraInfo, CardInfo, Fight, FightEntityInfo, FightStep,
    FightTeam, effect_type_enum::EffectType,
};

use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        card::{CardCommand, CardSetup},
    },
    packet::timeline,
    runtime::{determinism::RoundDeterminism, drain},
    skill::{
        effect::SkillEffectCatalog,
        subscriber,
        target::{TargetContext, TargetPool},
    },
};

use super::*;

fn effects<'a>(steps: &'a [FightStep], output: &mut Vec<&'a ActEffect>) {
    for step in steps {
        for effect in &step.act_effect {
            output.push(effect);
            if let Some(child) = &effect.fight_step {
                effects(std::slice::from_ref(child), output);
            }
        }
    }
}

fn card(owner_uid: i64, skill_id: i32) -> CardInfo {
    CardInfo {
        uid: Some(owner_uid),
        skill_id: Some(skill_id),
        temp_card: Some(false),
        energy: Some(0),
        ..Default::default()
    }
}

fn hedone_card(owner_uid: i64) -> CardInfo {
    CardInfo {
        hero_id: Some(3156),
        extra_infos: [5, 8]
            .into_iter()
            .map(|key| CardExtraInfo {
                key: Some(key),
                values: vec!["HedoneCard".to_owned()],
            })
            .collect(),
        ..card(owner_uid, 31560181)
    }
}

fn fight(buff_id: i32, state: [i32; 3]) -> Fight {
    Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-4),
                model_id: Some(3156),
                team_type: Some(1),
                current_hp: Some(10_000),
                buffs: vec![BuffInfo {
                    uid: Some(1108),
                    buff_id: Some(buff_id),
                    from_uid: Some(-4),
                    act_info: vec![BuffActInfo {
                        act_id: Some(1159),
                        param: state.into(),
                        str_param: Some(String::new()),
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                team_type: Some(2),
                current_hp: Some(10_000),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[test]
fn round_end_absorbs_normal_energy_through_the_registry() {
    crate::test_support::init_config();
    let fight = fight(31560185, [0, 10, 0]);
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![
                card(-4, 31446012),
                card(-4, 31447001),
                card(-4, 31446022),
                hedone_card(-4),
                card(-4, 31446062),
                card(-4, 31446012),
            ],
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [], [31560185]);

    let result = drain::run_group_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::Kind(EventKind::RoundEnd),
        crate::engine::event::dispatcher::ReactionLane::BuffActs,
        Some(&[-4]),
    )
    .unwrap();

    assert_eq!(
        managers
            .card
            .hand()
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        [31447001, 31560181, 31446012]
    );
    let special = managers
        .card
        .hand()
        .iter()
        .find(|card| card.skill_id == Some(31560181))
        .unwrap();
    assert!(special.extra_infos.iter().any(|info| info.key == Some(8)));
    assert!(!special.extra_infos.iter().any(|info| info.key == Some(5)));
    assert!(managers.buff.has_buff_id(-4, 31560152));
    assert_eq!(
        owner_state(&managers, -4).unwrap(),
        Some(HedoneCardState {
            owner_uid: -4,
            buff_uid: 1108,
            act_id: 1159,
            team_type: 1,
            stored: 6,
            maximum: 10,
            unlocked: true,
            config: parse(&[31560181, 3, 31560152, 1, 6, 10, 1, 1]).unwrap(),
        })
    );

    let projected = timeline::project_for_version(&result.frames, 7).unwrap();
    let mut projected_effects = Vec::new();
    effects(&projected, &mut projected_effects);
    assert!(projected_effects.iter().any(|effect| {
        effect.effect_type == Some(EffectType::Cardremove as i32)
            && effect.effect_num == Some(2)
            && effect.reserve_str.as_deref() == Some("1#3#5")
    }));
    assert!(projected_effects.iter().any(|effect| {
        effect.effect_type == Some(EffectType::Updatecarddata as i32)
            && effect.effect_num == Some(2)
            && effect
                .card_info
                .as_ref()
                .is_some_and(|card| card.skill_id == Some(31560181))
    }));
    assert!(projected_effects.iter().any(|effect| {
        effect.effect_type == Some(EffectType::Buffactinfoupdate as i32)
            && effect
                .buff_act_info
                .as_ref()
                .is_some_and(|info| info.act_id == Some(1159) && info.param == [6, 10, 1])
    }));
    assert_eq!(
        projected_effects
            .iter()
            .filter(|effect| {
                effect.effect_type == Some(EffectType::Buffadd as i32)
                    && effect.effect_num == Some(31560152)
            })
            .count(),
        3
    );
    let remove = projected_effects
        .iter()
        .position(|effect| effect.effect_type == Some(EffectType::Cardremove as i32))
        .unwrap();
    let rewards = projected_effects
        .iter()
        .enumerate()
        .filter_map(|(index, effect)| {
            (effect.effect_type == Some(EffectType::Buffadd as i32)
                && effect.effect_num == Some(31560152))
            .then_some(index)
        })
        .collect::<Vec<_>>();
    let unlock = projected_effects
        .iter()
        .position(|effect| effect.effect_type == Some(EffectType::Updatecarddata as i32))
        .unwrap();
    let state = projected_effects
        .iter()
        .position(|effect| {
            effect.effect_type == Some(EffectType::Buffactinfoupdate as i32)
                && effect
                    .buff_act_info
                    .as_ref()
                    .is_some_and(|info| info.act_id == Some(1159))
        })
        .unwrap();
    assert_eq!(rewards.len(), 3);
    assert!(remove < rewards[0]);
    assert!(rewards.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(rewards[2] < unlock && unlock < state);
}

#[test]
fn power_add_is_preserved_for_60334_without_scaling_stored_energy() {
    crate::test_support::init_config();
    let fight = fight(31560187, [0, 12, 0]);
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![
                card(-4, 31446012),
                card(-4, 31446022),
                card(-4, 31446062),
                hedone_card(-4),
            ],
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [], [31560187]);

    drain::run_group_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::Kind(EventKind::RoundEnd),
        crate::engine::event::dispatcher::ReactionLane::BuffActs,
        Some(&[-4]),
    )
    .unwrap();

    let state = owner_state(&managers, -4).unwrap().unwrap();
    assert_eq!(state.stored, 6);
    assert_eq!(state.maximum, 12);
    assert_eq!(state.config.power_add, 2);
}

#[test]
fn grammar_requires_the_complete_hedone_contract() {
    assert!(supports(&[31560181, 3, 31560152, 1, 6, 10, 1, 1]));
    assert!(!supports(&[31560181, 3, 31560152, 1, 6, 10, 1]));
    assert!(!supports(&[31560181, 3, 31560152, 2, 6, 10, 1, 1]));
    assert!(!supports(&[31560181, 3, 31560152, 1, 6, 10, 11, 1]));
}
