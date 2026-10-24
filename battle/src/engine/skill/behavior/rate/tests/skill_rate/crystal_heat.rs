use super::*;

#[test]
fn heat_scale_rate_uses_the_configured_scale_and_limit() {
    assert_eq!(heat_scale_skill_rate(12, &[1000, 20, 9999]), 240);
    assert_eq!(heat_scale_skill_rate(20, &[10000, 350, 9999]), 700);
}

#[test]
fn caster_targeted_heat_scale_rate_applies_to_the_whole_attack() {
    use crate::engine::{
        manager::gauge::{GaugeCommand, GaugeOperation},
        mechanic::lingering_glow,
        skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
    };

    let mut managers = BattleManagers::default();
    managers
        .gauge
        .execute_command(GaugeCommand::new(
            CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60255, "HeatScaleAddSkillRate"),
            },
            lingering_glow::key(1),
            GaugeOperation::Enable { max: Some(9999) },
        ))
        .unwrap();
    managers
        .gauge
        .execute_command(GaugeCommand::new(
            CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60255, "HeatScaleAddSkillRate"),
            },
            lingering_glow::key(1),
            GaugeOperation::ChangeValue { delta: 12 },
        ))
        .unwrap();
    let pool = TargetPool::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = TargetContext::default();

    assert!(matches!(
        crate::engine::skill::behavior::rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 200,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &ParsedBehavior::new(60255, "HeatScaleAddSkillRate", vec![1000, 20, 9999]),
        ),
        Some(ops) if ops.is_empty()
    ));
    assert_eq!(modifiers.rates[0].target_uid, 0);
    assert_eq!(modifiers.rates[0].fixed_value(), Some(240));
}

#[test]
fn purple_crystal_resolves_visible_lingering_glow_when_damage_is_planned() {
    use crate::engine::{
        manager::gauge::{GaugeCommand, GaugeManager, GaugeOperation},
        mechanic::lingering_glow,
        skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
    };

    let key = lingering_glow::key(1);
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60243, "CrystalAddSkillRate"),
    };
    let mut gauges = GaugeManager::default();
    gauges
        .execute_command(GaugeCommand::new(
            origin,
            key,
            GaugeOperation::Enable { max: Some(1000) },
        ))
        .unwrap();
    let modifier = SkillRateModifier::new(
        0,
        60243,
        crate::engine::skill::action::SkillRateAmount::gauge_current(key, 1_000, 4, 1),
        true,
    );

    gauges
        .execute_command(GaugeCommand::new(
            origin,
            key,
            GaugeOperation::AccumulateRawValue {
                amount: 120_000,
                stream: 60243,
            },
        ))
        .unwrap();
    assert_eq!(modifier.amount.resolve(&gauges), 480);

    gauges
        .execute_command(GaugeCommand::new(
            origin,
            key,
            GaugeOperation::AccumulateRawValue {
                amount: 30_000,
                stream: 60243,
            },
        ))
        .unwrap();
    assert_eq!(modifier.amount.resolve(&gauges), 600);

    gauges
        .execute_command(GaugeCommand::new(
            origin,
            key,
            GaugeOperation::AccumulateRawValue {
                amount: 500,
                stream: 60243,
            },
        ))
        .unwrap();
    assert_eq!(gauges.raw_value(key), Some(150_500));
    assert_eq!(modifier.amount.resolve(&gauges), 600);
}

#[test]
fn crystal_focus_prefers_the_first_enemy_tied_for_the_highest_halo() {
    crate::test_support::init_config();
    let enemy = |uid, layer| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(2),
        current_hp: Some(100),
        buffs: vec![BuffInfo {
            uid: Some(-uid),
            buff_id: Some(31340001),
            from_uid: Some(10),
            layer: Some(layer),
            ..Default::default()
        }],
        ..Default::default()
    };
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
        defender: Some(FightTeam {
            entitys: vec![enemy(-1, 1), enemy(-2, 2), enemy(-3, 2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);

    assert_eq!(
        first_enemy_with_highest_buff_amount(&pool, &managers, 10, 31340001),
        Some(-2)
    );
}

#[test]
fn selected_crystal_rank_row_scales_mass_and_focus_damage_by_matching_crystals() {
    let mut managers = BattleManagers::default();
    assert_eq!(
        crystal_fixed_skill_rates(&managers, 10, &[1200, 800, 1]),
        Some((0, 0))
    );
    assert!(managers.emanation.select(10, 210));
    assert_eq!(
        crystal_fixed_skill_rates(&managers, 10, &[1200, 800, 1]),
        Some((2400, 1600))
    );
    assert_eq!(
        crystal_fixed_skill_rates(&managers, 10, &[1200, 800, 2]),
        Some((1200, 800))
    );
    assert_eq!(
        crystal_fixed_skill_rates(&managers, 10, &[1200, 800, 4]),
        None
    );
}

#[test]
fn crystal_damage_components_declare_their_career_scope() {
    assert!(crystal_rate_career_scaled(
        BehaviorKind::CrystalAddSkillRate,
        false
    ));
    assert!(crystal_rate_career_scaled(
        BehaviorKind::CrystalAddSkillRate,
        true
    ));
    assert!(crystal_rate_career_scaled(
        BehaviorKind::CrystalAddCardRank,
        false
    ));
    assert!(crystal_rate_career_scaled(
        BehaviorKind::CrystalAddCardRank,
        true
    ));
}
