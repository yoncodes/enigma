use super::*;

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
