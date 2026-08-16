use sonettobuf::{FightTeam, HeroAttribute};

use super::*;
use crate::engine::skill::rule::{DefinitionKey, RuleDomain};

fn catalog() -> crate::catalog::BattleCatalog {
    crate::catalog::BattleCatalog::new(crate::test_support::game_data())
}

#[test]
fn configured_seed_preserves_defender_uid_reservations() {
    let fight = Fight {
        battle_id: Some(9_000_161),
        ..Default::default()
    };

    let configured = EntityManager::configured(catalog(), &fight);
    let legacy = EntityManager::seed_with_game_data(crate::test_support::game_data(), &fight);

    assert_eq!(configured.next_special_uid, -3);
    assert_eq!(configured.next_special_uid, legacy.next_special_uid);
}

#[test]
fn configured_ultimate_kind_applies_only_to_the_current_ultimate() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                ex_skill: Some(900),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = EntityManager::seed(&fight);

    manager
        .execute_skill_command(EntitySkillCommand {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(100012, "EzioBigSkillWeapon2"),
            },
            target_uid: 10,
            ultimate_kind: ExtraSkillKind::ExtraAction,
        })
        .unwrap();

    assert_eq!(
        manager.skill_kind(10, 900),
        Some(ExtraSkillKind::ExtraAction)
    );
    assert_eq!(manager.skill_kind(10, 901), None);
    assert_eq!(manager.skill_kind(11, 900), None);
}

#[test]
fn special_summon_uses_a_uid_after_the_configured_wave_roster() {
    crate::test_support::init_config();
    let mut fight = Fight {
        battle_id: Some(2514),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-2),
                team_type: Some(2),
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
    let mut manager = EntityManager::seed(&fight);
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60056, "SummonSp2"),
    };

    let changes = manager
        .execute_command(
            catalog(),
            EntityCommand {
                origin,
                source_uid: -2,
                target_uid: -2,
                operation: EntityOperation::SummonSpecial { model_id: 151416 },
            },
            &HpManager::default(),
        )
        .unwrap();
    manager.sync_to_fight(&mut fight);

    assert_eq!(changes.entity.uid, Some(-8));
    assert_eq!(changes.entity.position, Some(SPECIAL_POSITION));
    assert_eq!(changes.entity.model_id, Some(151416));
    assert_eq!(fight.defender.unwrap().sp_entitys, vec![changes.entity]);
}

#[test]
fn attacker_summon_keeps_its_registered_team_despite_negative_uid() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = EntityManager::seed(&fight);
    let changes = manager
        .execute_command(
            catalog(),
            EntityCommand {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(60056, "SummonSp2"),
                },
                source_uid: 10,
                target_uid: 10,
                operation: EntityOperation::SummonSpecial { model_id: 151416 },
            },
            &HpManager::default(),
        )
        .unwrap();
    let summoned_uid = changes.entity.uid.unwrap();

    assert!(summoned_uid < 0);
    assert_eq!(manager.team_type(summoned_uid), Some(1));
}

#[test]
fn replacing_a_wave_roster_deactivates_the_previous_combatants() {
    let entity = |uid, position| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(2),
        position: Some(position),
        ..Default::default()
    };
    let mut fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 1)],
            sub_entitys: vec![entity(-2, -1)],
            sp_entitys: vec![entity(-9, SPECIAL_POSITION)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = EntityManager::seed(&fight);

    manager.replace_team_roster(2, &[entity(-3, 1)], &[entity(-4, -1)]);
    manager.sync_to_fight(&mut fight);

    let defender = fight.defender.unwrap();
    assert_eq!(
        defender
            .entitys
            .iter()
            .filter_map(|entity| entity.uid)
            .collect::<Vec<_>>(),
        vec![-3]
    );
    assert_eq!(
        defender
            .sub_entitys
            .iter()
            .filter_map(|entity| entity.uid)
            .collect::<Vec<_>>(),
        vec![-4]
    );
    assert_eq!(defender.sp_entitys[0].uid, Some(-9));
}

#[test]
fn defeated_combatant_count_keeps_prior_waves_and_excludes_special_entities() {
    let entity = |uid, current_hp, position| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(2),
        position: Some(position),
        current_hp: Some(current_hp),
        attr: Some(HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 0, 1)],
            sub_entitys: vec![entity(-2, 100, -1)],
            sp_entitys: vec![entity(-9, 0, SPECIAL_POSITION)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = EntityManager::seed(&fight);
    let mut hp = HpManager::default();
    hp.seed(&fight);

    assert_eq!(manager.defeated_combatant_count(2, &hp), 1);

    let next = entity(-3, 0, 1);
    hp.register(&next);
    manager.replace_team_roster(2, std::slice::from_ref(&next), &[]);
    manager.sync_to_fight(&mut fight);

    assert_eq!(manager.defeated_combatant_count(2, &hp), 2);
}

