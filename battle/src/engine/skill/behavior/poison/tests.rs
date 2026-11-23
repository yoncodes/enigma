use sonettobuf::{Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::{
    manager::BattleManagers,
    runtime::determinism::RoundDeterminism,
    skill::{action::SkillModifiers, target::TargetContext},
};

#[test]
fn catapult_uses_configured_instances_and_stops_without_an_alternate_target() {
    crate::test_support::init_config();
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
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();
    let behavior = ParsedBehavior::new(60074, "CatapultBuff", vec![1, 2, 2, 30980111, 1, 2]);

    let ops = catapult_ops(
        &mut BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 1,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    assert_eq!(ops.len(), 3);
    assert!(matches!(
        &ops[0],
        RuleOp::Command(BattleCommand::Buff(BuffCommand::ReserveChildUids(
            BuffChildUidReservation {
                target_uid: -1,
                count: 1,
                ..
            }
        )))
    ));
    let mut events = crate::engine::event::bus::EventBus::default();
    for op in ops {
        crate::engine::runtime::executor::execute_rule_op(&mut managers, &mut events, op).unwrap();
    }
    assert_eq!(
        managers.buff.buff_act_amount(
            -1,
            crate::engine::skill::buff_act::registry::BuffActKind::Poison
        ),
        2
    );

    let conversion = ParsedBehavior::new(60110, "PoisonConvertToTargetBuff", vec![1, 31040013]);
    let ops = convert_poison_ops(
        &BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 2,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &conversion,
    )
    .unwrap();
    for op in ops {
        crate::engine::runtime::executor::execute_rule_op(&mut managers, &mut events, op).unwrap();
    }
    assert_eq!(
        managers.buff.buff_act_amount(
            -1,
            crate::engine::skill::buff_act::registry::BuffActKind::Poison
        ),
        1
    );
    assert_eq!(managers.buff.buff_id_amount(-1, 31040013), 1);
}

#[test]
fn powerful_poison_conversion_removes_distinct_instances_before_aggregate_grant() {
    crate::test_support::init_config();
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
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();
    let poison = ParsedBehavior::new(60074, "CatapultBuff", vec![1, 2, 3, 30980111, 1, 0]);
    let mut context = BehaviorOpContext {
        source_uid: 10,
        source_team: 1,
        target_uid: -1,
        active_skill_id: 1,
        transfer_count: 1,
        event: None,
        managers: &managers,
        pool: &pool,
        determinism: &mut determinism,
        modifiers: &mut modifiers,
        target: &mut target,
    };
    let mut events = crate::engine::event::bus::EventBus::default();
    for op in catapult_ops(&mut context, &poison).unwrap() {
        crate::engine::runtime::executor::execute_rule_op(&mut managers, &mut events, op).unwrap();
    }
    let mut poison_uids = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| {
            feature.owner_uid == -1
                && crate::engine::skill::buff_act::is_kind(
                    feature,
                    crate::engine::skill::buff_act::registry::BuffActKind::Poison,
                )
        })
        .map(|feature| feature.buff_uid)
        .collect::<Vec<_>>();
    poison_uids.sort_unstable();

    let conversion = ParsedBehavior::new(
        60284,
        "PoisonConvertToPowerfulPoisonBuff",
        vec![6, 31420003],
    );
    let ops = super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 2,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &conversion,
    )
    .unwrap();

    assert_eq!(ops.len(), 4);
    for (op, expected_uid) in ops[..3].iter().zip(&poison_uids) {
        assert!(matches!(
            op,
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
                target_uid: -1,
                selector: BuffRemoveSelector::Uid(uid),
                ..
            }))) if uid == expected_uid
        ));
    }
    assert!(matches!(
        &ops[3],
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
            target_uid: -1,
            buff_id: 31420003,
            amount: Some(3),
            occurrences: 1,
            ..
        })))
    ));
    events = crate::engine::event::bus::EventBus::default();
    for op in ops {
        crate::engine::runtime::executor::execute_rule_op(&mut managers, &mut events, op).unwrap();
    }
    for poison_uid in poison_uids {
        assert!(managers.buff.snapshot(-1, poison_uid).is_none());
    }
    assert_eq!(managers.buff.buff_id_amount(-1, 31420003), 3);
    let output_uid = managers.buff.buff_id_uid(-1, 31420003).unwrap();
    assert_eq!(
        managers.buff.snapshot(-1, output_uid).unwrap().duration,
        Some(2)
    );
}
