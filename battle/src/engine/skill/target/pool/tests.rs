use super::*;
use sonettobuf::{BuffInfo, FightTeam, HeroAttribute};

#[test]
fn target_buff_metadata_uses_configured_type_and_exact_feature_identities() {
    crate::test_support::init_config();
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let buff = TargetBuff::from_buff_info(
        catalog,
        &BuffInfo {
            buff_id: Some(31320113),
            from_uid: Some(42),
            ..Default::default()
        },
    );

    assert_eq!(buff.type_id, 31320113);
    assert_eq!(
        buff.status,
        Some(crate::engine::manager::buff::BuffStatus::NegativeStatus)
    );
    assert_eq!(buff.source_uid, 42);
    assert_eq!(
        buff.features,
        ["1037#104#0#2#-10#50", "1038#31320111#1#2#1", "720#3132#0",]
    );
    assert!(
        buff.has_buff_act_kind(crate::engine::skill::buff_act::registry::BuffActKind::MonsterLabel)
    );
    assert!(buff.has_monster_label(3132));
    assert!(!buff.has_monster_label(0));

    let wire_type = TargetBuff::from_buff_info(
        catalog,
        &BuffInfo {
            buff_id: Some(31320113),
            r#type: Some(77),
            ..Default::default()
        },
    );
    assert_eq!(wire_type.type_id, 77);

    let missing = TargetBuff::from_buff_info(
        catalog,
        &BuffInfo {
            buff_id: Some(-1),
            ..Default::default()
        },
    );
    assert_eq!(missing.type_id, 0);
    assert_eq!(missing.status, None);
    assert!(missing.act_kinds.is_empty());
    assert!(missing.monster_labels.is_empty());
}

#[test]
fn grouped_career_exposes_each_configured_afflatus() {
    crate::test_support::init_config();
    let entity = TargetEntity::from_fight_entity(&FightEntityInfo {
        uid: Some(1),
        current_hp: Some(1),
        career: Some(101),
        ..Default::default()
    })
    .unwrap();

    assert!(entity.has_career(1));
    assert!(entity.has_career(2));
    assert!(!entity.has_career(3));
}

#[test]
fn selected_destiny_stone_replaces_character_battle_tags() {
    crate::test_support::init_config();
    let entity = TargetEntity::from_fight_entity(&FightEntityInfo {
        uid: Some(1),
        model_id: Some(3081),
        current_hp: Some(1),
        destiny_stone: Some(308101),
        destiny_rank: Some(1),
        ..Default::default()
    })
    .unwrap();

    assert_eq!(entity.battle_tags, vec![102, 114, 116]);
}

#[test]
fn monster_hidden_stats_use_the_instance_job_curve() {
    crate::test_support::init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(30110801),
                entity_type: Some(2),
                level: Some(170),
                current_hp: Some(100),
                attr: Some(HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };

    let entity = TargetPool::from_fight(&fight).defender_main.remove(0);

    assert_eq!(entity.crit_rate, 48);
    assert_eq!(entity.crit_dmg, 1200);
    assert_eq!(entity.crit_def, 387);
    assert_eq!(entity.add_dmg, 140);
    assert_eq!(entity.drop_dmg, 318);
}

#[test]
fn skill_slot_resolves_card_skill_ids_to_their_configured_effect() {
    crate::test_support::init_config();
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                current_hp: Some(100),
                attr: Some(HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                skill_group1: vec![31260111, 31260112, 31260113],
                skill_group2: vec![31260121, 31260122, 31260123],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let managers = BattleManagers::default();

    assert_eq!(pool.skill_slot(&managers, 1, 31260171), 1);
    assert_eq!(pool.skill_slot(&managers, 1, 31260172), 1);
    assert_eq!(pool.skill_slot(&managers, 1, 31260121), 2);
}

#[test]
fn trial_skill_identity_fills_only_missing_groups_without_changing_deck_ownership() {
    crate::test_support::init_config();
    let configured = crate::catalog::BattleCatalog::new(crate::test_support::game_data())
        .trial_skill_groups(116_385_001, 3_149)
        .unwrap();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(3_149),
                trial_id: Some(116_385_001),
                current_hp: Some(1),
                skill_group1: vec![999],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };

    let entity = TargetPool::from_fight(&fight).attacker_main.remove(0);

    assert_eq!(entity.skill_group1, vec![999]);
    assert_eq!(entity.skill_group2, configured.group2);
    assert_eq!(crate::engine::manager::card::start::deck_size(&fight), 16);

    let empty_captured_groups = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(3_149),
                trial_id: Some(116_385_001),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let runtime_entity = TargetPool::from_fight(&empty_captured_groups)
        .attacker_main
        .remove(0);

    assert_eq!(runtime_entity.skill_group1, configured.group1);
    assert_eq!(
        crate::engine::manager::card::start::deck_size(&empty_captured_groups),
        0
    );
}

#[test]
fn trial_skill_identity_rejects_missing_or_mismatched_identity() {
    crate::test_support::init_config();

    for (trial_id, model_id) in [(None, 3_149), (Some(0), 3_149), (Some(116_385_001), 999)] {
        let entity = TargetEntity::from_fight_entity(&FightEntityInfo {
            uid: Some(1),
            model_id: Some(model_id),
            trial_id,
            current_hp: Some(1),
            ..Default::default()
        })
        .unwrap();

        assert!(entity.skill_group1.is_empty());
        assert!(entity.skill_group2.is_empty());
    }
}

#[test]
fn emitter_uses_average_attacker_stats_without_joining_the_team() {
    let entity = |uid, attack| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100),
        attr: Some(HeroAttribute {
            hp: Some(100),
            attack: Some(attack),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(1, 100), entity(2, 300)],
            player_entity: Some(entity(0, 0)),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 200)],
            ..Default::default()
        }),
        ..Default::default()
    });

    assert_eq!(
        pool.entity(crate::engine::manager::emitter::UID)
            .unwrap()
            .attack,
        200
    );
    assert_eq!(pool.attacker_main.len(), 2);
    assert_eq!(pool.team_type(0), Some(1));
    assert_eq!(
        pool.team_type(crate::engine::manager::emitter::UID),
        Some(1)
    );
    assert_eq!(
        pool.enemies(crate::engine::manager::emitter::UID, false)
            .iter()
            .map(|entity| entity.uid)
            .collect::<Vec<_>>(),
        vec![-1]
    );
    assert!(pool.enemies(404, false).is_empty());
}
