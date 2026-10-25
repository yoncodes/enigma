use super::nested_casts::attack_with_passives;
use super::*;

#[test]
fn a_follow_up_cast_from_a_nested_skill_still_runs() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                buffs: vec![BuffInfo {
                    uid: Some(30),
                    buff_id: Some(312451011),
                    from_uid: Some(10),
                    duration: Some(3),
                    ..Default::default()
                }],
                ..entity(-1)
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [312451115], []);
    // A nested cast never completes an action, so nothing is held for it.
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 312451115,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    let result = run_skill(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        invocation,
        crate::engine::skill::action::SkillModifiers::default(),
    )
    .unwrap();

    let kicks = result
        .events
        .iter()
        .filter(|event| {
            matches!(
                event,
                crate::engine::event::payload::BattleEvent::SkillAction(action)
                    if action.skill_id == 312451011
                        && action.phase == crate::engine::skill::action::SkillPhase::Immediate
            )
        })
        .count();
    assert_eq!(kicks, 1);
}

#[test]
fn an_attacked_targets_attack_start_reaction_runs_before_the_attacks_own_effects() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                passive_skill: vec![109380003],
                ..entity(-1)
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31090111, 109380003], []);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 31090111,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    invocation.mode = SkillExecutionMode::Active;
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
    let position = |matches: fn(&crate::engine::event::payload::BattleEvent) -> bool| {
        result
            .events
            .iter()
            .position(matches)
            .expect("the event is published")
    };
    // "When being actively attacked, the attack counts as a Stronger Afflatus attack."
    let stronger_afflatus = position(|event| {
        matches!(
            event,
            crate::engine::event::payload::BattleEvent::BuffAdded(buff)
                if buff.target_uid == 10 && buff.buff_id == 109380006
        )
    });
    let attack_effects = position(|event| {
        matches!(
            event,
            crate::engine::event::payload::BattleEvent::SkillAction(action)
                if action.skill_id == 31090111
                    && action.phase == crate::engine::skill::action::SkillPhase::Immediate
        )
    });
    assert!(stronger_afflatus < attack_effects);
}

#[test]
fn a_pre_effect_power_reaction_runs_before_the_actions_immediate_damage() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(109380001),
                    from_uid: Some(-1),
                    ..Default::default()
                }],
                ..entity(10)
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                passive_skill: vec![109380003],
                power_infos: vec![PowerInfo {
                    power_id: Some(9),
                    num: Some(0),
                    max: Some(5),
                }],
                buffs: vec![BuffInfo {
                    uid: Some(21),
                    buff_id: Some(109320109),
                    from_uid: Some(-1),
                    ..Default::default()
                }],
                ..entity(-1)
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog =
        SkillEffectCatalog::from_roots(config::configs::get(), [109380001, 109380003], []);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: -1,
        skill_id: 109380001,
    }
    .into();
    invocation.card_index = 1;
    invocation.target = SkillTarget::Explicit(10);
    invocation.mode = SkillExecutionMode::Active;
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
    let action = crate::engine::packet::timeline::project(&result.frames)
        .unwrap()
        .into_iter()
        .find(|step| step.act_id == Some(109380001))
        .expect("the attack projects a step");
    let power = action
        .act_effect
        .iter()
        .position(|effect| {
            effect
                .fight_step
                .as_ref()
                .is_some_and(|step| step.act_id == Some(109380003))
        })
        .expect("the pre-effect power reaction projects a step");
    let origin_damage = action
        .act_effect
        .iter()
        .position(|effect| {
            effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Origindamage as i32)
        })
        .expect("the action deals configured origin damage");

    assert!(power < origin_damage);
}

#[test]
fn a_later_slot_sees_the_changes_of_an_earlier_slot_in_the_same_phase() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    buffs: vec![BuffInfo {
                        uid: Some(30),
                        buff_id: Some(109380001),
                        from_uid: Some(-1),
                        duration: Some(3),
                        ..Default::default()
                    }],
                    ..entity(10)
                },
                entity(11),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [109380001], []);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: -1,
        skill_id: 109380001,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    run_action(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [],
        invocation,
    )
    .unwrap();

    // "After hitting a target afflicted with [Vacant], ... removes [Vacant] from them;
    // otherwise, inflicts [Vacant] on the target."
    assert!(!managers.buff.has_active_buff_id(10, 109380001));
    assert!(managers.buff.has_active_buff_id(11, 109380001));
    assert!(!managers.buff.has_active_buff_id(10, 109380005));
}

