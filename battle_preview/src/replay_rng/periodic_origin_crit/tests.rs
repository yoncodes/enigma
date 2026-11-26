use sonettobuf::FightHurtInfo;

use super::*;

#[test]
fn identifies_the_exact_periodic_crit_and_leaves_repeated_rolls_unseeded() {
    crate::init_test_config();
    let step = |target_uid| FightStep {
        act_type: Some(ActType::Effect as i32),
        act_id: Some(31570012),
        from_id: Some(-4),
        to_id: Some(target_uid),
        act_effect: vec![ActEffect {
            target_id: Some(target_uid),
            effect_type: Some(EffectType::Origincrit as i32),
            buff_act_id: Some(1156),
            hurt_info: Some(FightHurtInfo {
                from_uid: Some(-4),
                buff_act_id: Some(1156),
                buff_uid: Some(1012),
                damage_from_type: Some(DamageFromType::Buff as i32),
                hurt_effect: Some(EffectType::Origincrit as i32),
                ..Default::default()
            }),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut wrong_carrier = step(-7);
    wrong_carrier.act_id = Some(31570011);
    let mut wrong_config_effect = step(-8);
    wrong_config_effect.act_effect[0].config_effect = Some(60127);
    let mut wrong_frame = step(-9);
    wrong_frame.act_type = Some(ActType::Skill as i32);
    let mut wrong_damage_lane = step(-10);
    wrong_damage_lane.act_effect[0]
        .hurt_info
        .as_mut()
        .unwrap()
        .damage_from_type = Some(DamageFromType::Skill as i32);
    let mut conflicting_crit = step(-11);
    conflicting_crit.act_effect[0]
        .hurt_info
        .as_mut()
        .unwrap()
        .hurt_effect = Some(EffectType::Origindamage as i32);
    let round = FightRound {
        fight_step: vec![
            step(-5),
            step(-5),
            step(-6),
            wrong_carrier,
            wrong_config_effect,
            wrong_frame,
            wrong_damage_lane,
            conflicting_crit,
        ],
        ..Default::default()
    };
    assert_eq!(
        choices(config::configs::get(), &round),
        vec![((31570012, -4, -6), true)]
    );
}
