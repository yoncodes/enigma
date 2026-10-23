use super::*;

#[test]
fn each_death_event_group_settles_before_the_next_group_reacts() {
    crate::test_support::init_config();

    let entity = |uid, team_type, passive_skill| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        position: Some(if uid > 0 { 1 } else { -uid as i32 }),
        current_hp: Some(100),
        passive_skill,
        buffs: match uid {
            10 => vec![BuffInfo {
                uid: Some(1255),
                buff_id: Some(31491201),
                from_uid: Some(10),
                ..Default::default()
            }],
            -1 => vec![BuffInfo {
                uid: Some(1108),
                buff_id: Some(103374115),
                from_uid: Some(-1),
                layer: Some(3),
                ..Default::default()
            }],
            _ => Vec::new(),
        },
        attr: Some(HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, Vec::new())],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                entity(-1, 2, vec![103374115]),
                entity(-2, 2, vec![103374213]),
                entity(-3, 2, vec![103374313]),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut death_managers = BattleManagers::seeded(&fight);
    let origin = CommandOrigin {
        domain: RuleDomain::Lifecycle,
        key: DefinitionKey::new(0, "TestDeathEventGroup"),
    };
    let mut event_groups = Vec::new();
    let mut pending_deaths = Vec::new();
    for target_uid in [-2, -3] {
        let mut changes = death_managers
            .hp
            .execute_command(crate::engine::manager::hp::HpCommand::Kill(
                crate::engine::manager::hp::HpKill {
                    origin,
                    source_uid: 10,
                    target_uid,
                    config_effect: 1,
                },
            ))
            .unwrap();
        event_groups.push(changes.events());
        pending_deaths.extend(changes.take_deaths_with_shares());
    }
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut determinism = RoundDeterminism::default();
    let mut state = DrainState::new(TargetContext::default());
    let mut frames = Vec::new();
    let root = push_root(&mut frames, FrameOwner::Command, FrameTrigger::Active);
    let reactions = dispatch_event_groups(
        &pool,
        &managers,
        &catalog,
        &mut determinism,
        &mut state,
        &event_groups,
        &root,
        &root,
        Some(&root),
        &root,
        None,
        false,
        true,
        crate::engine::event::subscription::PublicationPhase::AfterPublish,
        None,
        None,
        Some(&pending_deaths),
    )
    .unwrap();
    assert_eq!(pending_deaths.len(), 2);
    assert_eq!(
        reactions
            .after_publish
            .iter()
            .filter_map(|queued| match &queued.op {
                RuleOp::Skill(invocation) => Some((invocation.plan.skill_id, 0)),
                RuleOp::DeathEventGroupComplete(death) => {
                    Some((0, death.transition().target_uid))
                }
                _ => None,
            })
            .collect::<Vec<_>>(),
        [
            (103374115, 0),
            (103374213, 0),
            (0, -2),
            (103374115, 0),
            (103374313, 0),
            (0, -3),
        ]
    );
    for target_uid in [-2, -3] {
        managers
            .hp
            .execute_command(crate::engine::manager::hp::HpCommand::Kill(
                crate::engine::manager::hp::HpKill {
                    origin,
                    source_uid: 10,
                    target_uid,
                    config_effect: 1,
                },
            ))
            .unwrap();
    }

    let mut queue = reactions.into_ordered().into();
    let result = drain_queue_with_frames(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext::default(),
        &mut queue,
        frames,
    )
    .unwrap();

    fn collect(step: &sonettobuf::FightStep, order: &mut Vec<(i32, i64)>) {
        if let Some(skill_id @ (103374115 | 103374213 | 103374313)) = step.act_id {
            order.push((skill_id, 0));
        }
        for effect in &step.act_effect {
            if effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Dead as i32) {
                order.push((0, effect.target_id.unwrap()));
            }
            if let Some(child) = effect.fight_step.as_ref() {
                collect(child, order);
            }
        }
    }

    let mut order = Vec::new();
    for step in crate::engine::packet::timeline::project(&result.frames).unwrap() {
        collect(&step, &mut order);
    }
    assert_eq!(
        order,
        [
            (103374115, 0),
            (103374213, 0),
            (0, -2),
            (103374115, 0),
            (103374313, 0),
            (0, -3),
        ]
    );
    assert_eq!(managers.buff.buff_id_amount(10, 103374114), 2);
    assert_eq!(managers.buff.buff_id_uid(10, 103374114), Some(1257));
}

#[test]
fn death_event_group_does_not_cleanup_a_revived_target() {
    crate::test_support::init_config();

    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                position: Some(1),
                current_hp: Some(100),
                attr: Some(HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                position: Some(1),
                current_hp: Some(100),
                attr: Some(HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31_250_181),
                    from_uid: Some(-1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut frames = Vec::new();
    let action = push_root(
        &mut frames,
        FrameOwner::Skill {
            source_uid: 10,
            skill_id: 1,
            card_index: 0,
            target_uid: Some(-1),
        },
        FrameTrigger::Active,
    );
    let mut queue = [QueuedOp {
        op: RuleOp::Command(BattleCommand::Hp(
            crate::engine::manager::hp::HpCommand::Kill(crate::engine::manager::hp::HpKill {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "TestDeathEventGroup"),
                },
                source_uid: 10,
                target_uid: -1,
                config_effect: 1,
            }),
        )),
        trigger: SkillOpTrigger::Active,
        skill_execution: None,
        frame_path: Some(action),
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber: None,
        caster_frame: None,
    }]
    .into();
    let result = drain_queue_with_frames(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &mut queue,
        frames,
    )
    .unwrap();

    assert_eq!(managers.hp.current(-1), 30);
    assert!(!result.outcomes.iter().any(|outcome| matches!(
        outcome,
        crate::engine::runtime::executor::RuleOutcome::DeathSettlement(_)
            | crate::engine::runtime::executor::RuleOutcome::Card(_)
    )));
}
