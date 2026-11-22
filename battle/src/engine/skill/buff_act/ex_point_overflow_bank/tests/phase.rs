use sonettobuf::effect_type_enum::EffectType;

use super::*;
use crate::engine::{
    packet::timeline::project_for_version,
    runtime::{
        determinism::RoundDeterminism,
        drain::{ReactionLane, run_group_event_lanes},
    },
    skill::{
        effect::SkillEffectCatalog,
        target::{TargetContext, TargetPool},
    },
};

#[test]
fn round_end_bank_owns_a_gain_that_entirely_overflows() {
    crate::test_support::init_config();
    let ally = |uid, ex_point| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100),
        ex_point: Some(ex_point),
        passive_skill: vec![31390151],
        buffs: vec![BuffInfo {
            uid: Some(uid + 10),
            buff_id: Some(31270400),
            from_uid: Some(uid),
            ..Default::default()
        }],
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![ally(10, 5), ally(11, 4)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31390151], [31270400]);
    let result = run_group_event_lanes(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::Kind(EventKind::RoundEnd),
        &[ReactionLane::Skills],
        Some(&[10, 11]),
    )
    .unwrap();
    let steps = project_for_version(&result.frames, 7).unwrap();
    let owner_steps = |step: &sonettobuf::FightStep| {
        step.act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
            .cloned()
            .collect::<Vec<_>>()
    };
    let [round_end] = steps.as_slice() else {
        panic!("expected the round-end root");
    };
    let owner_groups = owner_steps(round_end);
    let [full, available] = owner_groups.as_slice() else {
        panic!("expected an event group for each owner");
    };
    let bank = owner_steps(full);
    let [bank] = bank.as_slice() else {
        panic!("expected one bank event");
    };
    assert_eq!(
        (bank.act_id.unwrap_or_default(), bank.from_id),
        (0, Some(10))
    );
    let [credit] = bank.act_effect.as_slice() else {
        panic!("expected one committed bank credit");
    };
    assert_eq!(
        credit.effect_type,
        Some(EffectType::Expointoverflowbank as i32)
    );
    assert_eq!(credit.effect_num, Some(1));
    let stored = managers.buff.snapshot(10, 20).unwrap().act_common_params;
    assert_eq!(stored.as_deref(), Some("806#1"));
    let gain = owner_steps(available);
    let [gain] = gain.as_slice() else {
        panic!("expected one passive grant");
    };
    assert_eq!((gain.act_id, gain.from_id), (Some(31390151), Some(11)));
    assert_eq!(managers.ex_point.get(11), 5);
}
