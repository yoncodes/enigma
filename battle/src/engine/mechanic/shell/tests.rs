use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::skill::rule::{ConfiguredRuleKey, DefinitionKey, RuleDomain};

const ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::Behavior,
    key: DefinitionKey::new(60135, "ShellUseSkill"),
};
const RULE: ConfiguredRuleKey = ConfiguredRuleKey::new(31090114, 1, ORIGIN.key);

#[test]
fn changes_while_the_triggered_cast_waits_do_not_count() {
    let mut managers = BattleManagers::default();
    let casts_after = |managers: &mut BattleManagers, rule, changes: i32| {
        (0..changes)
            .map(|_| {
                execute(
                    managers,
                    ShellCommand::AccumulateAndUseSkill {
                        rule,
                        source_uid: 10,
                        target_uid: -1,
                        threshold: 7,
                        delta: 1,
                        skill_id: 31090114,
                    },
                )
                .unwrap()
                .skills
                .len()
            })
            .sum::<usize>()
    };

    // "After accumulating a total of 7 deployments and or retrievals, triggers" one cast.
    assert_eq!(casts_after(&mut managers, RULE, 7), 1);
    assert_eq!(casts_after(&mut managers, RULE, 7), 0);
    managers.release_held_rule_progress(10, RULE);
    assert_eq!(casts_after(&mut managers, RULE, 6), 0);
    assert_eq!(casts_after(&mut managers, RULE, 1), 1);
}

#[test]
fn releasing_a_cast_resumes_only_its_exact_rule() {
    let mut managers = BattleManagers::default();
    let other = ConfiguredRuleKey::new(RULE.effect_id, 2, RULE.definition);
    let trigger = |managers: &mut BattleManagers, rule| {
        (0..7).fold(0, |casts, _| {
            casts
                + execute(
                    managers,
                    ShellCommand::AccumulateAndUseSkill {
                        rule,
                        source_uid: 10,
                        target_uid: -1,
                        threshold: 7,
                        delta: 1,
                        skill_id: 31090114,
                    },
                )
                .unwrap()
                .skills
                .len()
        })
    };

    assert_eq!(trigger(&mut managers, RULE), 1);
    assert_eq!(trigger(&mut managers, other), 1);
    managers.release_held_rule_progress(10, RULE);

    assert_eq!(trigger(&mut managers, RULE), 1);
    assert_eq!(trigger(&mut managers, other), 0);
}

#[test]
fn deploy_after_the_stock_is_spent_deploys_nothing() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                team_type: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                team_type: Some(2),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);

    let changes = execute(
        &mut managers,
        ShellCommand::Deploy {
            origin: ORIGIN,
            source_uid: 10,
            target_uid: -1,
            stock_buff_id: 31090117,
            amount: 1,
        },
    )
    .unwrap();

    assert!(changes.buffs.is_empty() && changes.events.is_empty());
    assert_eq!(managers.buff.buff_id_amount(-1, 31090118), 0);
}

#[test]
fn negative_deploy_amount_moves_all_stock_and_removes_the_spent_stock() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                team_type: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(52),
                    buff_id: Some(31090117),
                    layer: Some(8),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                team_type: Some(2),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);

    let changes = execute(
        &mut managers,
        ShellCommand::Deploy {
            origin: ORIGIN,
            source_uid: 10,
            target_uid: -1,
            stock_buff_id: 31090117,
            amount: -1,
        },
    )
    .unwrap();

    assert_eq!(managers.buff.buff_id_amount(10, 31090117), 0);
    assert!(!managers.buff.has_buff_id(10, 31090117));
    assert_eq!(managers.buff.buff_id_amount(-1, 31090118), 8);
    assert_eq!(changes.buffs[0].change.removed[0].buff.layer, Some(0));
    assert_eq!(changes.events[0].amount, 8);
    // With the stock gone, the caster's shells are still found through the deployed ones.
    assert_eq!(
        crate::engine::skill::buff_act::shell::caster_shell_spec(&managers, 10)
            .map(|spec| spec.stock_buff_id),
        Some(31090117)
    );
}

