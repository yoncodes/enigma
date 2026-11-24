use super::*;
use crate::engine::{
    manager::ex_point::ExPointChanges, runtime::record::FrameItem,
    skill::effect::catalog::RuleIssueReason,
};

#[test]
fn team_moxie_grants_readiness_from_applied_payments() {
    crate::test_support::init_config();
    let entity = |uid, team_type, kind, points| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(100),
        ex_point_type: Some(kind),
        ex_point: Some(points),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    passive_skill: vec![312101412],
                    ..entity(10, 1, 0, 5)
                },
                entity(11, 1, 0, 3),
                entity(12, 1, 1, 5),
            ],
            sp_entitys: vec![entity(13, 1, 0, 5)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, 0, 5)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [312101412, 82800113], []);
    assert!(catalog.issues(312101412).is_empty());
    let [issue] = catalog.issues(82800113) else {
        panic!("expected one unsupported behavior");
    };
    assert_eq!(issue.opcode, Some(60203));
    assert_eq!(issue.reason, RuleIssueReason::UnsupportedBehavior);
    assert!(!issue.reason.is_blocking());

    let mut managers = BattleManagers::seeded(&fight);
    let debit = |uid, delta, spent| {
        let change = ExPointChange {
            origin: CommandOrigin {
                domain: RuleDomain::Lifecycle,
                key: DefinitionKey::new(0, "TestMoxieChange"),
            },
            source_uid: uid,
            target_uid: uid,
            delta,
            config_effect: 0,
            effect_type: 0,
        };
        RuleOp::Command(BattleCommand::ExPoint(if spent {
            ExPointCommand::Spend(change)
        } else {
            ExPointCommand::Change(change)
        }))
    };
    let result = run_command_group(
        &mut managers,
        &TargetPool::from_fight(&fight),
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 1,
            ..Default::default()
        },
        [
            debit(11, -3, true),
            debit(10, -3, false),
            debit(-1, -3, true),
            debit(12, -3, true),
            debit(13, -3, true),
            debit(10, -3, true),
        ],
    )
    .unwrap();
    let mut order = Vec::new();
    for frame in &result.frames {
        record_moxie_frames(frame, &mut order);
    }
    assert_eq!(
        order,
        [
            ("grant", 10),
            ("spend", 11),
            ("spend", -1),
            ("spend", 12),
            ("spend", 13),
            ("grant", 10),
            ("spend", 10)
        ],
    );
    let grants = result
        .events
        .iter()
        .filter_map(|event| match event {
            BattleEvent::BuffAdded(change) if change.buff_id == 31210125 => {
                Some((change.target_uid, change.after_amount))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(grants, vec![(10, 3)]);
    let readiness = managers
        .buff
        .active_for(10)
        .filter(|buff| buff.buff_id == Some(31210125))
        .collect::<Vec<_>>();
    assert_eq!(readiness.len(), 1);
    assert_eq!(readiness[0].layer, Some(5));
    assert_eq!(readiness[0].duration, Some(3));
}

fn record_moxie_frames(frame: &SemanticFrame, order: &mut Vec<(&'static str, i64)>) {
    for item in &frame.items {
        match item {
            FrameItem::Child(child) => {
                if let FrameOwner::Skill {
                    skill_id: 312101412,
                    target_uid: Some(uid),
                    ..
                } = child.owner
                    && !child.items.is_empty()
                {
                    order.push(("grant", uid));
                }
                record_moxie_frames(child, order);
            }
            FrameItem::Change(change) => {
                if let BattleChange::ExPoint(ExPointChanges::Value { change, .. }) = change.as_ref()
                    && change.spent
                {
                    order.push(("spend", change.target_uid));
                }
            }
            _ => (),
        }
    }
}
