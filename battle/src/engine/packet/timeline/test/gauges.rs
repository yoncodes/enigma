use super::*;

#[test]
fn inspiration_gauge_projects_as_emitter_energy() {
    let change = GaugeChange {
        origin: CommandOrigin {
            domain: RuleDomain::BuffAct,
            key: DefinitionKey::new(881, "UseSkillTeamAddEmitterEnergy"),
        },
        key: GaugeKey {
            kind: GaugeKind::ImpromptuInspiration,
            owner: GaugeOwner::Entity(crate::engine::manager::emitter::UID),
        },
        source_uid: 0,
        source_skill_id: 0,
        config_effect: 0,
        progress_raw_delta: 0,
        kind: GaugeChangeKind::Value,
        before: 2,
        requested_delta: 3,
        applied_delta: 3,
        after: 5,
        overflow: 0,
        before_max: Some(6),
        after_max: Some(6),
        enabled_before: true,
        enabled_after: true,
    };

    let effects = project_change_for_test(&BattleChange::Gauge(change)).unwrap();

    assert_eq!(
        effects[0].target_id,
        Some(crate::engine::manager::emitter::UID)
    );
    assert_eq!(effects[0].effect_num1, Some(3));
}

#[test]
fn per_type_buff_emitter_energy_projects_one_generic_effect_without_state_payloads() {
    let change = GaugeChange {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(60266, "PerTypeBuffAddEnergyToEmitter"),
        },
        key: GaugeKey {
            kind: GaugeKind::ImpromptuInspiration,
            owner: GaugeOwner::Entity(crate::engine::manager::emitter::UID),
        },
        source_uid: 10,
        source_skill_id: 0,
        config_effect: 60266,
        progress_raw_delta: 0,
        kind: GaugeChangeKind::Value,
        before: 0,
        requested_delta: 14,
        applied_delta: 14,
        after: 14,
        overflow: 0,
        before_max: Some(30),
        after_max: Some(30),
        enabled_before: true,
        enabled_after: true,
    };

    let effects = project_change_for_test(&BattleChange::Gauge(change)).unwrap();
    assert_eq!(effects.len(), 1);
    let effect = &effects[0];
    assert_eq!(effect.target_id, Some(crate::engine::manager::emitter::UID));
    assert_eq!(
        effect.effect_type,
        Some(EffectType::Emitterenergychange as i32)
    );
    assert_eq!(effect.effect_num, Some(1));
    assert_eq!(effect.effect_num1, Some(14));
    assert_eq!(effect.power_info, None);
    assert_eq!(effect.buff, None);
    assert_eq!(effect.entity, None);
    assert_eq!(effect.emitter_info, None);
    assert_eq!(effect.buff_act_info, None);
    assert_eq!(effect.reserve_str, None);
}

#[test]
fn team_energy_gauge_projects_one_generic_effect_without_power_or_marker_state() {
    let change = GaugeChange {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(60264, "PerTypeBuffAddEnergyToTeam"),
        },
        key: GaugeKey {
            kind: crate::engine::manager::gauge::GaugeKind::TeamEnergy,
            owner: crate::engine::manager::gauge::GaugeOwner::Team(1),
        },
        source_uid: 10,
        source_skill_id: 0,
        config_effect: 60264,
        progress_raw_delta: 0,
        kind: GaugeChangeKind::Value,
        before: 0,
        requested_delta: 7,
        applied_delta: 7,
        after: 7,
        overflow: 0,
        before_max: None,
        after_max: None,
        enabled_before: true,
        enabled_after: true,
    };

    let effects = project_change_for_test(&BattleChange::Gauge(change)).unwrap();
    assert_eq!(effects.len(), 1);
    let effect = &effects[0];
    assert_eq!(effect.target_id, Some(0));
    assert_eq!(
        effect.effect_type,
        Some(EffectType::Teamenergychange as i32)
    );
    assert_eq!(effect.effect_num, Some(1));
    assert_eq!(effect.effect_num1, Some(7));
    assert_eq!(effect.power_info, None);
    assert_eq!(effect.buff, None);
    assert_eq!(effect.entity, None);
    assert_eq!(effect.emitter_info, None);
    assert_eq!(effect.buff_act_info, None);
    assert_eq!(effect.reserve_str, None);
}

#[test]
fn bloodtithe_gauge_projects_creation_and_attributed_value_changes() {
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60191, "BloodPoolValueChange"),
    };
    let key = crate::engine::mechanic::bloodtithe::rule::key(1);
    let enabled = GaugeChange {
        origin,
        key,
        source_uid: 0,
        source_skill_id: 0,
        config_effect: 0,
        progress_raw_delta: 0,
        kind: GaugeChangeKind::Enabled,
        before: 0,
        requested_delta: 0,
        applied_delta: 0,
        after: 0,
        overflow: 0,
        before_max: None,
        after_max: Some(56),
        enabled_before: false,
        enabled_after: true,
    };
    let value = GaugeChange {
        source_uid: 10,
        config_effect: 7,
        kind: GaugeChangeKind::Value,
        requested_delta: 16,
        applied_delta: 16,
        after: 16,
        before_max: Some(56),
        ..enabled
    };

    let created = project_change_for_test(&BattleChange::Gauge(enabled)).unwrap();
    let changed = project_change_for_test(&BattleChange::Gauge(value)).unwrap();

    assert_eq!(created.len(), 2);
    assert_eq!(
        created[0].effect_type,
        Some(EffectType::Bloodpoolmaxcreate as i32)
    );
    assert_eq!(created[1].effect_num1, Some(56));
    assert_eq!(changed[0].target_id, Some(10));
    assert_eq!(changed[0].config_effect, Some(7));
    assert_eq!(changed[0].effect_num1, Some(16));
}
