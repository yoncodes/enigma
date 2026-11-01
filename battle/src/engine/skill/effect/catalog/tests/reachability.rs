use super::*;

#[test]
fn audit_reported_team_reachable_effects() {
    init_config();
    let game = config::configs::get();
    let mut failures = Vec::new();
    fn unsupported(kind: &ParsedConditionKind) -> bool {
        match kind {
            ParsedConditionKind::Unsupported(_) => true,
            ParsedConditionKind::Any(groups) => groups.iter().flatten().any(|condition| unsupported(&condition.kind)),
            ParsedConditionKind::Not(inner) => unsupported(inner),
            _ => false,
        }
    }
    for hero_id in [3120, 3088, 3126, 3125] {
        let roots = game.skill.iter().filter(|row| row.hero_id == hero_id)
            .map(|row| row.skill_effect)
            .chain(game.skill_passive_level.iter().filter(|row| row.hero_id == hero_id)
                .map(|row| row.skill_passive))
            .filter(|id| *id > 0).collect::<Vec<_>>();
        assert!(!roots.is_empty(), "missing skill roots for hero {hero_id}");
        let catalog = SkillEffectCatalog::from_roots(game, roots, []);
        let mut ids = catalog.effects.keys().copied().collect::<Vec<_>>();
        ids.sort_unstable();
        println!("hero {hero_id}: {} reachable effects", ids.len());
        for id in ids {
            let issues = catalog.issues(id);
            if !issues.is_empty() {
                failures.push(format!("hero {hero_id}, effect {id}: {issues:?}"));
            }
            let effect = catalog.get(id).unwrap();
            for (index, slot) in effect.slots.iter().enumerate() {
                if crate::engine::skill::behavior::registry::find(&slot.behavior).is_none()
                    || !crate::engine::skill::behavior::is_supported(&slot.behavior)
                    || !crate::engine::skill::behavior::is_slot_supported(slot)
                    || !crate::engine::skill::behavior::has_destination(&slot.behavior)
                    || slot.compiled_route.is_err()
                    || slot.conditions.iter().any(|condition| unsupported(&condition.kind))
                {
                    failures.push(format!("hero {hero_id}, effect {id}, slot {}: behavior {:?}, route {:?}",
                        index + 1, slot.behavior, slot.compiled_route));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn audit_reported_team_reachable_buff_acts() {
    init_config();
    let game = config::configs::get();
    let mut failures = Vec::new();
    for hero_id in [3120, 3088, 3126, 3125] {
        let roots = game.skill.iter().filter(|row| row.hero_id == hero_id)
            .map(|row| row.skill_effect)
            .chain(game.skill_passive_level.iter().filter(|row| row.hero_id == hero_id)
                .map(|row| row.skill_passive))
            .filter(|id| *id > 0).collect::<Vec<_>>();
        let catalog = SkillEffectCatalog::from_roots(game, roots, []);
        println!("hero {hero_id}: {} reachable buffs", catalog.reachable_buffs.len());
        for &buff_id in &catalog.reachable_buffs {
            let Some(buff) = game.skill_buff.get(buff_id) else { continue; };
            for raw in buff.features.split('|').map(str::trim).filter(|raw| !raw.is_empty()) {
                let Some(feature) = crate::engine::skill::buff_act::registry::resolve_feature(Some(game), raw) else {
                    failures.push(format!("hero {hero_id}, buff {buff_id}: malformed feature {raw}"));
                    continue;
                };
                let Some(act_id) = feature.act_id else {
                    failures.push(format!("hero {hero_id}, buff {buff_id}: missing feature ID {raw}"));
                    continue;
                };
                let Some(act) = game.buff_act.get(act_id) else {
                    if game.skill_buff.get(act_id).is_none() {
                        failures.push(format!("hero {hero_id}, buff {buff_id}: missing feature {act_id}"));
                    }
                    continue;
                };
                if crate::engine::skill::buff_act::registry::find(act.id, &act.r#type).is_none()
                    || feature.is_malformed()
                    || crate::engine::skill::buff_act::registry::destination_for_feature(Some(game), &feature).is_none()
                {
                    failures.push(format!("hero {hero_id}, buff {buff_id}: feature {} {} ({raw})",
                        act.id, act.r#type));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn audit_configured_effects_for_selected_heroes() {
    init_config();
    let game = config::configs::get();
    let mut failures = Vec::new();
    for hero_id in [3086, 3143, 3128, 3142, 3104, 3098, 3127] {
        let roots = game.skill.iter().filter(|row| row.hero_id == hero_id)
            .map(|row| row.skill_effect)
            .chain(game.skill_passive_level.iter().filter(|row| row.hero_id == hero_id)
                .map(|row| row.skill_passive))
            .filter(|id| *id > 0).collect::<Vec<_>>();
        assert!(!roots.is_empty());
        let catalog = SkillEffectCatalog::from_roots(game, roots, []);
        let mut ids = catalog.effects.keys().copied().collect::<Vec<_>>();
        ids.sort_unstable();
        println!("hero {hero_id}: {} reachable effects", ids.len());
        for id in ids {
            let issues = catalog.issues(id);
            if !issues.is_empty() {
                failures.push(format!("hero {hero_id}, effect {id}: {issues:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn configured_behaviors_keep_their_effect_and_slot_identity() {
    init_config();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let mut shell_rules = 0;

    for (&effect_id, effect) in &catalog.effects {
        for (index, slot) in effect.slots.iter().enumerate() {
            if slot.behavior.spec.kind
                != crate::engine::skill::behavior::classify::BehaviorKind::ShellUseSkill
            {
                continue;
            }
            shell_rules += 1;
            assert_eq!(slot.behavior.effect_id, effect_id);
            assert_eq!(slot.behavior.slot, index as u8 + 1);
        }
    }

    assert!(shell_rules > 0);
}

#[test]
fn fight_catalog_excludes_unrelated_heroes() {
    init_config();
    let fight = Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                passive_skill: vec![31270148],
                skill_group1: vec![31270111],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };

    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    assert!(catalog.get(31270148).is_some());
    assert!(catalog.get(31270111).is_some());
    assert!(catalog.get(31430141).is_none());
}

#[test]
fn interval_skills_are_identified_by_their_stop_behavior() {
    init_config();
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31450111, 31450121], []);

    assert!(catalog.is_interval(31450111));
    assert!(!catalog.is_interval(31450121));
}

#[test]
fn fight_catalog_includes_assist_boss_active_skills() {
    init_config();
    let fight = Fight {
        attacker: Some(sonettobuf::FightTeam {
            assist_boss_info: Some(sonettobuf::AssistBossInfo {
                skills: vec![sonettobuf::AssistBossSkillInfo {
                    skill_id: Some(116331205),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    assert!(catalog.get(116331205).is_some());
}

#[test]
fn fight_catalog_compiles_enigmas_round_start_field_condition() {
    init_config();
    let fight = Fight {
        defender: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                passive_skill: vec![31430149],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };

    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let effect = catalog.get(31430149).unwrap();

    assert!(catalog.issues(31430149).is_empty());
    assert!(effect.slots.iter().all(|slot| slot.compiled_route.is_ok()));
}

#[test]
fn dynamic_current_battle_roots_compile_enigmas_field_condition() {
    init_config();
    let mut catalog = SkillEffectCatalog::default();

    catalog.extend_roots_and_warn(config::configs::get(), [31430149], []);

    assert!(catalog.issues(31430149).is_empty());
    assert!(
        catalog
            .get(31430149)
            .unwrap()
            .slots
            .iter()
            .all(|slot| slot.compiled_route.is_ok())
    );
    assert!(catalog.get(31430149).is_some());
    assert!(catalog.get(31270148).is_none());
}

#[test]
fn add_summoned_expands_configured_unique_skills() {
    init_config();
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [307401612], []);

    assert!(catalog.get(307401612).is_some());
    assert!(catalog.get(307401711).is_some());
    assert!(catalog.get(307401721).is_some());
}

#[test]
fn eagle_exit_cleanup_compiles_its_exact_buff_family_dispel() {
    init_config();
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [30060141], []);
    let effect = catalog.get(30060141).unwrap();

    assert!(catalog.issues(30060141).is_empty());
    let cleanup = effect
        .slots
        .iter()
        .filter(|slot| {
            slot.behavior.spec.key.opcode == 90002
                && slot.behavior.spec.key.type_name == "Disperse2"
        })
        .collect::<Vec<_>>();
    assert_eq!(cleanup.len(), 2);
    assert!(cleanup.iter().all(|slot| slot.compiled_route.is_ok()));
    assert!(cleanup.iter().all(|slot| slot.target.code == 202));
}

#[test]
fn exact_enemy_damage_routes_compile_without_runtime_gaps() {
    init_config();
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [260341, 750331], [5112]);

    for skill_id in [260341, 750331] {
        let effect = catalog.get(skill_id).unwrap();
        assert!(catalog.issues(skill_id).is_empty());
        assert!(effect.slots.iter().all(|slot| slot.compiled_route.is_ok()));
        assert!(
            effect
                .slots
                .iter()
                .all(|slot| crate::engine::skill::behavior::has_destination(&slot.behavior))
        );
    }
}

#[test]
fn joe_missing_hp_rate_condition_compiles_as_an_active_modifier() {
    init_config();
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [30940171], []);
    let effect = catalog.get(30940171).unwrap();

    assert!(catalog.issues(30940171).is_empty());
    assert!(effect.slots.iter().all(|slot| slot.compiled_route.is_ok()));
    assert_eq!(effect.slots[0].conditions[0].opcode, 623203);
    assert!(crate::engine::skill::behavior::has_destination(
        &effect.slots[0].behavior
    ));
}

#[test]
fn missing_hp_attack_and_defense_modifiers_compile_as_separate_exact_lanes() {
    init_config();
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [342440140], []);
    let effect = catalog.get(342440140).unwrap();

    assert!(catalog.issues(342440140).is_empty());
    assert!(effect.slots.iter().all(|slot| slot.compiled_route.is_ok()));
    assert_eq!(effect.slots[0].conditions[0].opcode, 623204);
    assert_eq!(effect.slots[1].conditions[0].opcode, 623203);
    assert!(
        effect
            .slots
            .iter()
            .all(|slot| crate::engine::skill::behavior::has_destination(&slot.behavior))
    );
}

#[test]
fn anjo_negative_status_rate_condition_compiles_as_an_active_modifier() {
    init_config();
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31000441], []);
    let effect = catalog.get(31000441).unwrap();

    assert!(catalog.issues(31000441).is_empty());
    assert!(effect.slots.iter().all(|slot| slot.compiled_route.is_ok()));
    assert_eq!(effect.slots[1].conditions[0].opcode, 539203);
    assert!(crate::engine::skill::behavior::has_destination(
        &effect.slots[1].behavior
    ));
}

#[test]
fn kaalaa_baunaa_planet_removal_compiles_through_its_exact_behavior() {
    init_config();
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [307001333], []);
    let effect = catalog.get(307001333).unwrap();

    assert!(catalog.issues(307001333).is_empty());
    assert!(effect.slots.iter().all(|slot| slot.compiled_route.is_ok()));
    assert_eq!(effect.slots[1].behavior.spec.key.opcode, 60252);
    assert_eq!(
        effect.slots[1].behavior.spec.kind,
        crate::engine::skill::behavior::classify::BehaviorKind::DisperseForce3
    );
    assert!(crate::engine::skill::behavior::has_destination(
        &effect.slots[1].behavior
    ));
}

#[test]
fn entering_entities_extend_the_scoped_catalog_from_their_own_roots() {
    init_config();
    let mut catalog = SkillEffectCatalog::default();
    let entrant = sonettobuf::FightEntityInfo {
        passive_skill: vec![2531, 2370],
        ..Default::default()
    };

    catalog.extend_entities_and_warn(config::configs::get(), [&entrant]);

    assert!(catalog.get(2531).is_some());
    assert!(catalog.get(2370).is_some());
    assert!(catalog.get(31270148).is_none());
}

#[test]
fn fight_catalog_follows_heat_scale_cast_skills() {
    init_config();

    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [], [31340021]);

    assert!(catalog.get(31345153).is_some());
}

#[test]
fn fight_catalog_follows_master_halo_linked_passives() {
    init_config();

    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31260143], []);

    assert!(catalog.get(31260183).is_some());
}

#[test]
fn fight_catalog_follows_paper_circle_continuation_skill() {
    init_config();

    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [], [31050131]);

    assert!(catalog.get(31050151).is_some());
}

#[test]
fn fight_catalog_follows_count_continue_channel_skill() {
    init_config();

    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [], [31000133]);

    assert!(catalog.get(31000193).is_some());
}

#[test]
fn fight_catalog_follows_buff_act_linked_skill_aliases() {
    init_config();

    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [], [31260131]);

    assert!(catalog.get(31260171).is_some());
    assert_eq!(catalog.damage_rate(31260171), 1300);

    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [], [31200184]);
    assert!(catalog.get(31200192).is_some());
}

#[test]
fn fight_catalog_follows_shield_counter_skill() {
    init_config();

    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [], [30940181]);

    assert!(catalog.get(30940171).is_some());
}

#[test]
fn scoped_catalog_follows_configured_reinforced_skill_effects() {
    init_config();

    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [30860143], []);

    assert_eq!(catalog.reinforced_skill(30860143), Some(30861143));
    assert!(catalog.get(30861143).is_some());
    assert!(catalog.issues(30861143).is_empty());
}

#[test]
fn scoped_catalog_follows_buff_replacement_passive_links() {
    init_config();

    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [], [116385669]);

    assert!(catalog.reachable_buffs.contains(&116385670));
    assert!(catalog.get(116385685).is_some());
}

#[test]
fn scoped_catalog_follows_buff_owned_charge_linked_skill() {
    init_config();
    for (buff_id, skill_id) in [
        (115370004, 30110131),
        (31460141, 31460181),
        (31460142, 31460182),
        (31460143, 31460183),
    ] {
        let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [], [buff_id]);
        assert!(catalog.reachable_buffs.contains(&buff_id));
        assert!(catalog.get(skill_id).is_some());
    }
}

#[test]
fn scoped_catalog_follows_paper_heron_emitter_replacements() {
    init_config();
    for (buff_id, skill_id) in [(31410402, 31410001), (31411402, 31411001)] {
        let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [], [buff_id]);
        assert!(catalog.get(skill_id).is_some());
        assert!(catalog.issues(skill_id).is_empty(), "replacement skill has unresolved behaviors");
    }
}

#[test]
fn scoped_catalog_follows_configured_hero_upgrade_outcomes() {
    init_config();
    let fight = Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                passive_skill: vec![30860151],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };

    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    assert!(catalog.get(30861111).is_some());
    assert!(catalog.get(30861121).is_some());
    assert!(catalog.get(30861131).is_some());
    assert!(catalog.get(30861171).is_some());
    assert!(catalog.issues(30861171).is_empty());
    assert!(catalog.compiled_subscription_lanes(30861171).is_ok());
    assert!(catalog.reachable_buffs.contains(&30860173));
}


#[test]
fn audit_an_an_lee_reachable_effects() {
    init_config();
    let game = config::configs::get();
    let roots = game.skill.iter().filter(|row| row.hero_id == 3039)
        .map(|row| row.skill_effect)
        .chain(game.skill_passive_level.iter().filter(|row| row.hero_id == 3039)
            .map(|row| row.skill_passive))
        .filter(|id| *id > 0).collect::<Vec<_>>();
    let catalog = SkillEffectCatalog::from_roots(game, roots, []);
    let mut ids = catalog.effects.keys().copied().collect::<Vec<_>>();
    ids.sort_unstable();
    let issues = ids.iter().flat_map(|id| catalog.issues(*id).iter().cloned()).collect::<Vec<_>>();
    println!("An-an Lee reachable effects: {}", ids.len());
    assert!(issues.is_empty(), "An-an Lee unsupported effects: {issues:#?}");
}
