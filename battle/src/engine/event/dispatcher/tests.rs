use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::{
    event::{
        bus::EventBus,
        payload::{EntityDiedEvent, HitEvent},
    },
    manager::buff::{BuffCommand, CommandOrigin},
    runtime::executor::execute_rule_op,
    skill::{
        condition::{ParsedCondition, ParsedConditionKind, parse::BuffAddedScope},
        effect::{ParsedBehavior, ParsedSkillEffect, SkillEffectSlot},
        rule::{DefinitionKey, RuleDomain, output::BattleCommand},
        target::TargetRequest,
    },
};

#[test]
fn reaction_timing_selects_subscribers_before_rule_outputs() {
    let skill = |timing| SkillSubscriber {
        owner_uid: 10,
        skill_id: 100,
        slot_index: Some(0),
        key: crate::engine::event::subscription::SubscriptionKey::new(
            EventKind::SkillAction,
            DefinitionKey::new(1, "StageTest"),
        )
        .with_timing(timing),
    };
    let mut subscribers = subscriber::EventSubscribers {
        skills: vec![
            skill(crate::engine::event::subscription::ReactionTiming::Immediate),
            skill(crate::engine::event::subscription::ReactionTiming::AfterSkill),
            skill(crate::engine::event::subscription::ReactionTiming::AfterHitSkills),
        ],
        buff_acts: Vec::new(),
    };

    retain_reaction_timing(
        &mut subscribers,
        crate::engine::event::subscription::ReactionTiming::AfterSkill,
        &BattleEvent::Kind(EventKind::SkillAction),
        false,
    );

    assert_eq!(subscribers.skills.len(), 1);
    assert_eq!(
        subscribers.skills[0].key.timing,
        crate::engine::event::subscription::ReactionTiming::AfterSkill
    );
}

