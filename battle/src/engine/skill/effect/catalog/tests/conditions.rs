use super::*;

#[test]
fn device_ultimate_compound_uses_the_activation_driver_in_either_order() {
    init_config();

    for raw in ["25208&792208", "792208&25208"] {
        let conditions =
            crate::engine::skill::condition::parse_conditions(config::configs::get(), raw);
        let route =
            crate::engine::skill::rule::route::ConditionRoute::compile(&conditions).unwrap();

        let descriptors = &route.branches[0].conditions;
        assert_eq!(descriptors.len(), 2);
        for key in [
            crate::engine::skill::rule::DefinitionKey::new(25208, "UseExSkill"),
            crate::engine::skill::rule::DefinitionKey::new(792208, "UseDeviceSkill"),
        ] {
            assert_eq!(
                descriptors
                    .iter()
                    .filter(|descriptor| descriptor.key == key)
                    .count(),
                1
            );
        }
        assert_eq!(
            route.branches[0].driver,
            Some(crate::engine::skill::rule::route::ConditionDriver::Trigger(
                crate::engine::skill::rule::route::ConditionTrigger {
                    key: crate::engine::skill::rule::DefinitionKey::new(792208, "UseDeviceSkill",),
                    event: crate::engine::event::kind::EventKind::ConduitActivated,
                    phase: None,
                },
            ))
        );
    }
}

#[test]
fn ultimate_rank_compound_keeps_its_after_hit_driver_and_buff_branches() {
    init_config();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let slots = &catalog.get(30920211).unwrap().slots;

    for (slot, levels, buff_id, target_code) in [
        (&slots[2], vec![0, 1, 2, 3, 4], 31020113, 121),
        (&slots[3], vec![0, 1, 2, 3, 4], 31020112, 123),
        (&slots[4], vec![5], 31020119, 121),
        (&slots[5], vec![5], 31020118, 123),
    ] {
        assert_eq!(slot.condition_target.code, 103);
        assert!(slot.conditions.iter().any(|condition| {
            condition.kind == ParsedConditionKind::TargetIdentity {
                mode: crate::engine::skill::condition::TargetIdentityMode::ActiveSkillSourceModelId,
                value: 3102,
            }
        }));
        assert!(
            slot.conditions.iter().any(|condition| {
                condition.opcode == 25210 && condition.type_name == "UseExSkill"
            })
        );
        assert!(slot.conditions.iter().any(|condition| {
            condition.kind == ParsedConditionKind::ExSkillLevels(levels.clone())
        }));
        assert_eq!(
            slot.compiled_subscriptions().unwrap(),
            vec![SubscriptionKey::at_phase(
                crate::engine::event::kind::EventKind::SkillAction,
                crate::engine::skill::rule::DefinitionKey::new(25210, "UseExSkill"),
                Some(crate::engine::skill::action::SkillPhase::AfterHit),
            )]
        );
        assert_eq!(slot.behavior.spec.kind, BehaviorKind::AddBuff);
        assert_eq!(slot.behavior.arg(0), Some(buff_id));
        assert_eq!(slot.target.code, target_code);
    }
}

#[test]
fn target_count_and_effect_tag_share_the_effect_tag_driver() {
    init_config();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let subscriptions = catalog.compiled_subscriptions(432511).unwrap();

    assert!(subscriptions.iter().any(|subscription| {
        subscription.definition
            == crate::engine::skill::rule::DefinitionKey::new(34210, "UseSkillEffectTag")
            && subscription.phase == Some(crate::engine::skill::action::SkillPhase::HitPassives)
            && subscription.timing == crate::engine::event::subscription::ReactionTiming::AfterSkill
    }));
    assert!(
        !subscriptions
            .iter()
            .any(|subscription| subscription.definition.opcode == 500210)
    );
}