#[test]
fn transform_replaces_identity_without_changing_uid_or_position() {
    crate::test_support::init_config();
    let mut original = Defender::build_monster_with_uid(251417, -7, 1, 2).unwrap();
    original.passive_skill.push(71004);
    let mut fight = Fight {
        battle_id: Some(2514),
        defender: Some(FightTeam {
            entitys: vec![original],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = EntityManager::seed(&fight);
    let mut hp = HpManager::default();
    hp.seed(&fight);

    let changes = manager
        .execute_command(
            catalog(),
            EntityCommand {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(40006, "MonsterChange"),
                },
                source_uid: -7,
                target_uid: -7,
                operation: EntityOperation::Transform {
                    model_id: 251407,
                    parameters: [1000, 1],
                },
            },
            &hp,
        )
        .unwrap();
    manager.sync_to_fight(&mut fight);

    assert_eq!(changes.entity.uid, Some(-7));
    assert_eq!(changes.entity.position, Some(1));
    assert_eq!(changes.entity.model_id, Some(251407));
    assert!(changes.entity.passive_skill.contains(&1144005));
    assert!(!changes.entity.passive_skill.contains(&1144006));
    assert!(changes.entity.passive_skill.contains(&71004));
    assert_eq!(
        manager.passive_override(-7),
        Some(changes.entity.passive_skill.as_slice())
    );
    assert_eq!(fight.defender.unwrap().entitys[0].model_id, Some(251407));
}

#[test]
fn transform_carries_encounter_attribute_scaling_into_the_new_form() {
    crate::test_support::init_config();
    let mut original = Defender::build_monster_with_uid(30111001, -1, 1, 2).unwrap();
    original.passive_skill.splice(0..0, [530000151, 71004]);
    let captured = HeroAttribute {
        hp: Some(67_680),
        attack: Some(1_696),
        defense: Some(1_000),
        mdefense: Some(736),
        technic: Some(210),
        multi_hp_idx: Some(0),
        multi_hp_num: Some(0),
    };
    original.current_hp = Some(0);
    original.attr = Some(captured);
    original.base_attr = Some(captured);
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![original],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = EntityManager::seed(&fight);
    let mut hp = HpManager::default();
    hp.seed(&fight);

    let changes = manager
        .execute_command(
            catalog(),
            EntityCommand {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(40006, "MonsterChange"),
                },
                source_uid: -1,
                target_uid: -1,
                operation: EntityOperation::Transform {
                    model_id: 30111005,
                    parameters: [1000, 0],
                },
            },
            &hp,
        )
        .unwrap();

    let restored = HeroAttribute {
        multi_hp_idx: Some(-1),
        ..captured
    };
    assert_eq!(changes.entity.attr, Some(restored));
    assert_eq!(changes.entity.base_attr, Some(restored));
    assert_eq!(changes.entity.current_hp, captured.hp);
    assert_eq!(
        changes.entity.passive_skill,
        [
            530000151, 71004, 530000741, 530000742, 530002746, 530000747, 530000153,
        ]
    );
}

#[test]
fn transform_without_hp_restoration_preserves_current_hp_and_phase_marker() {
    crate::test_support::init_config();
    let mut original = Defender::build_monster_with_uid(900110103, -2, 1, 2).unwrap();
    original.current_hp = Some(658_185);
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![original],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = EntityManager::seed(&fight);
    let mut hp = HpManager::default();
    hp.seed(&fight);

    let changes = manager
        .execute_command(
            catalog(),
            EntityCommand {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(40006, "MonsterChange"),
                },
                source_uid: -2,
                target_uid: -2,
                operation: EntityOperation::Transform {
                    model_id: 900110104,
                    parameters: [0, 0],
                },
            },
            &hp,
        )
        .unwrap();

    assert_eq!(changes.entity.current_hp, Some(658_185));
    assert_eq!(changes.entity.attr.unwrap().multi_hp_idx, Some(0));
    assert_eq!(changes.entity.base_attr.unwrap().multi_hp_idx, Some(0));
}

#[test]
fn combatant_summon_joins_the_active_team_at_the_allocated_position() {
    crate::test_support::init_config();
    let mut fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                position: Some(1),
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
    let mut manager = EntityManager::seed(&fight);
    let changes = manager
        .execute_command(
            catalog(),
            EntityCommand {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(60008, "Summon"),
                },
                source_uid: -1,
                target_uid: -1,
                operation: EntityOperation::SummonCombatant {
                    model_id: 30111003,
                    position: 2,
                },
            },
            &HpManager::default(),
        )
        .unwrap();
    manager.sync_to_fight(&mut fight);

    assert_eq!(changes.entity.uid, Some(-2));
    assert_eq!(changes.entity.position, Some(2));
    assert_eq!(changes.entity.model_id, Some(30111003));
    assert_eq!(fight.defender.unwrap().entitys[1], changes.entity);
}
