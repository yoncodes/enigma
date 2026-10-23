use super::*;

fn attr_max_hp_change(
    source_uid: i64,
    target_uid: i64,
    before_current: i32,
    after_current: i32,
    delta: i32,
) -> BattleChange {
    BattleChange::Hp(Box::new(HpChanges {
        origin: CommandOrigin {
            domain: RuleDomain::BuffAct,
            key: DefinitionKey::new(100, "Attr"),
        },
        source_uid,
        target_uid,
        publishes_hit: true,
        damage: None,
        fake_hp_consumed: None,
        team_shared_shield_absorbed: None,
        team_shared_shield_removed: None,
        shared_hurt: None,
        shield_absorbed: None,
        shield_granted: None,
        max_hp: Some(MaxHpChange {
            target_uid,
            before_current,
            before_max: 100,
            delta,
            after_current,
            after_max: 100 + delta,
        }),
        hp: None,
        toughness: None,
        kill: None,
        death: None,
    }))
}

#[test]
fn attr_max_hp_wire_follows_applier_and_real_current_hp_changes() {
    let cases = [
        (10, 10, 100, 120, 20, vec![108, 109, 108, 109], 0),
        (20, 10, 100, 120, 20, vec![108, 109], 0),
        (10, 10, 100, 80, -20, vec![108, 109, 108], 0),
        (20, 10, 100, 80, -20, vec![108], 100),
        (10, 10, 50, 50, -20, vec![108, 108], 0),
    ];

    for (source_uid, target_uid, before_current, after_current, delta, expected, buff_act_id) in
        cases
    {
        let effects = project_change_for_test(&attr_max_hp_change(
            source_uid,
            target_uid,
            before_current,
            after_current,
            delta,
        ))
        .unwrap();

        assert_eq!(
            effects
                .iter()
                .map(|effect| effect.effect_type.unwrap())
                .collect::<Vec<_>>(),
            expected
        );
        assert!(
            effects
                .iter()
                .all(|effect| effect.target_id == Some(target_uid))
        );
        assert!(effects.iter().all(|effect| {
            effect.effect_type != Some(EffectType::Maxhpchange as i32)
                || effect.buff_act_id == Some(buff_act_id)
        }));
    }
}

#[test]
fn semantic_buff_marker_projects_real_damage_kill() {
    let effects = project_change_for_test(&BattleChange::BuffFeatureMarker(BuffMarkerResult {
        target_uid: -1,
        effect_type: EffectType::Realdamagekill as i32,
        effect_num: 9999,
        buff_act_id: 0,
    }))
    .unwrap();

    assert_eq!(effects[0].target_id, Some(-1));
    assert_eq!(
        effects[0].effect_type,
        Some(EffectType::Realdamagekill as i32)
    );
    assert_eq!(effects[0].effect_num, Some(9999));
}

#[test]
fn raspberry_capacity_at_cap_projects_sync_without_an_hp_change() {
    let result = crate::engine::skill::buff_act::raspberry::CapacityResult::AtCap(
        crate::engine::skill::buff_act::raspberry::CapacityAtCap {
            target_uid: 10,
            buff_uid: 77,
            buff_act_id: 60231,
            current: 2_502,
            cap: 2_502,
            max_hp: 83_400,
        },
    );

    let effects =
        project_change_for_test(&BattleChange::RaspberryCapacity(Box::new(result))).unwrap();

    assert_eq!(effects.len(), 2);
    assert_eq!(
        effects
            .iter()
            .map(|effect| effect.effect_type.unwrap())
            .collect::<Vec<_>>(),
        vec![
            EffectType::Buffactinfoupdate as i32,
            EffectType::Maxhpchange as i32,
        ]
    );
    assert_eq!(effects[0].target_id, Some(10));
    assert_eq!(effects[0].reserve_id, Some(77));
    assert_eq!(
        effects[0].buff_act_info.as_ref().unwrap().param,
        [2_502, 2_502]
    );
    assert_eq!(effects[1].effect_num, Some(83_400));
    assert_eq!(effects[1].buff_act_id, Some(60231));
    assert!(
        effects
            .iter()
            .all(|effect| effect.effect_type != Some(EffectType::Currenthpchange as i32))
    );
}

#[test]
fn recorded_pre_add_wire_effect_stays_before_buff_add() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut buffs = crate::engine::manager::buff::BuffManager::default();
    buffs.seed(&fight);
    let added = buffs
        .add(
            &crate::engine::manager::hp::HpManager::default(),
            10,
            10,
            7_280_002,
            0,
        )
        .unwrap();

    let effects = EffectPacket::buff_add_direct(&added);

    assert_eq!(
        effects[0].effect_type,
        Some(EffectType::Nuodikarandomattacknum as i32)
    );
    assert_eq!(effects[0].effect_num1, Some(1));
    assert_eq!(effects[1].effect_type, Some(EffectType::Buffadd as i32));
}

#[test]
fn projects_a_semantic_skill_frame_without_reading_runtime_state() {
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(20002, "AddExPoint"),
    };
    let frame = SemanticFrame {
        owner: FrameOwner::Skill {
            source_uid: 10,
            skill_id: 20,
            card_index: 0,
            target_uid: Some(-1),
        },
        trigger: crate::engine::runtime::record::FrameTrigger::Active,
        items: vec![FrameItem::Change(Box::new(BattleChange::ExPoint(
            ExPointChanges::Value {
                origin,
                change: ExPointApplyResult {
                    source_uid: 10,
                    target_uid: 10,
                    kind: ExPointKind::Common,
                    before: 0,
                    requested_delta: 1,
                    applied_delta: 1,
                    after: 1,
                    overflow: 0,
                    cap: 5,
                    effect_type: 0,
                    config_effect: 0,
                },
            },
        )))],
    };

    let steps = project(&[frame]).unwrap();

    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].from_id, Some(10));
    assert_eq!(steps[0].to_id, Some(-1));
    assert_eq!(steps[0].act_id, Some(20));
    assert_eq!(steps[0].act_effect[0].effect_num, Some(1));
}

#[test]
fn lifecycle_mechanic_projects_as_nested_fake_timeline() {
    let frame = SemanticFrame {
        owner: FrameOwner::Skill {
            source_uid: -1,
            skill_id: 20,
            card_index: 0,
            target_uid: Some(-1),
        },
        trigger: crate::engine::runtime::record::FrameTrigger::Active,
        items: vec![FrameItem::Child(Box::new(SemanticFrame {
            owner: FrameOwner::LifecycleMechanic,
            trigger: crate::engine::runtime::record::FrameTrigger::Active,
            items: vec![FrameItem::Change(Box::new(BattleChange::ToughnessReset(
                crate::engine::manager::toughness::ToughnessReset {
                    target_uid: -1,
                    before: None,
                    after: None,
                },
            )))],
        }))],
    };

    let steps = project(&[frame]).unwrap();
    let nested = steps[0].act_effect[0].fight_step.as_ref().unwrap();

    assert_eq!(nested.fake_timeline, Some(true));
    assert_eq!(
        nested.act_effect[0].effect_type,
        Some(EffectType::Toughnessreset as i32)
    );
}
