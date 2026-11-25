use super::*;

fn rapport_fixture() -> (TargetPool, BattleManagers, SkillEffectCatalog) {
    crate::test_support::init_config();
    let hero = |uid| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(1),
        current_hp: Some(100),
        attr: Some(HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![hero(10), hero(20)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::new(1, "AddBuff", vec![31460002]),
            TargetRequest::self_only(),
        )],
    });
    (
        TargetPool::from_fight(&fight),
        BattleManagers::seeded(&fight),
        catalog,
    )
}

fn grant() -> SkillInvocation {
    SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into()
}

fn order(items: &[FrameItem]) -> Vec<&'static str> {
    items
        .iter()
        .filter_map(|item| match item {
            FrameItem::Change(change) => match change.as_ref() {
                crate::engine::runtime::change::BattleChange::Buff(changes)
                    if changes.change.added.is_some() || !changes.change.refreshed.is_empty() =>
                {
                    Some("master")
                }
                _ => None,
            },
            FrameItem::Child(child) => match child.owner {
                FrameOwner::BuffRule { .. } => Some("fanout"),
                FrameOwner::Skill { skill_id: 100, .. } => Some("skill"),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[test]
fn layer_halo_fans_out_before_the_master_in_the_same_frame() {
    let (pool, mut managers, catalog) = rapport_fixture();
    for pass in 0..2 {
        let result = run_skill(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            grant(),
            Default::default(),
        )
        .unwrap();
        let skill = result
            .frames
            .iter()
            .find(|frame| matches!(frame.owner, FrameOwner::Skill { skill_id: 100, .. }))
            .expect("skill frame");
        assert_eq!(order(&skill.items), ["fanout", "master"]);
        let refreshed = skill.items.iter().any(|item| {
            matches!(item, FrameItem::Change(change) if matches!(change.as_ref(),
                crate::engine::runtime::change::BattleChange::Buff(changes)
                    if !changes.change.refreshed.is_empty()))
        });
        assert_eq!(refreshed, pass == 1);
    }
}

#[test]
fn event_owned_layer_halo_fanout_follows_the_master_step() {
    let (pool, mut managers, catalog) = rapport_fixture();
    for _ in 0..2 {
        let trigger = SkillOpTrigger::Event(BattleEvent::Kind(
            crate::engine::event::kind::EventKind::RoundEndEntitySettlement,
        ));
        let mut frames = Vec::new();
        let event_rule = push_root(&mut frames, FrameOwner::EventRule, frame_trigger(&trigger));
        let mut queue = VecDeque::from([QueuedOp {
            op: RuleOp::Skill(grant()),
            trigger,
            skill_execution: None,
            frame_path: None,
            parent_path: Some(event_rule),
            frame_group: None,
            independent_parent_group: None,
            frame_owner: None,
            subscriber: None,
            caster_frame: None,
        }]);
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
        assert_eq!(order(&result.frames[0].items), ["skill", "fanout"]);
    }
}

#[test]
fn setup_owned_layer_halo_fanout_follows_the_master_step() {
    assert_setup_layer_halo_fanout(false);
    assert_setup_layer_halo_fanout(true);
}

fn assert_setup_layer_halo_fanout(grouped: bool) {
    let (pool, mut managers, _) = rapport_fixture();
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::new(1, "AddBuff", vec![31460002]),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![crate::engine::skill::condition::ParsedCondition {
        opcode: 103,
        type_name: "None".to_owned(),
        kind: crate::engine::skill::condition::ParsedConditionKind::None(
            crate::engine::skill::condition::none::NoneMode::RoundStart,
        ),
        raw_args: Vec::new(),
    }];
    slot.compiled_route =
        crate::engine::skill::rule::route::ConditionRoute::compile(&slot.conditions);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![slot],
    });
    let trigger = SkillOpTrigger::Setup {
        stage: crate::engine::skill::rule::SetupStage::RoundStart,
        priority: 1,
    };
    let mut frames = Vec::new();
    let side = push_root(
        &mut frames,
        FrameOwner::SetupSide(crate::engine::runtime::record::SetupSide::Attacker),
        frame_trigger(&trigger),
    );
    let scope = if grouped {
        push_child(
            &mut frames,
            &side,
            FrameOwner::SetupEntity { owner_uid: 10 },
            frame_trigger(&trigger),
        )
    } else {
        side
    };
    let mut invocation = grant();
    invocation.condition_slot = Some(0);
    invocation.condition_key = Some(crate::engine::skill::rule::DefinitionKey::new(103, "None"));
    let mut queue = VecDeque::from([QueuedOp {
        op: RuleOp::Skill(invocation),
        trigger,
        skill_execution: None,
        frame_path: None,
        parent_path: Some(scope.clone()),
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber: None,
        caster_frame: None,
    }]);
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
    assert_eq!(
        order(&crate::engine::runtime::record::frame_at(&result.frames, &scope).items),
        ["skill", "fanout"]
    );
}
