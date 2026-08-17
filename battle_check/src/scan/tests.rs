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

    scan_closure(
        db,
        battle::catalog::BattleCatalog::new(db),
        &mut catalog,
        &mut skills,
        &mut buffs,
        &mut report,
    );

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
        battle::catalog::BattleCatalog::new(db),
        &mut catalog,
        &mut skills,
        &mut VecDeque::new(),
        &mut report,
    );

    assert!(report.checked_skills.contains(&929_010_774));
    assert!(report.checked_skills.contains(&929_010_741));
}

#[test]
fn count_continue_channel_expands_the_checked_skill_closure() {
    crate::init_config().unwrap();
    let db = config::get();
    let mut catalog = SkillEffectCatalog::default();
    let mut skills = VecDeque::new();
    let mut buffs = VecDeque::from([Pending {
        id: 31_000_133,
        path: "test root".to_owned(),
    }]);
    let mut report = Report {
        quiet: true,
        ..Default::default()
    };

    scan_closure(
        db,
        battle::catalog::BattleCatalog::new(db),
        &mut catalog,
        &mut skills,
        &mut buffs,
        &mut report,
    );

    assert!(report.checked_skills.contains(&31_000_193));
}

#[test]
fn buff_owned_charge_reports_manual_activation_gap_and_linked_skill() {
    crate::init_config().unwrap();
    let db = config::get();
    let mut catalog = SkillEffectCatalog::default();
    let mut skills = VecDeque::new();
    let mut buffs =
        VecDeque::from(
            [115_370_004, 31_460_141, 31_460_142, 31_460_143].map(|id| Pending {
                id,
                path: "test root".to_owned(),
            }),
        );
    let mut report = Report {
        quiet: true,
        ..Default::default()
    };

    scan_closure(
        db,
        battle::catalog::BattleCatalog::new(db),
        &mut catalog,
        &mut skills,
        &mut buffs,
        &mut report,
    );

    let key = CapabilityKey::new("buff-act", 1139, "MeiLeiErCharge");
    assert!(report.capabilities.contains(&key));
    assert!(report.gaps[&key].contains("manual activation is not proven"));
    for skill_id in [30_110_131, 31_460_181, 31_460_182, 31_460_183] {
        assert!(report.checked_skills.contains(&skill_id));
    }
}

#[test]
fn bendith_replacement_expands_every_reachable_skill() {
    crate::init_config().unwrap();
    let db = config::get();
    let mut catalog = SkillEffectCatalog::default();
    let mut skills = VecDeque::new();
    let mut buffs = VecDeque::from([Pending {
        id: 31_460_140,
        path: "test root".to_owned(),
    }]);
    let mut report = Report {
        quiet: true,
        ..Default::default()
    };

    scan_closure(
        db,
        battle::catalog::BattleCatalog::new(db),
        &mut catalog,
        &mut skills,
        &mut buffs,
        &mut report,
    );

    for skill_id in [
        31_460_241, 31_460_242, 31_460_243, 31_460_233, 31_460_234, 31_460_235,
    ] {
        assert!(report.checked_skills.contains(&skill_id), "{skill_id}");
    }
}

#[test]
fn bendith_mapping_without_a_unique_inverse_is_a_capability_gap() {
    crate::init_config().unwrap();
    let db = config::get();
    let mut catalog = SkillEffectCatalog::default();
    let mut skills = VecDeque::new();
    let mut buffs = VecDeque::from([Pending {
        id: 115_370_003,
        path: "test root".to_owned(),
    }]);
    let mut report = Report {
        quiet: true,
        ..Default::default()
    };

    scan_closure(
        db,
        battle::catalog::BattleCatalog::new(db),
        &mut catalog,
        &mut skills,
        &mut buffs,
        &mut report,
    );

    assert!(report.gaps.contains_key(&CapabilityKey::new(
        "buff-act",
        1138,
        "ReplaceEntitySkillGroup"
    )));
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
        battle::catalog::BattleCatalog::new(db),
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
