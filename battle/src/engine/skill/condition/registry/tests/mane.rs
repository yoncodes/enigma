use super::*;

#[test]
fn attack_conditions_keep_their_exact_routes() {
    let incoming = find_key(502202, "ActiveUseSkill").unwrap();
    assert_eq!(
        incoming.role,
        ConditionRole::Trigger {
            event: EventKind::SkillEffectStarted,
            phase: Some(SkillPhase::Immediate),
        }
    );
    assert_eq!(
        incoming.skill_action_observer,
        SkillActionObserver::AttackTarget
    );
    assert_eq!(
        incoming.attack_modifier_side,
        Some(AttackModifierSide::IncomingTarget)
    );

    for (opcode, type_name) in [(501209, "UseHurtSkill"), (792209, "UseDeviceSkill")] {
        assert_eq!(
            find_key(opcode, type_name).map(|definition| definition.role),
            Some(ConditionRole::Trigger {
                event: EventKind::TargetAttacked,
                phase: None,
            })
        );
    }
}

#[test]
fn attacked_reactions_follow_their_passive_source() {
    let timing = find_key(22209, "BeAttacked").unwrap().reaction_timing;

    assert_eq!(
        timing.resolve(PassiveSourceKind::Psychube),
        ReactionTiming::AfterHitObservers
    );
    assert_eq!(
        timing.resolve(PassiveSourceKind::Insight),
        ReactionTiming::AttackedSkills
    );
    assert_eq!(
        timing.resolve(PassiveSourceKind::BattleRule),
        ReactionTiming::AfterSharedHit
    );
}

#[test]
fn tuning_practice_observes_matching_incantations_at_action_start() {
    assert_eq!(
        parse(34201, "UseSkillEffectTag", &["4".into(), "6".into()]),
        Some(ParsedConditionKind::ActiveSkillEffectTag(vec![4, 6]))
    );
    assert_eq!(
        find_key(34201, "UseSkillEffectTag").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::Immediate),
        })
    );
}

#[test]
fn mass_attack_received_keeps_its_target_attacked_route() {
    assert_eq!(
        parse(53209, "HurtNumType", &["2".into()]),
        Some(ParsedConditionKind::ReceivedDamageTargetCountKind(2))
    );
    let definition = find_key(53209, "HurtNumType").unwrap();
    assert_eq!(definition.role, ConditionRole::Predicate);
    assert_eq!(definition.dependencies, &[EventKind::TargetAttacked]);
}

#[test]
fn completed_attack_target_count_keeps_its_after_damage_route() {
    assert_eq!(
        parse(53208, "HurtNumType", &["1".into()]),
        Some(ParsedConditionKind::DamageTargetCountKind(1))
    );
    assert_eq!(
        find_key(53208, "HurtNumType").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::AfterDamage),
        })
    );
}

#[test]
fn attack_modifier_target_count_keeps_its_exact_predicate_route() {
    assert_eq!(
        parse(53203, "HurtNumType", &["1".into()]),
        Some(ParsedConditionKind::DamageTargetCountKind(1))
    );
    assert_eq!(
        find_key(53203, "HurtNumType").map(|definition| definition.role),
        Some(ConditionRole::Predicate)
    );
}

#[test]
fn buff_and_obscurity_conditions_keep_their_exact_routes() {
    assert_eq!(
        parse(88, "BuffTypeAdd", &["4".into()]),
        Some(ParsedConditionKind::BuffTypeAdded(vec![4]))
    );
    assert_eq!(
        find_key(88, "BuffTypeAdd").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::BuffAdded,
            phase: None,
        })
    );
    let power_ratio = find_key(749209, "PowerRatio").unwrap();
    assert_eq!(power_ratio.role, ConditionRole::Predicate);
    assert_eq!(power_ratio.dependencies, &[EventKind::TargetAttacked]);
    assert_eq!(
        power_ratio.reaction_timing,
        ReactionTimingPolicy::Fixed(ReactionTiming::AttackedSkills)
    );
    let absent = find_key(57209, "NoBuffId").unwrap();
    assert_eq!(absent.role, ConditionRole::Predicate);
    assert_eq!(absent.dependencies, &[EventKind::TargetAttacked]);
    assert_eq!(
        absent.reaction_timing,
        ReactionTimingPolicy::Fixed(ReactionTiming::AttackedSkills)
    );
}