#[test]
fn a_riposte_completes_as_an_ally_action() {
    let result = attack_with_passives(
        Vec::new(),
        vec![31050141],
        Vec::new(),
        Some(crate::engine::skill::condition::extra::ExtraSkillKind::Riposte),
    );

    // Flutterpage: "After any ally takes an action, gains 1 stack of [Gust]".
    assert!(result.events.iter().any(|event| matches!(
        event,
        crate::engine::event::payload::BattleEvent::BuffAdded(gust)
            if gust.target_uid == 11 && gust.buff_id == 31050111
    )));
}

#[test]
fn power_compare_observes_an_extra_action_before_its_force_field_cost() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                passive_skill: vec![31050143],
                power_infos: vec![PowerInfo {
                    power_id: Some(EUREKA_RESOURCE_ID),
                    num: Some(2),
                    max: Some(7),
                }],
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31050146),
                    from_uid: Some(10),
                    ..Default::default()
                }],
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
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31050151, 31050143], []);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 31050151,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    invocation.target = SkillTarget::Explicit(-1);

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
    let hit = result
        .events
        .iter()
        .position(|event| matches!(event, BattleEvent::Hit(hit) if hit.skill_id == 31050151))
        .expect("extra action hit");
    let recovered = result
        .events
        .iter()
        .position(|event| {
            matches!(
                event,
                BattleEvent::EurekaChanged(change)
                    if change.target_uid == 10 && change.applied_delta == 2
            )
        })
        .expect("post-hit Eureka recovery");

    assert!(hit < recovered);
    assert!(!result.events.iter().any(|event| matches!(
        event,
        BattleEvent::BuffAdded(buff) if buff.target_uid == 10 && buff.buff_id == 31050151
    )));
}

#[test]
fn flutterpage_gust_frame_follows_ally_action_target() {
    fn run(target_uid: i64) -> (i64, bool) {
        crate::test_support::init_config();
        let entity = |uid| FightEntityInfo {
            uid: Some(uid),
            current_hp: Some(100_000),
            attr: Some(HeroAttribute {
                hp: Some(100_000),
                attack: Some(1_000),
                ..Default::default()
            }),
            ..Default::default()
        };
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![
                    entity(10),
                    FightEntityInfo {
                        passive_skill: vec![31050141],
                        ..entity(11)
                    },
                ],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![entity(-1)],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);
        const ACTION_SKILL_ID: i32 = 999_999_008;
        let mut catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31050141], []);
        catalog.insert(ParsedSkillEffect {
            skill_id: ACTION_SKILL_ID,
            slots: Vec::new(),
        });
        let mut invocation: SkillInvocation = SkillRequest {
            source_uid: 10,
            skill_id: ACTION_SKILL_ID,
        }
        .into();
        invocation.target = SkillTarget::Explicit(target_uid);
        invocation.mode = SkillExecutionMode::Active;
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

        let action = crate::engine::packet::timeline::project(&result.frames)
            .unwrap()
            .into_iter()
            .find(|step| step.act_id == Some(ACTION_SKILL_ID))
            .expect("the action projects a step");
        let gust = action
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
            .find(|step| step.act_id == Some(31050141))
            .expect("Flutterpage reacts to the ally action");
        let gust_committed = result.events.iter().any(|event| {
            matches!(
                event,
                crate::engine::event::payload::BattleEvent::BuffAdded(buff)
                    if buff.target_uid == 11 && buff.buff_id == 31050111
            )
        });
        (
            gust.to_id.expect("the Gust frame has a target"),
            gust_committed,
        )
    }

    assert_eq!(run(11), (11, true));
    assert_eq!(run(-1), (-1, true));
}

