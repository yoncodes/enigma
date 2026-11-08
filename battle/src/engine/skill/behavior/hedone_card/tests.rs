use sonettobuf::{ActEffect, Fight, FightEntityInfo, FightTeam, effect_type_enum::EffectType};

use crate::engine::{
    manager::{
        BattleManagers,
        card::{CardAddHedone, CardCommand, CardSetup},
    },
    packet::timeline,
    runtime::{determinism::RoundDeterminism, drain},
    skill::{
        action::SkillModifiers,
        behavior::{BehaviorOpContext, classify::BehaviorSpec, registry::BehaviorHandler},
        effect::{ParsedBehavior, SkillEffectCatalog},
        target::{TargetContext, TargetPool},
    },
};

use super::*;

fn effects<'a>(steps: &'a [sonettobuf::FightStep], output: &mut Vec<&'a ActEffect>) {
    for step in steps {
        for effect in &step.act_effect {
            output.push(effect);
            if let Some(child) = &effect.fight_step {
                effects(std::slice::from_ref(child), output);
            }
        }
    }
}

#[test]
fn exact_two_argument_form_grants_the_carrier_then_adds_the_locked_card() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-4),
                model_id: Some(3156),
                team_type: Some(1),
                current_hp: Some(10_000),
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
            hand: Vec::new(),
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60333, "HedoneAddCard"),
        vec![31560181, 31560180],
        vec!["31560181".into(), "31560180".into()],
    );
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();
    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: -4,
            source_team: 1,
            target_uid: -4,
            active_skill_id: 31560131,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();
    let result = drain::run_command_group(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut determinism,
        TargetContext::default(),
        ops,
    )
    .unwrap();

    let buff = managers
        .buff
        .active_for(-4)
        .find(|buff| buff.buff_id == Some(31560180))
        .unwrap();
    assert!(
        buff.act_info
            .iter()
            .any(|info| { info.act_id == Some(1159) && info.param == [0, 7, 0] })
    );
    let [card] = managers.card.hand() else {
        panic!("HedoneAddCard must add exactly one card")
    };
    let card = card.clone();
    assert_eq!(card.uid, Some(-4));
    assert_eq!(card.skill_id, Some(31560181));
    assert_eq!(card.hero_id, Some(3156));
    assert!([5, 8].into_iter().all(|key| {
        card.extra_infos
            .iter()
            .any(|info| info.key == Some(key) && info.values == ["HedoneCard"])
    }));

    let projected = timeline::project_for_version(&result.frames, 7).unwrap();
    let mut projected_effects = Vec::new();
    effects(&projected, &mut projected_effects);
    let state = projected_effects
        .iter()
        .position(|effect| effect.effect_type == Some(EffectType::Buffactinfoupdate as i32))
        .unwrap();
    let buff_add = projected_effects
        .iter()
        .position(|effect| effect.effect_type == Some(EffectType::Buffadd as i32))
        .unwrap();
    let card_add = projected_effects
        .iter()
        .position(|effect| effect.effect_type == Some(EffectType::Addhandcard as i32))
        .unwrap();
    let card_update = projected_effects
        .iter()
        .position(|effect| effect.effect_type == Some(EffectType::Updatecarddata as i32))
        .unwrap();
    assert!(state < buff_add && buff_add < card_add && card_add + 1 == card_update);
    assert_eq!(projected_effects[card_add].effect_num, Some(1));
    assert_eq!(projected_effects[card_update].effect_num, Some(1));
    assert_eq!(
        projected_effects[card_add]
            .card_info
            .as_ref()
            .and_then(|card| card.skill_id),
        card.skill_id
    );
    assert_eq!(
        projected_effects[card_update]
            .card_info
            .as_ref()
            .and_then(|card| card.skill_id),
        card.skill_id
    );

    let mut target = TargetContext::default();
    assert!(
        Handler::emit_ops(
            BehaviorOpContext {
                source_uid: -4,
                source_team: 1,
                target_uid: -4,
                active_skill_id: 31560131,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        )
        .unwrap()
        .is_empty()
    );
    assert!(
        managers
            .execute_card(CardCommand::AddHedone(CardAddHedone {
                origin: super::super::command_origin(&behavior).unwrap(),
                owner_uid: -4,
                skill_id: 31560181,
                hero_id: 3156,
                team_type: 1,
            }))
            .is_err()
    );
}

#[test]
fn optional_starting_energy_remains_a_separate_unsupported_contract() {
    assert!(Handler::supports(&ParsedBehavior::new(
        60333,
        "HedoneAddCard",
        vec![31560181, 31560180],
    )));
    assert!(!Handler::supports(&ParsedBehavior::new(
        60333,
        "HedoneAddCard",
        vec![31560181, 31560181, 2],
    )));
}