#[test]
fn death_reactions_run_subscriber_frames_before_side_frames() {
    let skill = |skill_id, definition| SkillSubscriber {
        owner_uid: -1,
        skill_id,
        slot_index: Some(0),
        key: crate::engine::event::subscription::SubscriptionKey::new(
            EventKind::EntityDied,
            definition,
        ),
    };
    let passes = ordered_subscriber_passes(
        subscriber::EventSubscribers {
            skills: vec![
                skill(1, DefinitionKey::new(8, "Dead")),
                skill(2, DefinitionKey::new(648003, "SelfExit")),
                skill(3, DefinitionKey::new(812, "Dead")),
            ],
            buff_acts: Vec::new(),
        },
        &BattleEvent::EntityDied(EntityDiedEvent {
            source_uid: 10,
            target_uid: -1,
        }),
    );

    assert_eq!(
        passes
            .iter()
            .map(|pass| {
                pass.skills
                    .iter()
                    .map(|subscriber| subscriber.skill_id)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>(),
        vec![vec![2], vec![3], vec![1]]
    );
}

#[test]
fn shared_hit_delay_is_owned_by_the_subscription_timing() {
    let subscriber = || SkillSubscriber {
        owner_uid: 10,
        skill_id: 100,
        slot_index: Some(0),
        key: crate::engine::event::subscription::SubscriptionKey::new(
            EventKind::TargetAttacked,
            DefinitionKey::new(22209, "BeAttacked"),
        )
        .with_timing(crate::engine::event::subscription::ReactionTiming::AfterSharedHit),
    };
    let hit = |share_count| {
        BattleEvent::Hit(HitEvent {
            origin: CommandOrigin {
                domain: RuleDomain::Skill,
                key: DefinitionKey::new(1, "SkillDamage"),
            },
            source_uid: -1,
            target_uid: 10,
            skill_id: 1,
            amount: 100,
            resolved_damage: 100,
            shield_absorbed: 0,
            career_restraint: false,
            damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
            share_count,
            assassinate: false,
            ignore_riposte: false,
        })
    };

    let mut normal = subscriber::EventSubscribers {
        skills: vec![subscriber()],
        buff_acts: Vec::new(),
    };
    retain_reaction_timing(
        &mut normal,
        crate::engine::event::subscription::ReactionTiming::Immediate,
        &hit(0),
        true,
    );
    assert_eq!(normal.skills.len(), 1);

    let mut shared = subscriber::EventSubscribers {
        skills: vec![subscriber()],
        buff_acts: Vec::new(),
    };
    retain_reaction_timing(
        &mut shared,
        crate::engine::event::subscription::ReactionTiming::AfterSharedHit,
        &hit(3),
        true,
    );
    assert_eq!(shared.skills.len(), 1);
}

#[test]
fn buff_act_subscription_selects_declared_reaction_timing_before_rule_outputs() {
    let subscriber = |opcode, type_name: &'static str| BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: i64::from(opcode),
        buff_id: opcode,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: crate::engine::event::subscription::SubscriptionKey::new(
            EventKind::SkillAction,
            DefinitionKey::new(opcode, type_name),
        )
        .with_timing(crate::engine::skill::buff_act::registry::reaction_timing(
            opcode, type_name,
        )),
        act_type: type_name.to_owned(),
        effect_time: 0,
        effect_condition: 0,
        args: Vec::new(),
        raw: String::new(),
    };
    let buff_acts = vec![
        subscriber(800, "TeammateInjuryCount"),
        subscriber(1023, "LostHpAddExtraBloodPoolValue"),
        subscriber(302, "BeatBack"),
        subscriber(870, "Shell"),
        subscriber(871, "ShellDebuff"),
    ];
    for (timing, expected) in [
        (
            crate::engine::event::subscription::ReactionTiming::Immediate,
            DefinitionKey::new(800, "TeammateInjuryCount"),
        ),
        (
            crate::engine::event::subscription::ReactionTiming::AfterSettlement,
            DefinitionKey::new(1023, "LostHpAddExtraBloodPoolValue"),
        ),
        (
            crate::engine::event::subscription::ReactionTiming::AfterAction,
            DefinitionKey::new(302, "BeatBack"),
        ),
        (
            crate::engine::event::subscription::ReactionTiming::AfterHitShell,
            DefinitionKey::new(870, "Shell"),
        ),
        (
            crate::engine::event::subscription::ReactionTiming::BeforeAfterHitObservers,
            DefinitionKey::new(871, "ShellDebuff"),
        ),
    ] {
        let mut subscribers = subscriber::EventSubscribers {
            skills: Vec::new(),
            buff_acts: buff_acts.clone(),
        };

        retain_reaction_timing(
            &mut subscribers,
            timing,
            &BattleEvent::Kind(EventKind::SkillAction),
            false,
        );

        assert_eq!(
            subscribers
                .buff_acts
                .iter()
                .map(|subscriber| subscriber.key.definition)
                .collect::<Vec<_>>(),
            vec![expected]
        );
    }
}

#[test]
fn completed_action_routes_same_and_opposing_team_thresholds_separately() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                passive_skill: vec![30660191],
                ..Default::default()
            }],
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
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.eureka.add_max(10, 1, 5);
    managers.eureka.set(10, 1, 5);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let dispatch_action = |source_uid| {
        dispatch_event(
            &pool,
            &managers,
            &catalog,
            &mut crate::engine::runtime::determinism::RoundDeterminism::default(),
            &BattleEvent::AllyAction(crate::engine::skill::action::ActionEvent {
                source_uid,
                ..Default::default()
            }),
        )
        .unwrap()
    };

    let allied = dispatch_action(10);
    assert!(matches!(
        allied.skills.as_slice(),
        [(subscriber, _)]
            if subscriber.key.definition
                == DefinitionKey::new(180212999, "PowerCompare")
    ));

    let opposing = dispatch_action(-1);
    assert!(matches!(
        opposing.skills.as_slice(),
        [(subscriber, _)]
            if subscriber.key.definition
                == DefinitionKey::new(180213999, "PowerCompare")
    ));
}

#[test]
fn derived_skill_cast_does_not_require_a_skill_action_phase() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            assist_boss: Some(FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(999_999),
                passive_skill: vec![12_720_012],
                ..Default::default()
            }),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-2),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let event = BattleEvent::SkillAction(crate::engine::skill::action::SkillActionEvent {
        source_uid: -1,
        skill_id: 370_001_002,
        target_uid: -2,
        target_uids: vec![-2],
        attacked_target_uids: vec![-2],
        phase: crate::engine::skill::action::SkillPhase::HitPassives,
        skill_slot: -1,
        is_attack: true,
        rank: 1,
        skill_type: 0,
        effect_tag: 2,
        assassinate: false,
        ignore_riposte: false,
        damage_amount: 1,
        kill_count: 0,
        crit_count: 0,
        guard_break_count: 0,
        additional_moxie: 0,
        extra_skill_kind: 0,
        mode: crate::engine::skill::action::SkillExecutionMode::Active,
        teammate_injury_count: 0,
        teammate_injury_count_not_reset: 0,
        team_injury_count_round: 0,
        card_enchants: Vec::new(),
        buff_additions: Vec::new(),
    });

    let dispatched = dispatch_event(
        &pool,
        &managers,
        &catalog,
        &mut crate::engine::runtime::determinism::RoundDeterminism::default(),
        &event,
    )
    .unwrap();

    assert!(dispatched.skills.iter().any(|(subscriber, _)| {
        subscriber.owner_uid == -1 && subscriber.skill_id == 12_720_012
    }));
}

