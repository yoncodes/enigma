use sonettobuf::{BuffInfo, effect_type_enum::EffectType};

use super::*;

#[test]
fn scaled_shield_stacks_before_its_configured_duration_refresh() {
    crate::test_support::init_config();
    let entity = |uid, buffs, shield_value| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(10_000),
        shield_value: Some(shield_value),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        buffs,
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    skill_group2: vec![31270121],
                    ..entity(
                        10,
                        vec![BuffInfo {
                            uid: Some(1_002),
                            buff_id: Some(31270201),
                            from_uid: Some(10),
                            layer: Some(2),
                            ..Default::default()
                        }],
                        0,
                    )
                },
                entity(
                    11,
                    vec![BuffInfo {
                        uid: Some(1_004),
                        buff_id: Some(31270005),
                        from_uid: Some(10),
                        duration: Some(1),
                        ..Default::default()
                    }],
                    1_000,
                ),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, vec![], 0)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31270121], []);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 31270121,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    invocation.mode = SkillExecutionMode::Active;
    invocation.card_index = 1;
    let result = run_action(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [],
        invocation,
    )
    .unwrap();
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let action = steps
        .iter()
        .find(|step| step.act_id == Some(31270121))
        .unwrap();
    let effects = &action.act_effect;
    let update = |effect: &sonettobuf::ActEffect, uid| {
        effect.effect_type == Some(EffectType::Buffupdate as i32)
            && effect.buff.as_ref().and_then(|buff| buff.uid) == Some(uid)
    };
    let supply = effects
        .iter()
        .position(|effect| {
            effect.target_id == Some(11)
                && effect.effect_type == Some(EffectType::Changeshield as i32)
        })
        .unwrap();
    let consume = effects
        .iter()
        .position(|effect| update(effect, 1_002))
        .unwrap();
    let refreshes: Vec<_> = effects
        .iter()
        .enumerate()
        .filter(|(_, effect)| update(effect, 1_004))
        .map(|(index, _)| index)
        .collect();
    let [refresh] = refreshes.as_slice() else {
        panic!("expected a single duration refresh, got {refreshes:?}");
    };
    assert!(supply < consume && consume < *refresh);
    assert_eq!(managers.hp.shield(11), 2_500);
    assert_eq!(managers.buff.snapshot(11, 1_004).unwrap().duration, Some(3));
}
