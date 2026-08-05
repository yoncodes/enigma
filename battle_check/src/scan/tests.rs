use super::*;

#[test]
fn capability_gaps_are_not_ready() {
    let mut report = Report::default();
    report.gap(
        CapabilityKey::new("buff-act", 794, "ModifyMaxBurnLayers"),
        "unregistered buff act",
    );

    assert!(!report.is_ready());
}

#[test]
fn reachable_timed_buff_reports_a_missing_duration_route() {
    crate::init_config().unwrap();
    let db = config::get();
    let mut catalog = SkillEffectCatalog::default();
    let mut skills = VecDeque::new();
    let mut buffs = VecDeque::from([Pending {
        id: 630_091,
        path: "test root".to_owned(),
    }]);
    let mut report = Report {
        quiet: true,
        ..Default::default()
    };

    scan_closure(db, &mut catalog, &mut skills, &mut buffs, &mut report);

    assert!(
        report
            .gaps
            .contains_key(&CapabilityKey::new("effect-time", 209, "BuffDuration",))
    );
}

#[test]
fn gap_paths_preserve_exact_buff_provenance() {
    let mut report = Report::default();
    let key = CapabilityKey::new("buff-include", 7, "ValueBearingType7(7#10)");

    report.gap_at(
        key.clone(),
        "UnresolvedIncludePolicy",
        "episode 1 > skill 2 > slot 3 > buff 4".to_owned(),
    );

    assert_eq!(
        report.gap_paths[&key],
        ["episode 1 > skill 2 > slot 3 > buff 4".to_owned()].into()
    );
}

#[test]
fn transformed_models_expand_the_checked_skill_closure() {
    crate::init_config().unwrap();
    let db = config::get();
    let mut skills = VecDeque::new();
    let mut report = Report {
        quiet: true,
        ..Default::default()
    };

    collect_battle_roots(30_510_110, 9_290_107, db, &mut skills, &mut report).unwrap();
    let mut catalog = SkillEffectCatalog::from_roots(
        db,
        skills.iter().map(|pending| pending.id),
        std::iter::empty(),
    );
    scan_closure(
        db,
        &mut catalog,
        &mut skills,
        &mut VecDeque::new(),
        &mut report,
    );

    assert!(report.checked_skills.contains(&929_010_774));
    assert!(report.checked_skills.contains(&929_010_741));
}

#[test]
fn tower_assist_boss_forms_accept_the_implemented_group_capacity_policy() {
    crate::init_config().unwrap();
    let db = config::get();
    let mut skills = VecDeque::new();
    let mut report = Report {
        quiet: true,
        ..Default::default()
    };

    collect_tower_assist_boss_roots(6, db, &mut skills).unwrap();
    let mut catalog = SkillEffectCatalog::from_roots(
        db,
        skills.iter().map(|pending| pending.id),
        std::iter::empty(),
    );
    scan_closure(
        db,
        &mut catalog,
        &mut skills,
        &mut VecDeque::new(),
        &mut report,
    );

    assert!(report.checked_skills.contains(&13020011));
    assert!(report.checked_skills.contains(&13020012));
    assert!(!report.gaps.contains_key(&CapabilityKey::new(
        "buff-include",
        13,
        "GroupCapacity(13#5)"
    )));
}

#[test]
fn semantic_destination_is_independent_from_wire_metadata() {
    assert_eq!(buff_act_capability(None), None);
    assert_eq!(
        buff_act_capability(buff_act::registry::destination(100, "Attr", &[])),
        Some("transaction")
    );
    assert_eq!(
        buff_act_capability(buff_act::registry::destination(10000, "EzioBigSkill", &[],)),
        Some("state-consumer")
    );
}