#[test]
fn ally_attacked_observer_uses_primary_hit_targets_once_per_action() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10), entity(11), entity(12)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let subscriber = |owner_uid| SkillSubscriber {
        owner_uid,
        skill_id: 1,
        slot_index: Some(0),
        key: crate::engine::event::subscription::SubscriptionKey::at_phase(
            EventKind::SkillAction,
            DefinitionKey::new(22213, "BeAttacked"),
            Some(crate::engine::skill::action::SkillPhase::HitPassives),
        ),
    };
    let mut action = crate::engine::skill::action::SkillActionEvent {
        source_uid: -1,
        skill_id: 2,
        target_uid: 10,
        target_uids: vec![10, 11],
        attacked_target_uids: vec![10, 11],
        phase: crate::engine::skill::action::SkillPhase::HitPassives,
        skill_slot: 1,
        is_attack: true,
        rank: 1,
        skill_type: 1,
        effect_tag: 1,
        assassinate: false,
        ignore_riposte: false,
        damage_amount: 1,
        kill_count: 0,
        crit_count: 0,
        guard_break_count: 0,
        additional_moxie: 0,
        extra_skill_kind: 0,
        mode: crate::engine::skill::action::SkillExecutionMode::Active,
        teammate_injury_count: 0,
        teammate_injury_count_not_reset: 0,
        team_injury_count_round: 0,
        card_enchants: Vec::new(),
        buff_additions: Vec::new(),
    };

    assert!(skill_subscriber_observes_action(
        &pool,
        &subscriber(12),
        &action
    ));
    assert!(skill_subscriber_observes_action(
        &pool,
        &subscriber(10),
        &action
    ));
    assert!(!skill_subscriber_observes_action(
        &pool,
        &subscriber(-1),
        &action
    ));

    action.attacked_target_uids = vec![10];
    assert!(skill_subscriber_observes_action(
        &pool,
        &subscriber(10),
        &action
    ));
}

#[test]
fn team_skill_observer_sees_allied_nested_actions() {
    crate::test_support::init_config();
    let entity = |uid, team_type| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(100),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1), entity(11, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2)],
            ..Default::default()
        }),
        ..Default::default()
    });
    let subscriber = SkillSubscriber {
        owner_uid: 10,
        skill_id: 1,
        slot_index: Some(0),
        key: crate::engine::event::subscription::SubscriptionKey::at_phase(
            EventKind::SkillAction,
            DefinitionKey::new(1001212, "Assassinate"),
            Some(crate::engine::skill::action::SkillPhase::AfterHit),
        ),
    };
    let action = |source_uid| crate::engine::skill::action::SkillActionEvent {
        source_uid,
        skill_id: 2,
        target_uid: -1,
        target_uids: vec![-1],
        attacked_target_uids: vec![-1],
        phase: crate::engine::skill::action::SkillPhase::AfterHit,
        skill_slot: 1,
        is_attack: true,
        rank: 1,
        skill_type: 1,
        effect_tag: 1,
        assassinate: true,
        ignore_riposte: false,
        damage_amount: 1,
        kill_count: 0,
        crit_count: 0,
        guard_break_count: 0,
        additional_moxie: 0,
        extra_skill_kind: crate::engine::skill::condition::extra::ExtraSkillKind::Riposte.id(),
        mode: crate::engine::skill::action::SkillExecutionMode::Nested,
        teammate_injury_count: 0,
        teammate_injury_count_not_reset: 0,
        team_injury_count_round: 0,
        card_enchants: Vec::new(),
        buff_additions: Vec::new(),
    };

    assert!(skill_subscriber_observes_action(
        &pool,
        &subscriber,
        &action(11)
    ));
    assert!(!skill_subscriber_observes_action(
        &pool,
        &subscriber,
        &action(-1)
    ));
}

