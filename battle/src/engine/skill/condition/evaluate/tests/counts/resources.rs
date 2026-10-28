use super::super::*;
use crate::engine::{
    event::kind::EventKind,
    manager::eureka::EUREKA_RESOURCE_ID,
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
