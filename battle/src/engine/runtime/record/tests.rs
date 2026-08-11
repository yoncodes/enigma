use super::*;

#[test]
fn resolved_skill_target_does_not_replace_the_invocation_target() {
    let mut frames = Vec::new();
    let skill = push_root(
        &mut frames,
        FrameOwner::Skill {
            source_uid: 10,
            skill_id: 20,
            card_index: 0,
            target_uid: Some(30),
        },
        FrameTrigger::Active,
    );

    set_skill_target(&mut frames, &skill, Some(10));

    assert!(matches!(
        frames[0].owner,
        FrameOwner::Skill {
            target_uid: Some(30),
            ..
        }
    ));
}

#[test]
fn setup_events_share_the_current_entity_scope() {
    let trigger = FrameTrigger::Setup {
        stage: SetupStage::EnterFight,
        priority: 0,
    };
    let mut frames = Vec::new();
    let side = push_root(
        &mut frames,
        FrameOwner::SetupSide(SetupSide::Attacker),
        trigger.clone(),
    );
    let entity = push_child(
        &mut frames,
        &side,
        FrameOwner::SetupEntity { owner_uid: 10 },
        trigger.clone(),
    );
    let skill = push_child(
        &mut frames,
        &entity,
        FrameOwner::Skill {
            source_uid: 10,
            skill_id: 20,
            card_index: 0,
            target_uid: Some(10),
        },
        trigger,
    );

    assert_eq!(event_scope_path(&frames, &skill), entity);
}

#[test]
fn active_skill_events_keep_the_nested_skill_scope_during_setup() {
    let setup = FrameTrigger::Setup {
        stage: SetupStage::EnterFight,
        priority: 0,
    };
    let mut frames = Vec::new();
    let side = push_root(
        &mut frames,
        FrameOwner::SetupSide(SetupSide::Attacker),
        setup.clone(),
    );
    let entity = push_child(
        &mut frames,
        &side,
        FrameOwner::SetupEntity { owner_uid: 10 },
        setup,
    );
    let skill = push_child(
        &mut frames,
        &entity,
        FrameOwner::Skill {
            source_uid: 10,
            skill_id: 20,
            card_index: 0,
            target_uid: Some(11),
        },
        FrameTrigger::Active,
    );

    assert_eq!(event_scope_path(&frames, &skill), skill);
}

#[test]
fn reactive_skill_events_share_the_current_setup_entity_scope() {
    let setup = FrameTrigger::Setup {
        stage: SetupStage::EnterFight,
        priority: 0,
    };
    let mut frames = Vec::new();
    let side = push_root(
        &mut frames,
        FrameOwner::SetupSide(SetupSide::Attacker),
        setup.clone(),
    );
    let entity = push_child(
        &mut frames,
        &side,
        FrameOwner::SetupEntity { owner_uid: 10 },
        setup,
    );
    let skill = push_child(
        &mut frames,
        &entity,
        FrameOwner::Skill {
            source_uid: 10,
            skill_id: 20,
            card_index: 0,
            target_uid: Some(10),
        },
        FrameTrigger::Event(BattleEvent::Kind(
            crate::engine::event::kind::EventKind::BuffChanged,
        )),
    );

    assert_eq!(event_scope_path(&frames, &skill), entity);
}

#[test]
fn runtime_buff_act_events_publish_reactive_skills_beside_the_causing_frame() {
    let trigger = FrameTrigger::Active;
    let mut frames = Vec::new();
    let skill = push_root(
        &mut frames,
        FrameOwner::Skill {
            source_uid: 10,
            skill_id: 20,
            card_index: 0,
            target_uid: Some(11),
        },
        trigger.clone(),
    );
    let buff_act = push_child(
        &mut frames,
        &skill,
        FrameOwner::BuffAct {
            owner_uid: 11,
            source_uid: 10,
            buff_uid: 30,
            buff_id: 40,
            key: crate::engine::skill::rule::DefinitionKey::new(50, "BuffAct"),
        },
        trigger,
    );

    assert_eq!(event_scope_path(&frames, &buff_act), skill);
}

#[test]
fn event_skills_keep_committed_changes_in_the_active_action_scope() {
    let mut frames = Vec::new();
    let action = push_root(
        &mut frames,
        FrameOwner::Skill {
            source_uid: 10,
            skill_id: 20,
            card_index: 0,
            target_uid: Some(11),
        },
        FrameTrigger::Active,
    );
    let reaction = push_child(
        &mut frames,
        &action,
        FrameOwner::Skill {
            source_uid: 12,
            skill_id: 30,
            card_index: 0,
            target_uid: Some(12),
        },
        FrameTrigger::Event(BattleEvent::AllyAction(Default::default())),
    );

    assert_eq!(active_skill_scope_path(&frames, &reaction), Some(action));
}

#[test]
fn conduit_child_is_the_active_skill_scope_but_the_wrapper_is_not() {
    let mut frames = Vec::new();
    let wrapper = push_root(
        &mut frames,
        FrameOwner::ConduitAction {
            source_uid: 10,
            group: 1,
            skill_position: 1,
            target_uid: None,
        },
        FrameTrigger::Active,
    );
    let child = push_child(
        &mut frames,
        &wrapper,
        FrameOwner::ConduitSkill {
            source_uid: 10,
            skill_id: 31490111,
            card_index: 1,
            target_uid: None,
        },
        FrameTrigger::Active,
    );

    assert_eq!(active_skill_scope_path(&frames, &wrapper), None);
    assert_eq!(active_skill_scope_path(&frames, &child), Some(child));
}

#[test]
fn conduit_child_target_updates_both_anchors_without_replacing_them() {
    let mut frames = Vec::new();
    let wrapper = push_root(
        &mut frames,
        FrameOwner::ConduitAction {
            source_uid: 10,
            group: 1,
            skill_position: 1,
            target_uid: None,
        },
        FrameTrigger::Active,
    );
    let child = push_child(
        &mut frames,
        &wrapper,
        FrameOwner::ConduitSkill {
            source_uid: 10,
            skill_id: 31490111,
            card_index: 1,
            target_uid: None,
        },
        FrameTrigger::Active,
    );

    set_skill_target(&mut frames, &child, Some(20));
    set_skill_target(&mut frames, &child, Some(30));

    assert!(matches!(
        frames[0].owner,
        FrameOwner::ConduitAction {
            target_uid: Some(20),
            ..
        }
    ));
    assert!(matches!(
        owner_at_path(&frames, &child),
        FrameOwner::ConduitSkill {
            target_uid: Some(20),
            ..
        }
    ));
}