#[test]
fn a_riposte_follows_the_attack_inside_its_own_buff_step() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                buffs: vec![BuffInfo {
                    uid: Some(30),
                    buff_id: Some(2292031),
                    from_uid: Some(10),
                    duration: Some(3),
                    ..Default::default()
                }],
                ..entity(10)
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            // "After any ally takes an action, gains 1 stack of [Gust]".
            entitys: vec![
                entity(-1),
                FightEntityInfo {
                    passive_skill: vec![31050141],
                    ..entity(-2)
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog =
        SkillEffectCatalog::from_roots(config::configs::get(), [31090111, 312301611, 31050141], []);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: -1,
        skill_id: 31090111,
    }
    .into();
    invocation.target = SkillTarget::Explicit(10);
    invocation.mode = SkillExecutionMode::Active;
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

    // [Fighting Experience]: "After an ally is attacked, Ezio ripostes with [Counter Kill]".
    let attack = crate::engine::packet::timeline::project(&result.frames)
        .unwrap()
        .into_iter()
        .find(|step| step.act_id == Some(31090111))
        .expect("the attack projects a step");
    let reactions = attack
        .act_effect
        .iter()
        .filter_map(|effect| effect.fight_step.as_ref())
        .collect::<Vec<_>>();
    let ally_action = reactions
        .iter()
        .position(|step| step.act_id == Some(31050141))
        .expect("the attacker's ally reacts to the action");
    let riposte = reactions.last().expect("the attack has reaction steps");
    assert!(ally_action < reactions.len() - 1);
    assert_eq!((riposte.act_id, riposte.from_id), (Some(2292031), Some(10)));
    assert!(
        riposte
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
            .any(|step| step.act_id == Some(312301611))
    );
}

#[test]
fn an_assassination_synchronization_gain_lands_in_the_attack_step() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                passive_skill: vec![312301403],
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
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog =
        SkillEffectCatalog::from_roots(config::configs::get(), [312301142, 312301403], []);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 312301142,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    invocation.mode = SkillExecutionMode::Active;
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

    // "When a basic incantation triggers [Assassination], Synchronization +5%."
    let attack = crate::engine::packet::timeline::project(&result.frames)
        .unwrap()
        .into_iter()
        .find(|step| step.act_id == Some(312301142))
        .expect("the attack projects a step");
    assert!(
        attack
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
            .all(|step| step.act_id != Some(312301403))
    );
    assert!(
        attack
            .act_effect
            .iter()
            .any(|effect| effect.effect_type == Some(111) && effect.target_id == Some(10))
    );
}

fn ezio_card_with_passives(passives: Vec<i32>) -> DrainResult {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                passive_skill: passives.clone(),
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
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_roots(
        config::configs::get(),
        std::iter::once(312301142).chain(passives),
        [],
    );
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 312301142,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    invocation.mode = SkillExecutionMode::Active;
    run_action(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [],
        invocation,
    )
    .unwrap()
}

#[test]
fn a_one_target_assassination_satisfies_skill_type_one() {
    let result = ezio_card_with_passives(vec![435115]);

    // "When triggering an [Assassination], if the attack aims at 1 target, gains Critical DMG +6%".
    assert!(result.events.iter().any(|event| matches!(
        event,
        crate::engine::event::payload::BattleEvent::BuffAdded(buff)
            if buff.target_uid == 10 && buff.buff_id == 435125
    )));
}

#[test]
fn allied_assassination_reactions_follow_the_attacks_after_hit_reactions() {
    let result = ezio_card_with_passives(vec![312301533, 435115]);

    let attack = crate::engine::packet::timeline::project(&result.frames)
        .unwrap()
        .into_iter()
        .find(|step| step.act_id == Some(312301142))
        .expect("the attack projects a step");
    let order = attack
        .act_effect
        .iter()
        .filter_map(|effect| effect.fight_step.as_ref())
        .filter_map(|step| step.act_id)
        .filter(|act_id| [312301533, 435115].contains(act_id))
        .collect::<Vec<_>>();
    // "When any ally triggers an [Assassination]" answers the completed action.
    assert_eq!(order, vec![435115, 312301533]);
}

#[test]
fn skill_effect_tag_reactions_follow_source_after_hit_effects() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                passive_skill: vec![116330011],
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
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31340121, 116330011], []);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 31340121,
    }
    .into();
    invocation.target = SkillTarget::Explicit(10);
    invocation.mode = SkillExecutionMode::Active;

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
    let action = crate::engine::packet::timeline::project(&result.frames)
        .unwrap()
        .into_iter()
        .find(|step| step.act_id == Some(31340121))
        .expect("the source skill projects a step");
    let source_effect = action
        .act_effect
        .iter()
        .position(|effect| effect.buff.as_ref().and_then(|buff| buff.buff_id) == Some(31340002))
        .expect("the source skill applies its own buff");
    let reaction = action
        .act_effect
        .iter()
        .position(|effect| {
            effect.fight_step.as_ref().and_then(|step| step.act_id) == Some(116330011)
        })
        .expect("the effect-tag reaction projects a nested step");

    assert!(source_effect < reaction);
}

