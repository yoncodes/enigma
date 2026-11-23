use battle::tooling::scan::parse_conditions;

use super::*;

#[test]
fn queue_rank_rules_have_reachable_routes() {
    crate::init_config().unwrap();
    let db = config::get();
    for skill_id in [31390141, 30990151] {
        let mut report = Report::default();
        scan_closure(
            db,
            battle::catalog::BattleCatalog::new(db),
            &mut SkillEffectCatalog::default(),
            &mut VecDeque::from([Pending {
                id: skill_id,
                path: "test root".to_owned(),
            }]),
            &mut VecDeque::new(),
            &mut report,
        );

        assert!(report.is_ready(), "{:?}", report.errors);
    }
}

#[test]
fn grouped_queue_preparation_requires_every_alternative_to_need_the_event() {
    crate::init_config().unwrap();
    let db = config::get();
    let catalog = SkillEffectCatalog::from_roots(db, [30990151], []);
    let mut slot = catalog.get(30990151).unwrap().slots[0].clone();
    for (conditions, has_gap) in [("1061|1061", true), ("1061|210", false), ("1061!", false)] {
        slot.conditions = parse_conditions(db, conditions);
        slot.compiled_route =
            ConditionRoute::compile_for_behavior(&slot.conditions, &slot.behavior.spec);
        assert_eq!(
            behavior::missing_queue_preparation_route(&slot),
            has_gap,
            "{conditions}",
        );
    }
}

#[test]
fn closure_reports_raw_slots_after_blank_slots() {
    crate::init_config().unwrap();
    let db = config::get();
    let mut report = Report {
        explain: true,
        ..Default::default()
    };
    scan_closure(
        db,
        battle::catalog::BattleCatalog::new(db),
        &mut SkillEffectCatalog::default(),
        &mut VecDeque::from([Pending {
            id: 23390242,
            path: "test root".to_owned(),
        }]),
        &mut VecDeque::new(),
        &mut report,
    );

    assert!(report.explanations.iter().any(|entry| {
        entry.starts_with("Slot skill=23390242 slot=3 ") && entry.contains("behavior=1:AddBuff")
    }));
    assert!(
        report
            .explanations
            .iter()
            .any(|entry| { entry.starts_with("Condition skill=23390242 slot=3 opcode=208 ") })
    );
    assert!(report.errors.iter().any(|entry| {
        entry.contains(" > slot 3 > buff 23390031") && entry.contains("act=811 type=Dream")
    }));
}
