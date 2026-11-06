use super::*;

#[test]
fn depleted_wave_resets_the_empty_defender_area_before_invalidation() {
    let (mut runtime, _) = wave_clear_runtime(Vec::new());
    runtime.fight.defender.as_mut().unwrap().device_area = Some(Default::default());
    runtime.managers.conduit =
        crate::engine::manager::conduit::ConduitManager::seed(&runtime.fight);
    let ai_choices = runtime
        .fight
        .defender
        .as_ref()
        .unwrap()
        .entitys
        .iter()
        .map(|entity| determinism::AiSkillChoice {
            source_uid: entity.uid.unwrap(),
            skill_id: entity.skill_group1[0],
            target_uid: 10,
        })
        .collect();
    seed_ai_turn(&mut runtime, ai_choices);
    runtime.managers.hp.lose(-1, i32::MAX, 10);
    runtime.managers.hp.lose(-2, i32::MAX, 10);

    let round = runtime
        .build_begin_round_from_schedule(&BeginRoundRequest::default())
        .unwrap();

    let effects = round
        .fight_step
        .iter()
        .flat_map(|step| &step.act_effect)
        .collect::<Vec<_>>();
    let resets = effects
        .iter()
        .enumerate()
        .filter(|(_, effect)| {
            effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Devicepowerclear as i32)
                && effect.team_type == Some(2)
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let invalidation = effects
        .iter()
        .position(|effect| {
            effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Cardinvalid as i32)
                && effect.team_type == Some(2)
        })
        .unwrap();

    assert_eq!(resets.len(), 1);
    assert!(resets[0] < invalidation);
    assert_eq!(runtime.fight.cur_wave, Some(2));
}