#[test]
fn a_skill_cast_by_another_skill_keeps_its_step_without_effects() {
    // "When any ally triggers an [Assassination], if the carrier owns [Force Field], ...":
    // without a Force Field the cast skill changes nothing but still shows its step.
    let result = ezio_card_with_passives(vec![435211]);

    let attack = crate::engine::packet::timeline::project(&result.frames)
        .unwrap()
        .into_iter()
        .find(|step| step.act_id == Some(312301142))
        .expect("the attack projects a step");
    let reaction = attack
        .act_effect
        .iter()
        .filter_map(|effect| effect.fight_step.as_ref())
        .find(|step| step.act_id == Some(435211))
        .expect("the psychube reacts");
    assert_eq!((reaction.from_id, reaction.to_id), (Some(10), Some(-1)));
    let cast = reaction
        .act_effect
        .iter()
        .filter_map(|effect| effect.fight_step.as_ref())
        .find(|step| step.act_id == Some(435221))
        .expect("the cast skill keeps its step");
    assert_eq!((cast.from_id, cast.to_id), (Some(10), Some(10)));
    assert!(cast.act_effect.is_empty());
}

#[test]
fn hit_damage_reactions_follow_semantic_settlement_phases() {
    crate::test_support::init_config();
    let entity = |uid, attack, hp| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(hp),
        attr: Some(HeroAttribute {
            attack: Some(attack),
            hp: Some(100_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let dot = |uid, buff_id| BuffInfo {
        uid: Some(uid),
        buff_id: Some(buff_id),
        from_uid: Some(11),
        duration: Some(3),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    buffs: vec![BuffInfo {
                        uid: Some(36),
                        buff_id: Some(109380007),
                        from_uid: Some(-1),
                        layer: Some(5),
                        ..Default::default()
                    }],
                    ..entity(10, 1_000, 100_000)
                },
                entity(11, 1_000, 100_000),
                entity(12, 1_000, 100_000),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                passive_skill: vec![109320108, 109380003, 109380004, 434111],
                buffs: vec![
                    dot(30, 312451021),
                    dot(31, 312451011),
                    dot(32, 312451011),
                    dot(33, 312451031),
                    dot(34, 312451011),
                    BuffInfo {
                        uid: Some(35),
                        buff_id: Some(31090112),
                        from_uid: Some(12),
                        layer: Some(15),
                        ..Default::default()
                    },
                ],
                ..entity(-1, 1_000, 97_900)
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_roots(
        config::configs::get(),
        [312301142, 109320108, 109380003, 109380004, 434111],
        [],
    );
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 312301142,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    invocation.mode = SkillExecutionMode::Active;

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

    assert_eq!(
        result
            .events
            .iter()
            .filter(|event| matches!(
                event,
                crate::engine::event::payload::BattleEvent::BuffAdded(buff)
                    if buff.buff_id == 109320114
            ))
            .count(),
        1
    );

    fn collect(step: &sonettobuf::FightStep, found: &mut Vec<i32>) {
        if let Some(
            act_id @ (312451021 | 312451011 | 312451031 | 109320108 | 109380003 | 109380004
            | 434111),
        ) = step.act_id
        {
            found.push(act_id);
        }
        for effect in &step.act_effect {
            if effect.buff.as_ref().and_then(|buff| buff.buff_id) == Some(31090112) {
                found.push(31090112);
            }
            if let Some(child) = &effect.fight_step {
                collect(child, found);
            }
        }
    }
    let mut found = Vec::new();
    for step in crate::engine::packet::timeline::project(&result.frames).unwrap() {
        collect(&step, &mut found);
    }

    assert_eq!(
        found,
        vec![
            109380003, 312451021, 312451011, 312451011, 312451031, 312451011, 109380003, 109380004,
            434111, 109320108, 31090112,
        ]
    );
}