#[test]
fn retrieve_all_returns_every_deployed_stack_to_the_configured_stock() {
    crate::test_support::init_config();
    let entity = |uid, team_type, buff_id, buff_uid, layer| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100),
        team_type: Some(team_type),
        buffs: vec![BuffInfo {
            uid: Some(buff_uid),
            buff_id: Some(buff_id),
            layer: Some(layer),
            ..Default::default()
        }],
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 31090111, 52, 8)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                entity(-1, 2, 31090112, 100_001, 3),
                entity(-2, 2, 31090112, 100_002, 2),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);

    let changes = execute(
        &mut managers,
        ShellCommand::RetrieveAll {
            origin: ORIGIN,
            source_uid: 10,
            stock_buff_id: 31090111,
        },
    )
    .unwrap();

    assert_eq!(managers.buff.buff_id_amount(-1, 31090112), 0);
    assert_eq!(managers.buff.buff_id_amount(-2, 31090112), 0);
    assert_eq!(managers.buff.buff_id_amount(10, 31090111), 13);
    assert_eq!(changes.buffs[0].change.removed[0].buff.layer, Some(0));
    assert_eq!(changes.buffs[2].change.removed[0].buff.layer, Some(0));
    assert_eq!(changes.buffs.len(), 4);
    assert_eq!(changes.events.len(), 2);
    assert_eq!(changes.events[0].kind, ShellChangeKind::Retrieved);
    assert_eq!(changes.events[0].target_uid, -2);
    assert_eq!(changes.events[0].amount, 2);
    assert_eq!(changes.events[1].target_uid, -1);
    assert_eq!(changes.events[1].amount, 3);
}

#[test]
fn retrieval_regrants_stock_from_the_carrier() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                team_type: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                team_type: Some(2),
                buffs: vec![BuffInfo {
                    uid: Some(52),
                    buff_id: Some(31090112),
                    from_uid: Some(10),
                    layer: Some(3),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);

    execute(
        &mut managers,
        ShellCommand::Retrieve {
            origin: ORIGIN,
            source_uid: 10,
            target_uid: -1,
            stock_buff_id: 31090111,
            amount: 1,
        },
    )
    .unwrap();

    let stock_uid = managers.buff.buff_id_uid(10, 31090111).unwrap();
    assert_eq!(
        managers.buff.snapshot(10, stock_uid).unwrap().from_uid,
        Some(-1)
    );
}

#[test]
fn retrieve_all_without_deployed_shells_is_a_no_op() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(52),
                    buff_id: Some(31090111),
                    layer: Some(8),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);

    let changes = execute(
        &mut managers,
        ShellCommand::RetrieveAll {
            origin: ORIGIN,
            source_uid: 10,
            stock_buff_id: 31090111,
        },
    )
    .unwrap();

    assert_eq!(managers.buff.buff_id_amount(10, 31090111), 8);
    assert!(changes.buffs.is_empty());
    assert!(changes.events.is_empty());
}

#[test]
fn configured_progress_emits_one_skill_per_completed_threshold() {
    let mut managers = BattleManagers::default();
    let command = |delta| ShellCommand::AccumulateAndUseSkill {
        rule: RULE,
        source_uid: 10,
        target_uid: -1,
        threshold: 5,
        delta,
        skill_id: 31090174,
    };

    assert!(
        execute(&mut managers, command(3))
            .unwrap()
            .skills
            .is_empty()
    );
    let changes = execute(&mut managers, command(4)).unwrap();

    assert_eq!(changes.skills.len(), 1);
    assert_eq!(changes.skills[0].plan.skill_id, 31090174);
    assert_eq!(
        changes.skills[0].target,
        crate::engine::skill::action::SkillTarget::Explicit(-1)
    );
}
