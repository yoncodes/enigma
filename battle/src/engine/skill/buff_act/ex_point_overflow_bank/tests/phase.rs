use crate::engine::runtime::drain::BattleAccess;
use sonettobuf::effect_type_enum::EffectType;

use super::*;
use crate::engine::{
    packet::timeline::project_for_version,
    runtime::{
        determinism::RoundDeterminism,
        drain::{ReactionLane, run, run_group_event_lanes},
    },
    skill::{
        action::{SkillExecutionMode, SkillInvocation, SkillRequest},
        effect::SkillEffectCatalog,
        rule::output::RuleOp,
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
        BattleAccess::new(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
        ),
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

#[test]
fn hit_passive_finishes_before_banking_moxie() {
    crate::test_support::init_config();
    let buff = |uid, id, layer| BuffInfo {
        uid: Some(uid),
        buff_id: Some(id),
        from_uid: Some(10),
        layer,
        ..Default::default()
    };
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(10_000),
        attr: Some(sonettobuf::HeroAttribute {
            hp: Some(10_000),
            attack: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                model_id: Some(3127),
                ex_point: Some(5),
                passive_skill: vec![31270145],
                buffs: vec![buff(20, 31270400, None), buff(21, 31270409, Some(2))],
                ..entity(10)
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers.attribute.override_ex(
        10,
        &sonettobuf::HeroExAttribute {
            cri: Some(10_000),
            ..Default::default()
        },
    );
    let catalog = SkillEffectCatalog::from_roots(
        config::configs::get(),
        [31270111, 31270145],
        [31270400, 31270409],
    );
    let mut attack: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 31270111,
    }
    .into();
    attack.mode = SkillExecutionMode::Active;
    let result = run(
        &mut managers,
        &TargetPool::from_fight(&fight),
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 3,
            ..Default::default()
        },
        [RuleOp::Skill(attack)],
    )
    .unwrap();
    let steps = project_for_version(&result.frames, 7).unwrap();
    let effects = &steps
        .iter()
        .find(|step| step.act_id == Some(31270111))
        .unwrap()
        .act_effect;
    let passive_index = effects
        .iter()
        .position(|effect| {
            effect
                .fight_step
                .as_ref()
                .is_some_and(|step| step.act_id == Some(31270145))
        })
        .unwrap();
    let passive = effects[passive_index].fight_step.as_ref().unwrap();
    let [consume] = passive.act_effect.as_slice() else {
        panic!("expected the passive to spend its charge");
    };
    assert_eq!(consume.effect_type, Some(EffectType::Buffupdate as i32));
    let bank = effects[passive_index + 1].fight_step.as_ref().unwrap();
    let [credit] = bank.act_effect.as_slice() else {
        panic!("expected one bank credit after the passive");
    };
    assert_eq!(
        credit.effect_type,
        Some(EffectType::Expointoverflowbank as i32)
    );
    assert_eq!(managers.buff.buff_id_amount(10, 31270409), 1);
    assert_eq!(
        managers
            .buff
            .snapshot(10, 20)
            .unwrap()
            .act_common_params
            .as_deref(),
        Some("806#1")
    );
}