#[test]
fn team_actor_buff_act_observes_an_allied_skill_action() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(433011),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let event = BattleEvent::SkillAction(crate::engine::skill::action::SkillActionEvent {
        source_uid: 11,
        skill_id: 100,
        target_uid: -1,
        target_uids: vec![-1],
        attacked_target_uids: vec![-1],
        phase: crate::engine::skill::action::SkillPhase::AfterHit,
        skill_slot: 1,
        is_attack: true,
        rank: 1,
        skill_type: 1,
        effect_tag: 1,
        assassinate: false,
        ignore_riposte: false,
        damage_amount: 1,
        kill_count: 0,
        crit_count: 0,
        guard_break_count: 0,
        additional_moxie: 0,
        extra_skill_kind: 0,
        mode: crate::engine::skill::action::SkillExecutionMode::Active,
        teammate_injury_count: 1,
        teammate_injury_count_not_reset: 1,
        team_injury_count_round: 1,
        card_enchants: Vec::new(),
        buff_additions: Vec::new(),
    });

    let dispatched = dispatch_event(
        &TargetPool::from_fight(&fight),
        &managers,
        &catalog,
        &mut crate::engine::runtime::determinism::RoundDeterminism::default(),
        &event,
    )
    .unwrap();

    assert!(dispatched.buff_acts.iter().any(|(subscriber, ops)| {
        subscriber.key.definition == DefinitionKey::new(800, "TeammateInjuryCount")
            && matches!(
                ops,
                Ok(Some(ops)) if matches!(ops.as_slice(), [buff_act::BuffActRuleOp {
                    op: RuleOp::Command(BattleCommand::Buff(BuffCommand::AccumulateActValue(_))),
                    ..
                }])
            )
    }));
}

#[test]
fn committed_buff_addition_selects_its_registered_subscriber() {
    crate::test_support::init_config();
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::new(20002, "AddExPoint", vec![1]),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 583004,
        type_name: "AccTeamAddBuffCountByBuffId".to_owned(),
        kind: ParsedConditionKind::AccBuffAddedCount {
            buff_ids: vec![101],
            threshold: 1,
            scope: BuffAddedScope::Team,
        },
        raw_args: vec!["101".to_owned(), "1".to_owned()],
    }];
    slot.compiled_route =
        crate::engine::skill::rule::route::ConditionRoute::compile(&slot.conditions);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![slot],
    });
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                passive_skill: vec![100],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut events = EventBus::default();
    execute_rule_op(
        &mut managers,
        &mut events,
        crate::engine::skill::rule::output::RuleOp::Command(BattleCommand::Buff(
            BuffCommand::Grant(crate::engine::manager::buff::BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 10,
                target_uid: 10,
                buff_id: 101,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )),
    )
    .unwrap();
    let event = events.pop().expect("committed buff event");

    let dispatched = dispatch_event(
        &pool,
        &managers,
        &catalog,
        &mut crate::engine::runtime::determinism::RoundDeterminism::default(),
        &event,
    )
    .unwrap();

    assert_eq!(event.kind(), EventKind::BuffAdded);
    assert!(matches!(
        dispatched.skills.as_slice(),
        [(subscriber, RuleOp::Skill(invocation))] if subscriber.skill_id == 100
            && subscriber.key.event == EventKind::BuffAdded
            && invocation.condition_key
                == Some(DefinitionKey::new(583004, "AccTeamAddBuffCountByBuffId"))
    ));
}

#[test]
fn exact_linked_buff_act_emits_a_skill_output() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(30880131),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());

    let dispatched = dispatch(
        &TargetPool::from_fight(&fight),
        &managers,
        &catalog,
        EventKind::RoundEnd,
    );

    assert!(matches!(
        dispatched.buff_acts.as_slice(),
        [(
            subscriber,
            Ok(Some(ops))
        )] if subscriber.key.definition == DefinitionKey::new(759, "UseSkillToEnemy")
            && matches!(ops.as_slice(), [buff_act::BuffActRuleOp {
                op: RuleOp::Skill(
                crate::engine::skill::action::SkillInvocation {
                    plan: crate::engine::skill::action::SkillRequest {
                        source_uid: 10,
                        skill_id: 30880171,
                    },
                    ..
                }
            ),
                ..
            }])
    ));
}

