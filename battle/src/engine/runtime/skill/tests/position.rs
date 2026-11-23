use sonettobuf::effect_type_enum::EffectType;

use super::*;
use crate::engine::{
    event::bus::EventBus,
    packet::timeline::project_change_for_test,
    runtime::determinism::SkillTargetChoice,
    runtime::{change::BattleChange, executor::execute_rule_op_with},
};

#[test]
fn scylla_ultimate_moves_its_configured_random_target_after_hit() {
    crate::test_support::init_config();
    let combatant = |uid, team_type, position| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        position: Some(position),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            defense: Some(500),
            mdefense: Some(500),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                combatant(10, 1, 1),
                combatant(11, 1, 2),
                combatant(12, 1, 3),
                combatant(13, 1, 4),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![combatant(-1, 2, 1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_skill_target_choices([SkillTargetChoice {
        skill_id: 119_391_046,
        source_uid: -1,
        target_code: 206,
        targets: vec![13],
        additional_targets: Vec::new(),
        crit_targets: Vec::new(),
        additional_crit_targets: Vec::new(),
    }]);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: -1,
        skill_id: 119_391_046,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    let mut execution = SkillExecution::new(TargetContext::default());

    let command = loop {
        let phase = invocation.phase.unwrap_or(SkillPhase::Immediate);
        let emission = emit_ops(
            invocation,
            &managers,
            &pool,
            &catalog,
            &mut determinism,
            &mut execution,
            &SkillOpTrigger::Active,
        )
        .unwrap();
        if let Some(command) = emission
            .ops
            .into_iter()
            .find_map(|emission| match emission.op {
                RuleOp::Command(BattleCommand::MoveFront(command)) => Some(command),
                _ => None,
            })
        {
            assert_eq!(phase, SkillPhase::AfterHit);
            break command;
        }
        invocation = emission
            .continuation
            .expect("configured ultimate reaches its after-hit movement");
    };

    assert_eq!(command.target_uid, 13);
    assert_eq!(command.distance, 2);
    let outcome = execute_rule_op_with(
        &mut managers,
        &mut EventBus::default(),
        &mut determinism,
        RuleOp::Command(BattleCommand::MoveFront(command)),
    )
    .unwrap();
    let committed = outcome.changes();
    let [change] = committed.as_slice() else {
        panic!("movement produces one committed change")
    };
    let BattleChange::MoveFront(changes) = change else {
        panic!("movement keeps its semantic change")
    };

    assert_eq!(managers.entity.snapshot(10).unwrap().position, Some(1));
    assert_eq!(managers.entity.snapshot(11).unwrap().position, Some(3));
    assert_eq!(managers.entity.snapshot(12).unwrap().position, Some(4));
    assert_eq!(managers.entity.snapshot(13).unwrap().position, Some(2));
    let effects = project_change_for_test(change).unwrap();
    assert_eq!(effects[0].target_id, Some(13));
    assert_eq!(effects[0].effect_type, Some(EffectType::Movefront as i32));
    assert_eq!(effects[0].effect_num, Some(2));
    assert_eq!(effects[0].config_effect, Some(60061));
    assert_eq!(effects[0].reserve_str.as_deref(), Some("12#4|11#3"));
    assert_eq!(changes.target.before, 4);
}
