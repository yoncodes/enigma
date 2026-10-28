use super::super::*;
use crate::engine::{
    event::kind::EventKind,
    manager::{eureka::EUREKA_RESOURCE_ID, indicator::IndicatorCommand},
    skill::{action::SkillPhase, condition::registry::ConditionRole},
};

#[test]
fn per_aura_repeats_once_per_current_eureka() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                power_infos: vec![PowerInfo {
                    power_id: Some(EUREKA_RESOURCE_ID),
                    num: Some(4),
                    max: Some(5),
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let condition = exact_condition(765203, "PerAura", &[]);

    assert_eq!(condition.kind, ParsedConditionKind::PerEureka);
    assert_eq!(
        crate::engine::skill::condition::registry::find_key(765203, "PerAura")
            .map(|definition| definition.role),
        Some(ConditionRole::Trigger {
            event: EventKind::SkillAction,
            phase: Some(SkillPhase::Immediate),
        })
    );
    assert_eq!(
        conditions_fire_count(
            &[condition],
            10,
            &[10],
            Some(&BattleManagers::seeded(&fight)),
            &TargetPool::from_fight(&fight),
            TargetContext::default(),
        ),
        4
    );
}

#[test]
fn indicator_threshold_reads_the_configured_indicator() {
    init_config();
    let fight = Fight::default();
    let mut managers = BattleManagers::seeded(&fight);
    let condition = exact_condition(98306, "IndicatorNum", &["3713", "15"]);

    managers.indicator.add(IndicatorCommand {
        indicator_id: 3713,
        amount: 14,
        config_effect: 60016,
    });
    assert!(!conditions_match(
        std::slice::from_ref(&condition),
        0,
        &[],
        Some(&managers),
        &TargetPool::from_fight(&fight),
        TargetContext::default(),
    ));

    managers.indicator.add(IndicatorCommand {
        indicator_id: 3713,
        amount: 1,
        config_effect: 60016,
    });
    assert!(conditions_match(
        &[condition],
        0,
        &[],
        Some(&managers),
        &TargetPool::from_fight(&fight),
        TargetContext::default(),
    ));
}
