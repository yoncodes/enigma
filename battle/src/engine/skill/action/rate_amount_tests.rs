use super::*;
use crate::engine::{
    manager::gauge::{GaugeCommand, GaugeManager, GaugeOperation},
    mechanic::lingering_glow,
    skill::rule::{DefinitionKey, RuleDomain},
};

#[test]
fn freezing_dynamic_rates_excludes_later_gauge_changes() {
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60243, "CrystalAddSkillRate"),
    };
    let key = lingering_glow::key(1);
    let mut gauges = GaugeManager::default();
    gauges
        .execute_command(GaugeCommand::new(
            origin,
            key,
            GaugeOperation::Enable { max: Some(1_000) },
        ))
        .unwrap();
    gauges
        .execute_command(GaugeCommand::new(
            origin,
            key,
            GaugeOperation::AccumulateRawValue {
                amount: 139_000,
                stream: 60243,
            },
        ))
        .unwrap();
    let mut modifiers = SkillModifiers {
        rates: vec![SkillRateModifier::new(
            0,
            60243,
            SkillRateAmount::gauge_current(key, 1_000, 4, 1),
            true,
        )],
        ..Default::default()
    };

    modifiers.freeze_rate_amounts(&gauges);
    gauges
        .execute_command(GaugeCommand::new(
            origin,
            key,
            GaugeOperation::AccumulateRawValue {
                amount: 10_000,
                stream: 60243,
            },
        ))
        .unwrap();

    assert_eq!(modifiers.rates[0].fixed_value(), Some(556));
}
