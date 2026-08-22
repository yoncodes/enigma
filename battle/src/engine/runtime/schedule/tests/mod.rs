use sonettobuf::{BuffInfo, CardInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute, PowerInfo};

use super::*;
use crate::engine::{
    manager::{
        card::CardPlay,
        field::{FieldCommand, FieldDefinition, FieldOperation},
    },
    mechanic::impromptu::{build_plan, enable_rule_ops, inspiration_key, team_energy_key},
    skill::{
        behavior::classify::BehaviorSpec,
        buff_act,
        condition::{
            ParsedCondition, ParsedConditionKind, buff::BuffConditionMode, none::NoneMode,
        },
        effect::{ParsedBehavior, ParsedSkillEffect, SkillEffectSlot},
        rule::{CommandOrigin, DefinitionKey, RuleDomain, route::ConditionRoute},
        target::TargetRequest,
    },
};
use crate::test_support::init_config;

fn buff_gated_generic_temp_card_fixture() -> (Fight, SkillEffectCatalog) {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3087),
                team_type: Some(1),
                current_hp: Some(100),
                passive_skill: vec![40],
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(30870331),
                    from_uid: Some(10),
                    duration: Some(3),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let behavior = ParsedBehavior::new(50031, "AddSpTempCard", vec![30870391]);
    let mut slot = SkillEffectSlot::new(behavior, TargetRequest::self_only());
    slot.conditions = vec![ParsedCondition {
        opcode: 19106,
        type_name: "HasBuffId".into(),
        kind: ParsedConditionKind::BuffId {
            mode: BuffConditionMode::Present,
            buff_ids: vec![30870131],
        },
        raw_args: vec!["30870131".into()],
    }];
    slot.compiled_route =
        ConditionRoute::compile_for_behavior(&slot.conditions, &slot.behavior.spec);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 40,
        slots: vec![slot],
    });
    (fight, catalog)
}

fn assert_buff_gated_generic_temp_card(managers: &BattleManagers, result: &DrainResult) {
    let card = managers
        .card
        .hand()
        .iter()
        .find(|card| card.skill_id == Some(30870391))
        .unwrap();
    assert_eq!(card.uid, Some(0));
    assert_eq!(card.hero_id, Some(0));
    assert_eq!(card.card_type, Some(0));
    assert_eq!(card.temp_card, Some(true));

    fn collect_card_effects(
        step: &sonettobuf::FightStep,
        effects: &mut Vec<(Option<i32>, Option<i32>)>,
    ) {
        for effect in &step.act_effect {
            if matches!(
                effect.effect_type,
                Some(value)
                    if value == sonettobuf::effect_type_enum::EffectType::Spcardadd as i32
                        || value
                            == sonettobuf::effect_type_enum::EffectType::Changetotempcard as i32
            ) {
                effects.push((effect.effect_type, effect.config_effect));
            }
            if let Some(child) = effect.fight_step.as_ref() {
                collect_card_effects(child, effects);
            }
        }
    }
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let mut effects = Vec::new();
    for step in &steps {
        collect_card_effects(step, &mut effects);
    }
    assert_eq!(
        effects,
        vec![
            (
                Some(sonettobuf::effect_type_enum::EffectType::Spcardadd as i32),
                Some(50031),
            ),
            (
                Some(sonettobuf::effect_type_enum::EffectType::Changetotempcard as i32),
                Some(50031),
            ),
        ]
    );
}

mod actions;
mod entry;
mod mechanics;
mod refill;
mod round_start;
mod settlement;
mod start;
