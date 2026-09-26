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
fn summoned_definitions_expand_the_checked_skill_closure() {
    crate::init_config().unwrap();
    let db = config::get();
    let mut catalog = SkillEffectCatalog::default();
    let mut skills = VecDeque::from([Pending {
        id: 307_401_612,
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
        &mut VecDeque::new(),
        &mut report,
    );

    assert!(report.checked_skills.contains(&307_401_711));
    assert!(report.checked_skills.contains(&307_401_721));
}

#[test]
fn missing_summoned_definition_fails_loudly() {
    crate::init_config().unwrap();
    let mut skills = VecDeque::new();
    let mut report = Report::default();

    enqueue_summoned_skills(
        config::get(),
        i32::MAX,
        "test root",
        &mut skills,
        &mut report,
    );

    assert!(skills.is_empty());
    assert_eq!(
        report.errors,
        ["MissingSummoned path=test root".to_owned()].into()
    );
}

#[test]
fn device_owned_max_roots_use_configured_device_skills() {
    crate::init_config().unwrap();
    let db = config::get();
    let mut skills = VecDeque::new();
    let mut report = Report {
        quiet: true,
        ..Default::default()
    };

    collect_hero_build_roots(3144, None, None, db, &mut skills, &mut report).unwrap();

    let ids = skills
        .into_iter()
        .map(|pending| pending.id)
        .collect::<Vec<_>>();
    assert!(ids.contains(&31444111));
    assert!(ids.contains(&31441121));
    assert!(ids.contains(&31445131));
    for false_id in [31440112, 31440113, 31440122, 31440123] {
        assert!(
            !ids.contains(&false_id),
            "unexpected character skill {false_id}"
        );
    }
}

#[test]
fn non_device_max_roots_keep_character_groups_and_ultimate() {
    crate::init_config().unwrap();
    let db = config::get();
    let mut skills = VecDeque::new();
    let mut report = Report {
        quiet: true,
        ..Default::default()
    };

    assert_eq!(
        battle::catalog::configured_conduit_device_id(db, 3134, 5, 0),
        None
    );
    collect_hero_build_roots(3134, None, None, db, &mut skills, &mut report).unwrap();

    let ids = skills
        .into_iter()
        .map(|pending| pending.id)
        .collect::<Vec<_>>();
    assert!(ids.contains(&31345111));
    assert!(ids.contains(&31344121));
    assert!(ids.contains(&31345131));
}

#[test]
fn choice_families_keep_primary_roots_and_scan_alternatives() {
    crate::init_config().unwrap();
    let db = config::get();
    let mut skills = VecDeque::new();
    let mut report = Report {
        quiet: true,
        explain: true,
        ..Default::default()
    };

    collect_hero_build_roots(3120, None, None, db, &mut skills, &mut report).unwrap();
    let root_ids = skills.iter().map(|pending| pending.id).collect::<Vec<_>>();
    assert!(root_ids.contains(&312001215));
    for child_id in [31200164, 31200231] {
        assert!(
            !root_ids.contains(&child_id),
            "choice child became a root: {child_id}"
        );
    }

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

    for child_id in [31200164, 31200231] {
        assert!(report.checked_skills.contains(&child_id));
        assert!(catalog.get(child_id).is_some());
    }
    assert!(report.explanations.iter().any(|explanation| {
        explanation.contains("Skill id=31200164")
            && explanation.contains("role=choice")
            && explanation.contains("choice primary 312001215 > alternative 31200164")
    }));
}

#[test]
fn paper_heron_choice_roots_keep_only_the_first_family() {
    crate::init_config().unwrap();
    let db = config::get();
    let mut skills = VecDeque::new();
    let mut report = Report {
        quiet: true,
        ..Default::default()
    };

    collect_hero_build_roots(3135, None, None, db, &mut skills, &mut report).unwrap();
    let root_ids = skills.iter().map(|pending| pending.id).collect::<Vec<_>>();
    for root_id in [313501171, 313501181, 313501191] {
        assert!(root_ids.contains(&root_id));
    }
    for child_id in [
        313501117, 313501127, 313501137, 313501118, 313501128, 313501138,
    ] {
        assert!(
            !root_ids.contains(&child_id),
            "choice child became a root: {child_id}"
        );
    }
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

#[test]
fn malformed_buff_act_arguments_fail_loudly_without_trailing_references() {
    crate::init_config().unwrap();
    let db = config::get();
    let feature = buff_act_registry::resolve_feature(Some(db), "865#bad#31460181").unwrap();

    assert!(feature.is_malformed());
    assert!(feature.values.is_empty());
    assert!(feature.references(Some(db)).skills.is_empty());
    assert_eq!(
        malformed_buff_act_error("test root", 1, 865, "AddPassiveSkills", &feature),
        Some(
            "MalformedBuffActArguments path=test root > buff 1 act=865 type=AddPassiveSkills reason=InvalidInteger { cell: 1, item: 0 } raw=\"865#bad#31460181\""
                .to_owned()
        )
    );
}