#[test]
fn setup_stage_is_not_encoded_as_a_runtime_event() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                passive_skill: vec![31340141],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());

    let dispatched = dispatch_setup(&pool, &managers, &catalog, SetupStage::RoundStart, 1);

    assert!(matches!(
        dispatched.as_slice(),
        [(
            SetupSubscriber {
                owner_uid: 10,
                skill_id: 31340141,
                slot_index: 1,
                stage: SetupStage::RoundStart,
                priority: 1,
                key: setup_key,
            },
            RuleOp::Skill(SkillInvocation {
                condition_key: Some(condition_key),
                ..
            }),
        )] if *setup_key == DefinitionKey::new(103, "None")
            && *condition_key == DefinitionKey::new(103, "None")
    ));
}

#[test]
fn compiled_setup_keeps_exact_routes_independent() {
    let subscribers = vec![
        SetupSubscriber {
            owner_uid: 10,
            skill_id: 100,
            slot_index: 0,
            stage: SetupStage::EnterFight,
            priority: 0,
            key: DefinitionKey::new(5, "EnterFight"),
        },
        SetupSubscriber {
            owner_uid: 10,
            skill_id: 100,
            slot_index: 1,
            stage: SetupStage::EnterFight,
            priority: 0,
            key: DefinitionKey::new(573002, "PerTeamOtherEntityDmgType"),
        },
    ];

    let outputs = compiled_setup_outputs(subscribers);

    assert!(matches!(
        outputs.as_slice(),
        [
            (
                SetupSubscriber { key: first_key, .. },
                RuleOp::Skill(SkillInvocation {
                    condition_key: Some(first_condition),
                    ..
                })
            ),
            (
                SetupSubscriber { key: second_key, .. },
                RuleOp::Skill(SkillInvocation {
                    condition_key: Some(second_condition),
                    ..
                })
            )
        ] if *first_key == DefinitionKey::new(5, "EnterFight")
            && *first_condition == *first_key
            && *second_key == DefinitionKey::new(573002, "PerTeamOtherEntityDmgType")
            && *second_condition == *second_key
    ));
}

#[test]
fn committed_hit_discovers_target_attacked_skill_and_be_attacked_buff_act() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                passive_skill: vec![530000151],
                buffs: vec![
                    BuffInfo {
                        uid: Some(20),
                        buff_id: Some(530000111),
                        layer: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(21),
                        buff_id: Some(30620111),
                        layer: Some(1),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let event = BattleEvent::Hit(HitEvent {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(1, "Damage"),
        },
        source_uid: 10,
        target_uid: -1,
        skill_id: 100,
        amount: 50,
        resolved_damage: 50,
        shield_absorbed: 0,
        career_restraint: false,
        damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
        share_count: 0,
        assassinate: false,
        ignore_riposte: false,
    });

    let dispatched = dispatch_event(
        &TargetPool::from_fight(&fight),
        &managers,
        &catalog,
        &mut crate::engine::runtime::determinism::RoundDeterminism::default(),
        &event,
    )
    .unwrap();

    assert!(
        dispatched
            .skills
            .iter()
            .any(|(subscriber, _)| subscriber.owner_uid == -1
                && subscriber.skill_id == 530000151
                && subscriber.key.event == EventKind::TargetAttacked)
    );
    assert!(dispatched.buff_acts.iter().any(|(subscriber, ops)| {
        subscriber.owner_uid == -1
            && subscriber.key.event == EventKind::BeAttacked
            && subscriber.key.definition == DefinitionKey::new(926, "ExPointAddByHit")
            && matches!(ops, Ok(Some(ops)) if matches!(
                ops.as_slice(),
                [
                    buff_act::BuffActRuleOp {
                        op: RuleOp::Command(
                            crate::engine::skill::rule::output::BattleCommand::Buff(_)
                        ),
                        ..
                    },
                    buff_act::BuffActRuleOp {
                        op: RuleOp::BuffFeatureMarker {
                            buff_act_id: 926,
                            ..
                        },
                        ..
                    },
                    buff_act::BuffActRuleOp {
                        op: RuleOp::Command(
                            crate::engine::skill::rule::output::BattleCommand::ExPoint(_)
                        ),
                        ..
                    }
                ]
            ))
    }));
}