#[test]
fn psychube_buff_category_gate_keeps_threshold_then_status_order() {
    init_config();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());

    assert_eq!(
        catalog.condition_kind(
            432715,
            crate::engine::skill::rule::DefinitionKey::new(511201, "HasTypeBuffIdsMoreThan",),
        ),
        Some(&ParsedConditionKind::BuffStatusCount {
            status_ids: vec![8],
            compare: crate::engine::skill::condition::ConditionCompare::GreaterThanOrEqual,
            threshold: 2,
        })
    );
    assert_eq!(
        catalog.condition_kind(
            432715,
            crate::engine::skill::rule::DefinitionKey::new(701201, "HasMasterHalo"),
        ),
        Some(&ParsedConditionKind::MasterHalo)
    );
}

#[test]
fn master_halo_immediate_gate_uses_the_skill_extra_type_driver() {
    init_config();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let effect = catalog.get(435811).unwrap();
    let slot = &effect.slots[1];

    assert!(slot.conditions.iter().any(|condition| {
        condition.opcode == 701203 && condition.kind == ParsedConditionKind::MasterHalo
    }));
    assert_eq!(
        slot.compiled_subscriptions().unwrap(),
        vec![SubscriptionKey::at_phase(
            crate::engine::event::kind::EventKind::SkillEffectStarted,
            crate::engine::skill::rule::DefinitionKey::new(403203, "SkillExtraType"),
            Some(crate::engine::skill::action::SkillPhase::Immediate),
        )]
    );
}

#[test]
fn ritual_dance_threshold_compiles_the_captured_immediate_route() {
    init_config();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());

    assert_eq!(
        catalog.condition_kind(
            31100531,
            crate::engine::skill::rule::DefinitionKey::new(
                537201,
                "HasTypeIdBuffTotalCountMoreThan",
            ),
        ),
        Some(&ParsedConditionKind::BuffTypeCount {
            type_ids: vec![31100201],
            compare: crate::engine::skill::condition::ConditionCompare::GreaterThanOrEqual,
            threshold: 4,
        })
    );
    assert!(
        catalog
            .compiled_subscriptions(31100531)
            .unwrap()
            .iter()
            .any(|subscription| {
                subscription.definition
                    == crate::engine::skill::rule::DefinitionKey::new(
                        537201,
                        "HasTypeIdBuffTotalCountMoreThan",
                    )
                    && subscription.event == crate::engine::event::kind::EventKind::SkillAction
                    && subscription.phase
                        == Some(crate::engine::skill::action::SkillPhase::Immediate)
            })
    );
}

#[test]
fn liang_yue_poison_scaling_compiles_as_an_active_modifier() {
    init_config();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());

    assert_eq!(
        catalog.condition_kind(
            31100563,
            crate::engine::skill::rule::DefinitionKey::new(669203, "PerBuffGroupCount"),
        ),
        Some(&ParsedConditionKind::PerBuffGroupCount { group_id: 7 })
    );
    assert!(catalog.issues(31100563).iter().all(|issue| {
        issue.opcode != Some(669203) || issue.type_name.as_deref() != Some("PerBuffGroupCount")
    }));
}

#[test]
fn crucible_arsenal_compiles_each_bullet_type_modifier() {
    init_config();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let effect = catalog.get(310201621).unwrap();
    let key = crate::engine::skill::rule::DefinitionKey::new(651203, "PerBullet");

    for (slot, attr_id) in effect.slots[1..4].iter().zip([205, 214, 211]) {
        assert_eq!(slot.condition_target.code, 103);
        assert_eq!(slot.target.code, 103);
        assert_eq!(slot.behavior.spec.kind, BehaviorKind::AttrFix);
        assert_eq!(slot.behavior.args, vec![attr_id, 50]);
        assert!(slot.conditions.iter().any(|condition| {
            condition.opcode == key.opcode
                && condition.type_name == key.type_name
                && condition.kind
                    == ParsedConditionKind::PerBullet {
                        divisor: 1,
                        max_count: 8,
                    }
        }));
        assert!(slot.compiled_route.is_ok());
    }
    assert!(catalog.issues(310201621).iter().all(|issue| {
        issue.opcode != Some(651203) || issue.type_name.as_deref() != Some("PerBullet")
    }));
}

