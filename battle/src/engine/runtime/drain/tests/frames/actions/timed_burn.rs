use super::*;
use sonettobuf::effect_type_enum::EffectType;

#[test]
fn timed_carrier_grants_burn_before_a_mass_attack() {
    crate::test_support::init_config();
    let wingbeat = BuffInfo {
        uid: Some(63),
        buff_id: Some(31390122),
        from_uid: Some(20),
        duration: Some(3),
        ..Default::default()
    };
    let entity = |uid, team| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team),
        current_hp: Some(10_000),
        attr: Some(sonettobuf::HeroAttribute {
            hp: Some(10_000),
            attack: Some(500),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut holder = entity(10, 1);
    holder.model_id = Some(3105);
    holder.buffs = vec![wingbeat.clone()];
    let mut target = entity(-1, 2);
    target.buffs = vec![BuffInfo {
        uid: Some(64),
        buff_id: Some(4150001),
        from_uid: Some(30),
        layer: Some(2),
        count: Some(1),
        ..Default::default()
    }];
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![holder, entity(20, 1), entity(30, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![target, entity(-2, 2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let carrier_before = managers.buff.snapshot(10, 63).unwrap();
    let pool = TargetPool::from_fight(&fight);
    let catalog =
        SkillEffectCatalog::from_roots(config::configs::get(), [31050112], [31390122, 4150001]);
    let mut action: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 31050112,
    }
    .into();
    action.mode = SkillExecutionMode::Active;
    let result = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(action)],
    )
    .unwrap();

    let steps = crate::engine::packet::timeline::project_for_version(&result.frames, 7).unwrap();
    let attack = steps
        .iter()
        .find_map(|step| crate::test_support::find_step(step, 31050112))
        .unwrap();
    let child = attack
        .act_effect
        .iter()
        .enumerate()
        .filter_map(|(index, effect)| effect.fight_step.as_ref().map(|step| (index, step)))
        .filter(|(_, step)| step.act_id == wingbeat.buff_id)
        .collect::<Vec<_>>();
    let [(index, pass)] = child.as_slice() else {
        panic!("expected one Wingbeat activation for both targets");
    };
    assert_eq!((pass.from_id, pass.to_id), (Some(20), Some(10)));
    let markers = pass
        .act_effect
        .iter()
        .filter(|effect| effect.buff_act_id == Some(748))
        .collect::<Vec<_>>();
    assert_eq!(markers.len(), 2);
    assert!(markers.iter().all(|effect| {
        effect.effect_type == Some(EffectType::None as i32)
            && effect.target_id == Some(10)
            && effect.effect_num == wingbeat.buff_id
    }));
    let hit = attack
        .act_effect
        .iter()
        .position(|effect| {
            effect.effect_type == Some(EffectType::Damage as i32)
                || effect.effect_type == Some(EffectType::Crit as i32)
        })
        .unwrap();
    assert!(*index < hit);

    for (target_uid, source_uid, layers) in [(-1, 30, 3), (-2, 10, 1)] {
        let burn = managers
            .buff
            .active_for(target_uid)
            .find(|buff| buff.buff_id == Some(4150001))
            .unwrap();
        assert_eq!(
            (burn.from_uid, burn.layer),
            (Some(source_uid), Some(layers))
        );
    }
    assert_eq!(managers.buff.snapshot(10, 63), Some(carrier_before));
}
