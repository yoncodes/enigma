use super::*;

#[test]
fn mass_action_per_target_behaviors_follow_entity_map_order() {
    crate::test_support::init_config();
    let entity = |uid, buffs| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(10_000),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        buffs,
        ..Default::default()
    };
    let buff = |uid, buff_id| BuffInfo {
        uid: Some(uid),
        buff_id: Some(buff_id),
        from_uid: Some(-1),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(234410900, Vec::new()),
                entity(237352626, vec![buff(2, 109380005)]),
                entity(240494381, vec![buff(3, 109380001)]),
                entity(240494371, vec![buff(4, 109380001)]),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                model_id: Some(109380001),
                ..entity(-1, Vec::new())
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
    invocation.target = SkillTarget::Explicit(234410900);
    let ops = emit_all_ops(
        invocation,
        &managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &SkillOpTrigger::Active,
    )
    .unwrap();

    let origin_damage = ops
        .iter()
        .filter_map(|op| match op {
            RuleOp::Command(BattleCommand::Hp(HpCommand::Damage(damage)))
                if damage.hurt.damage_from
                    == crate::engine::manager::hp::HurtDamageFromType::SkillEffect =>
            {
                Some(damage.target_uid)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let grants = ops
        .iter()
        .filter_map(|op| match op {
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant)))
                if grant.buff_id == 109380001 =>
            {
                Some(grant.target_uid)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let vacant_expiries = ops
        .iter()
        .filter_map(|op| match op {
            RuleOp::Command(BattleCommand::Buff(BuffCommand::ForceExpire(remove)))
                if remove.selector == BuffRemoveSelector::IdOrType(109380001) =>
            {
                Some(remove.target_uid)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(origin_damage, [240494371, 240494381]);
    assert_eq!(vacant_expiries, [237352626]);
    assert_eq!(grants, [240494371, 240494381, 234410900]);
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
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    super::super::super::invoke::apply_event_context(&managers, &catalog, &mut context, &hit);
    assert!(context.active_skill_is_attack);
    assert_eq!(context.active_skill_mode, SkillExecutionMode::Device);
    assert_eq!(context.damage_target_count_kind, 2);

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

#[test]
fn broken_guard_reaction_requires_surviving_the_triggering_hit() {
    use crate::engine::manager::hp::{HpDamage, HurtDamageFromType, HurtInfoData};

    crate::test_support::init_config();
    let skill_id = 109350304;
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                toughness_value: Some(100),
                toughness_point: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    for (amount, expected_hp) in [(950, 50), (1_000, 0)] {
        let mut managers = BattleManagers::seeded(&fight);
        let outcome = managers
            .execute_hp(HpCommand::Damage(HpDamage {
                origin: CommandOrigin {
                    domain: RuleDomain::Skill,
                    key: DefinitionKey::new(1, "SkillDamage"),
                },
                source_uid: 10,
                target_uid: -1,
                amount,
                config_effect: -1,
                effect_kind: DamageEffectKind::Normal,
                assassinate: false,
                ignore_riposte: false,
                hurt: HurtInfoData {
                    from_uid: 10,
                    is_crit: false,
                    career_restraint: true,
                    reduce_hp: -amount,
                    effect_id: 1,
                    skill_id: 1,
                    damage_from: HurtDamageFromType::Skill,
                    buff_act_id: 0,
                    buff_uid: 0,
                    hurt_effect_type: 0,
                    display_amount: None,
                },
            }))
            .unwrap();
        assert_eq!(managers.hp.current(-1), expected_hp);
        let event = outcome
            .events()
            .into_iter()
            .find(|event| matches!(event, BattleEvent::ToughnessBroken { .. }))
            .expect("a lethal hit can still break guard");
        assert!(managers.toughness.is_broken(-1));

        let pool = TargetPool::from_fight(&fight).runtime_view(&managers);
        let mut invocation: SkillInvocation = SkillRequest {
            source_uid: -1,
            skill_id,
        }
        .into();
        invocation.condition_key = Some(DefinitionKey::new(2092, "None"));
        invocation.target = SkillTarget::Explicit(-1);
        let ops = emit_all_ops(
            invocation,
            &managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            &SkillOpTrigger::Event(event),
        )
        .unwrap();
        if expected_hp == 0 {
            assert!(ops.is_empty());
        } else {
            let [
                RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(loss))),
                RuleOp::Command(BattleCommand::ToughnessRecover(recover)),
            ] = ops.as_slice()
            else {
                panic!("expected guard-break HP loss followed by recovery");
            };
            assert!(
                managers
                    .execute_hp(HpCommand::Lose(*loss))
                    .unwrap()
                    .caused_death()
            );
            assert!(managers.toughness.recover(*recover).is_some());
        }
    }
}

#[test]
fn prepared_status_threshold_includes_nonattacks_and_excludes_energy_cards() {
    crate::test_support::init_config();
    let mut status_buffs = [31450171, 31450143, 31440116, 31450111]
        .into_iter()
        .enumerate()
        .map(|(index, buff_id)| BuffInfo {
            uid: Some(index as i64 + 1),
            buff_id: Some(buff_id),
            from_uid: Some(10),
            ..Default::default()
        })
        .collect::<Vec<_>>();
    status_buffs.push(BuffInfo {
        uid: Some(5),
        buff_id: Some(103374114),
        from_uid: Some(-2),
        layer: Some(2),
        ..Default::default()
    });
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                buffs: status_buffs,
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [103374322], []);

    let emit = |is_attack, mode| {
        let mut invocation: SkillInvocation = SkillRequest {
            source_uid: 10,
            skill_id: 103374322,
        }
        .into();
        invocation.condition_key = Some(DefinitionKey::new(42203, "HasTypeBuffMoreThan"));
        invocation.phase = Some(SkillPhase::Immediate);
        let event = BattleEvent::SkillEffectsPrepared(Box::new(
            crate::engine::skill::action::SkillActionEvent {
                source_uid: 10,
                card_index: 0,
                skill_id: 200,
                target_uid: 10,
                target_uids: vec![10],
                attacked_target_uids: vec![10],
                phase: SkillPhase::Immediate,
                skill_slot: 1,
                is_attack,
                rank: 1,
                skill_type: 1,
                effect_tag: 1,
                assassinate: false,
                ignore_riposte: false,
                damage_amount: 0,
                kill_count: 0,
                crit_count: 0,
                guard_break_count: 0,
                additional_moxie: 0,
                extra_skill_kind: 0,
                mode,
                teammate_injury_count: 0,
                teammate_injury_count_not_reset: 0,
                team_injury_count_round: 0,
                card_enchants: Vec::new(),
                buff_additions: Vec::new(),
            },
        ));
        emit_all_ops(
            invocation,
            &managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext {
                active_skill_id: 200,
                active_skill_is_attack: is_attack,
                active_skill_mode: mode,
                ..Default::default()
            },
            &SkillOpTrigger::Event(event),
        )
        .unwrap()
    };

    for (is_attack, mode, expected) in [
        (true, SkillExecutionMode::Active, true),
        (false, SkillExecutionMode::Active, true),
        (false, SkillExecutionMode::Device, true),
        (false, SkillExecutionMode::DeviceCard, false),
    ] {
        let ops = emit(is_attack, mode);
        assert_eq!(
            ops.iter().any(|op| matches!(
                op,
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant)))
                    if grant.buff_id == 103374322 && grant.target_uid == 10
            )),
            expected,
            "{mode:?}"
        );
    }
}