#[test]
fn nights_command_compiles_each_bullet_type_grant_after_hit() {
    init_config();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let key = crate::engine::skill::rule::DefinitionKey::new(651210, "PerBullet");

    for skill_id in 30920141..=30920149 {
        let slot = &catalog.get(skill_id).unwrap().slots[1];
        assert_eq!(slot.condition_target.code, 103);
        assert_eq!(slot.target.code, 103);
        assert_eq!(slot.behavior.spec.kind, BehaviorKind::AddBuff);
        assert!(slot.conditions.iter().any(|condition| {
            condition.opcode == key.opcode
                && condition.type_name == key.type_name
                && condition.kind
                    == ParsedConditionKind::PerBullet {
                        divisor: 1,
                        max_count: 99,
                    }
        }));
        assert!(
            slot.compiled_subscriptions()
                .unwrap()
                .iter()
                .any(|subscription| {
                    subscription.definition == key
                        && subscription.event == crate::engine::event::kind::EventKind::SkillAction
                        && subscription.phase
                            == Some(crate::engine::skill::action::SkillPhase::AfterHit)
                })
        );
        assert!(catalog.issues(skill_id).iter().all(|issue| {
            issue.opcode != Some(651210) || issue.type_name.as_deref() != Some("PerBullet")
        }));
    }
}

#[test]
fn from_the_depths_keeps_its_once_per_battle_limit() {
    init_config();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());

    assert_eq!(
        catalog.limit_for_condition(
            433011,
            crate::engine::skill::rule::DefinitionKey::new(629210, "TeammateInjuryCount",),
        ),
        1
    );
    assert_eq!(
        catalog.limit_for_condition(
            433011,
            crate::engine::skill::rule::DefinitionKey::new(630212, "TeammateInjuryCountNotReset",),
        ),
        0
    );
}

#[test]
fn from_game_db_keeps_skill_effect_logic_target() {
    init_config();

    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());

    assert_eq!(catalog.logic_target(31140151), 112);
}

#[test]
fn from_game_db_resolves_dynamic_behavior_targets_like_lua() {
    init_config();

    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let effect = catalog.get(530000421).unwrap();

    assert_eq!(effect.slots[0].target.code, 202);
}

#[test]
fn dynamic_behavior_targets_keep_per_target_conditions_for_logic_targets() {
    init_config();

    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let effect = catalog.get(30980121).unwrap();

    assert_eq!(effect.slots[1].condition_target.code, 0);
    assert_eq!(effect.slots[1].target.code, 202);
    assert!(effect.slots[1].target_from_condition);
}

#[test]
fn from_game_db_compiles_lifecycle_conditions_to_setup_stages() {
    init_config();

    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());

    let enter = catalog.get(31140142).unwrap();
    assert!(
        !enter.slots[0]
            .setup_keys(crate::engine::skill::rule::SetupStage::EnterFight, 0)
            .is_empty()
    );

    let battle_start = catalog.get(31200146).unwrap();
    assert!(
        !battle_start.slots[0]
            .setup_keys(crate::engine::skill::rule::SetupStage::BattleStart, 0)
            .is_empty()
    );

    let career_static = catalog.get(434511).unwrap();
    assert!(
        !career_static.slots[1]
            .setup_keys(crate::engine::skill::rule::SetupStage::EnterFight, 0)
            .is_empty()
    );

    let per_career_static = catalog.get(31280143).unwrap();
    assert!(
        !per_career_static.slots[1]
            .setup_keys(crate::engine::skill::rule::SetupStage::EnterFight, 0)
            .is_empty()
    );
}

#[test]
fn active_and_passive_use_skill_type_not_damage_type() {
    init_config();

    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());

    assert!(catalog.is_active(30610111));
    assert!(!catalog.is_passive(30610111));
    assert!(catalog.is_passive(30610141));
    assert_eq!(catalog.skill_type(30950127), 2);
}
