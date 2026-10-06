use super::*;

#[test]
fn mass_action_behaviors_prioritize_the_selected_target() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(10_000),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10), entity(20), entity(30), entity(40)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                model_id: Some(109380001),
                ..entity(-1)
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: -1,
        skill_id: 109380001,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    invocation.target = SkillTarget::Explicit(40);
    let mut execution = SkillExecution::new(TargetContext::default());
    let mut grants = Vec::new();

    loop {
        let emission = emit_ops(
            invocation,
            &managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            &mut execution,
            &SkillOpTrigger::Active,
        )
        .unwrap();
        grants.extend(
            emission
                .ops
                .iter()
                .filter_map(|emission| match &emission.op {
                    RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant)))
                        if grant.buff_id == 109380001 =>
                    {
                        Some(grant.target_uid)
                    }
                    _ => None,
                }),
        );
        let Some(continuation) = emission.continuation else {
            break;
        };
        invocation = continuation;
    }

    assert_eq!(grants, vec![40, 20, 30, 10]);
}

#[test]
fn committed_conduit_hit_satisfies_attack_conditions() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(1001),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let skill_id = 31490121;
    assert!(managers.conduit.owns_skill(10, skill_id));

    let hit = managers
        .execute_hp(HpCommand::Damage(crate::engine::manager::hp::HpDamage {
            origin: FIELD_ORIGIN,
            source_uid: 10,
            target_uid: -1,
            amount: 100,
            config_effect: 0,
            effect_kind: crate::engine::manager::hp::DamageEffectKind::Normal,
            assassinate: false,
            ignore_riposte: false,
            hurt: crate::engine::manager::hp::HurtInfoData {
                from_uid: 10,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 0,
                skill_id,
                damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: 0,
                display_amount: None,
            },
        }))
        .unwrap()
        .events()
        .into_iter()
        .find(|event| matches!(event, BattleEvent::Hit(_)))
        .unwrap();

    let mut context = TargetContext::default();
    super::super::super::invoke::apply_event_context(&managers, &mut context, &hit);
    assert!(context.active_skill_is_attack);
    assert_eq!(context.active_skill_mode, SkillExecutionMode::Device);

    for (opcode, type_name) in [(501209, "UseHurtSkill"), (792209, "UseDeviceSkill")] {
        let condition = ParsedCondition {
            opcode,
            type_name: type_name.into(),
            kind: crate::engine::skill::condition::registry::parse(opcode, type_name, &[]).unwrap(),
            raw_args: Vec::new(),
        };
        assert!(crate::engine::skill::condition::evaluate::conditions_match(
            &[condition],
            -1,
            &[-1],
            Some(&managers),
            &pool,
            context,
        ));
    }
}