#[test]
fn share_damage_observes_the_share_hurt_split() {
    assert_eq!(
        find_key(695213, "ShareDamage").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::DamageShared,
            phase: None,
        })
    );
}

#[test]
fn settlement_and_round_start_conditions_keep_their_exact_routes() {
    assert_eq!(
        find_key(750307, "PlayerHasBuff").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::RoundEndAfterFinalSettlement,
            phase: None,
        })
    );
    assert_eq!(
        find_key(57101, "NoBuffId").map(|definition| definition.role),
        Some(ConditionRole::Setup {
            stage: SetupStage::RoundStartCondition,
            priority: 101,
        })
    );
}

#[test]
fn natural_buff_expiry_observes_buff_removal() {
    assert_eq!(
        parse(515005, "BuffIdExpireOnly", &["4004".into()]),
        Some(ParsedConditionKind::BuffExpired(vec![4004]))
    );
    assert_eq!(
        find_key(515005, "BuffIdExpireOnly").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::BuffRemoved,
            phase: None,
        })
    );
    assert_eq!(
        find_key(515005, "BuffIdExpireOnly").map(|definition| definition.reaction_frame_scope),
        Some(ReactionFrameScope::EventSibling)
    );
}

#[test]
fn captured_mane_conditions_keep_their_exact_routes() {
    assert_eq!(
        parse(73203, "TeammateAliveNum", &["2".into()]),
        Some(ParsedConditionKind::EntityCount {
            scope: crate::engine::skill::condition::parse::EntityCountScope::AliveOtherTeammates,
            compare: ConditionCompare::Equal,
            count: 2,
        })
    );
    assert_eq!(
        find_key(73203, "TeammateAliveNum").map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::Immediate),
        })
    );

    assert_eq!(
        parse(42209, "HasTypeBuffMoreThan", &["6".into(), "1,5".into()]),
        Some(ParsedConditionKind::VisibleStackedStatusTypeCount {
            status_ids: vec![1, 5],
            compare: ConditionCompare::GreaterThanOrEqual,
            threshold: 6,
        })
    );
    let status_count = find_key(42209, "HasTypeBuffMoreThan").unwrap();
    assert_eq!(status_count.role, ConditionRole::Predicate);
    assert_eq!(status_count.dependencies, &[EventKind::TargetAttacked]);
    assert_eq!(
        status_count.reaction_timing.resolve(Default::default()),
        ReactionTiming::AfterSharedHit
    );
    let hurt_skill = find_key(501209, "UseHurtSkill").unwrap();
    assert_eq!(
        hurt_skill.reaction_multiplicity,
        ReactionMultiplicity::OncePerAction
    );
    assert_eq!(
        hurt_skill.reaction_observer_scope,
        ReactionObserverScope::EventTarget
    );

    assert_eq!(
        parse(89304, "ExpointMoreThan", &["3".into()]),
        Some(ParsedConditionKind::ExPoint {
            compare: ConditionCompare::GreaterThanOrEqual,
            threshold: 3,
        })
    );
    let ex_point = find_key(89304, "ExpointMoreThan").unwrap();
    assert_eq!(
        ex_point.role,
        ConditionRole::Trigger {
            event: EventKind::RoundEndAfterSettlement,
            phase: None,
        }
    );
    assert_eq!(ex_point.reaction_frame_target, ReactionFrameTarget::Owner);
}

#[test]
fn assassination_psychubes_wait_for_synchronization_damage() {
    let policy = find_key(1001212, "Assassinate")
        .unwrap()
        .skill_action_trigger;
    assert_eq!(
        policy,
        SkillActionTriggerPolicy::PsychubeRequiresSynchronizationDamage
    );

    let synchronization = crate::engine::skill::action::SkillExecutionMode::Synchronization;
    assert!(policy.rejects_action(
        435211,
        true,
        synchronization,
        PassiveSourceKind::Psychube,
        0,
    ));
    assert!(!policy.rejects_action(
        435211,
        true,
        synchronization,
        PassiveSourceKind::Psychube,
        1,
    ));
    assert!(!policy.rejects_action(
        31050141,
        true,
        synchronization,
        PassiveSourceKind::Insight,
        0,
    ));
    assert!(!policy.rejects_action(
        435211,
        true,
        crate::engine::skill::action::SkillExecutionMode::Active,
        PassiveSourceKind::Psychube,
        0,
    ));
}
