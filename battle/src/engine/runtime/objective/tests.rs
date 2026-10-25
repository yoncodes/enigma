use super::*;
use crate::engine::skill::action::ActionEvent;
use sonettobuf::{Fight, FightEntityInfo, FightTeam};

fn completed_action(mode: SkillExecutionMode, effect_tag: SkillEffectTag) -> RuleOutcome {
    RuleOutcome::SkillLifecycle(SkillLifecycle::ActionCompleted(ActionEvent {
        source_uid: 1,
        skill_id: 100,
        rank: 3,
        effect_tag: effect_tag as i32,
        mode,
        damage_amount: 5_001,
        kill_count: 2,
        ..Default::default()
    }))
}

#[test]
fn records_only_committed_active_incantations_and_actual_finisher() {
    let catalog = SkillEffectCatalog::default();
    let commands = Vec::new();
    let mut progress = ObjectiveProgress::default();
    let result = DrainResult {
        outcomes: vec![
            completed_action(SkillExecutionMode::DirectBig, SkillEffectTag::Debuff),
            completed_action(SkillExecutionMode::Active, SkillEffectTag::Heal),
        ],
        ..Default::default()
    };

    progress.record_player_round(&commands, &catalog, &result, false);
    assert!(progress.used_healing_incantation);
    assert!(!progress.used_debuff_incantation);
    assert_eq!(progress.max_incantation_damage, 5_001);
    assert_eq!(progress.finishing_action, None);

    progress.record_player_round(&commands, &catalog, &result, true);
    assert_eq!(
        progress.finishing_action,
        Some(FinishingAction {
            rank: 3,
            kill_count: 2,
            is_ultimate: false,
        })
    );

    let follow_up = DrainResult {
        outcomes: vec![
            completed_action(SkillExecutionMode::Active, SkillEffectTag::Heal),
            completed_action(SkillExecutionMode::DirectBig, SkillEffectTag::Debuff),
        ],
        ..Default::default()
    };
    progress.record_player_round(&commands, &catalog, &follow_up, true);
    assert_eq!(progress.finishing_action, None);
}

#[test]
fn evaluates_every_configured_advanced_condition_type() {
    let progress = ObjectiveProgress {
        max_ultimates_in_round: 3,
        max_incantation_damage: 5_000,
        finishing_action: Some(FinishingAction {
            rank: 3,
            kill_count: 2,
            is_ultimate: true,
        }),
        ..Default::default()
    };
    let context = || ObjectiveContext {
        dead_attackers: 0,
        current_round: 5,
        average_hp_ratio: 0.751,
    };

    for (type_id, attr) in [
        (1, 1),
        (2, 5),
        (3, 5),
        (4, 1),
        (4, 2),
        (5, 2),
        (6, 1),
        (6, 2),
        (6, 3),
        (6, 4),
        (7, 3),
        (8, 4_999),
        (9, 750),
    ] {
        assert!(condition_met(
            &progress,
            AdvancedConditionType::from_id(type_id).unwrap(),
            attr,
            context(),
        ));
    }
    assert!(!condition_met(
        &progress,
        AdvancedConditionType::IncantationDamage,
        5_000,
        context(),
    ));
    assert!(!condition_met(
        &progress,
        AdvancedConditionType::AverageHp,
        750,
        ObjectiveContext {
            average_hp_ratio: 0.75,
            ..context()
        },
    ));
    assert!(AdvancedConditionType::from_id(19).is_none());
    assert!(AdvancedConditionType::from_id(10).is_none());
}

#[test]
fn promoted_reserve_does_not_erase_casualty_objectives() {
    let entity = |uid, hp, position| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(hp),
        position: Some(position),
        team_type: Some(1),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(1, 100, 1)],
            sub_entitys: vec![entity(2, 100, -1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = BattleRuntime {
        managers: crate::engine::manager::BattleManagers::seeded(&fight),
        fight,
        ..Default::default()
    };

    runtime.managers.hp.lose(1, 100, -1);
    let promotions = runtime.managers.promote_reserves(&mut runtime.fight);
    runtime.objectives.record_promotions(&promotions);
    runtime.managers.sync_roster(&runtime.fight);

    assert_eq!(
        runtime.fight.attacker.as_ref().unwrap().entitys[0].uid,
        Some(2)
    );
    assert_eq!(runtime.meets_advanced_condition(1, 1), Some(false));
    assert_eq!(runtime.meets_advanced_condition(3, 99), Some(false));
    assert_eq!(runtime.meets_advanced_condition(9, 500), Some(false));
    assert_eq!(runtime.meets_advanced_condition(9, 499), Some(true));
}
