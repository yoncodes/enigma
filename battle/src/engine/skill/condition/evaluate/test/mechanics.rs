use super::*;

#[test]
fn blood_pool_max_uses_runtime_state() {
    init_config();
    let condition = ParsedCondition {
        opcode: 740203,
        type_name: String::new(),
        kind: ParsedConditionKind::BloodPoolMax { min: 50, max: 999 },
        raw_args: vec!["50".into(), "999".into()],
    };

    assert!(conditions_match(
        std::slice::from_ref(&condition),
        10,
        &[10],
        None,
        &TargetPool::default(),
        TargetContext {
            blood_pool_max: 84,
            ..Default::default()
        },
    ));
    assert!(!conditions_match(
        &[condition],
        10,
        &[10],
        None,
        &TargetPool::default(),
        TargetContext::default(),
    ));
}

#[test]
fn blood_pool_value_selects_the_configured_shared_gauge() {
    init_config();
    let condition = |config_effect| ParsedCondition {
        opcode: 726304,
        type_name: "BloodPoolValue".into(),
        kind: ParsedConditionKind::BloodPoolValue {
            min: 60_000,
            max: 1_000_000,
            config_effect,
        },
        raw_args: vec!["60000".into(), "1000000".into(), config_effect.to_string()],
    };
    let context = TargetContext {
        blood_pool_value: 70_000,
        heat_scale_raw_value: 60_000,
        ..Default::default()
    };

    assert!(conditions_match(
        &[condition(0)],
        10,
        &[10],
        None,
        &TargetPool::default(),
        context,
    ));
    assert!(conditions_match(
        &[condition(1)],
        10,
        &[10],
        None,
        &TargetPool::default(),
        context,
    ));
    assert!(!conditions_match(
        &[condition(2)],
        10,
        &[10],
        None,
        &TargetPool::default(),
        context,
    ));
}

#[test]
fn ally_attacked_is_distinct_from_carrier_attacked() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(1),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let condition = ParsedCondition {
        opcode: 22213,
        type_name: String::new(),
        kind: ParsedConditionKind::AllyAttacked,
        raw_args: Vec::new(),
    };
    let matches = |hit_target_uid| {
        conditions_match(
            std::slice::from_ref(&condition),
            10,
            &[10],
            None,
            &pool,
            TargetContext {
                hit_target_uid,
                ..Default::default()
            },
        )
    };

    assert!(matches(11));
    assert!(!matches(10));
    assert!(!matches(-1));
}

#[test]
fn no_action_round_checks_the_owner_card_state() {
    init_config();
    let condition = ParsedCondition {
        opcode: 46301,
        type_name: String::new(),
        kind: ParsedConditionKind::NoActionRound,
        raw_args: Vec::new(),
    };

    assert!(conditions_match(
        std::slice::from_ref(&condition),
        10,
        &[],
        None,
        &TargetPool::default(),
        TargetContext::default(),
    ));
    assert!(!conditions_match(
        &[condition],
        10,
        &[],
        None,
        &TargetPool::default(),
        TargetContext {
            owner_played_card: true,
            ..Default::default()
        },
    ));
}

#[test]
fn random_condition_requires_and_compares_the_runtime_roll() {
    init_config();
    let condition = ParsedCondition {
        opcode: 552210,
        type_name: String::new(),
        kind: ParsedConditionKind::Random { threshold: 500 },
        raw_args: vec!["500".to_owned()],
    };
    let matches = |condition_random_roll| {
        conditions_match(
            std::slice::from_ref(&condition),
            10,
            &[],
            None,
            &TargetPool::default(),
            TargetContext {
                condition_random_roll,
                ..Default::default()
            },
        )
    };

    assert!(!matches(None));
    assert!(matches(Some(499)));
    assert!(!matches(Some(500)));
}

#[test]
fn follow_up_and_riposte_are_also_extra_actions() {
    init_config();
    assert!(extra_action_kind_matches(1, &[1]));
    assert!(extra_action_kind_matches(2, &[1]));
    assert!(extra_action_kind_matches(3, &[1]));
    assert!(extra_action_kind_matches(2, &[2]));
    assert!(!extra_action_kind_matches(1, &[2]));
}

#[test]
fn other_ally_extra_action_rejects_the_passive_owner() {
    init_config();
    let condition = ParsedCondition {
        opcode: 403212,
        type_name: "SkillExtraType".into(),
        kind: ParsedConditionKind::ExtraAction {
            mode: crate::engine::skill::condition::extra::ExtraActionConditionMode::OtherAllyAction,
            kinds: vec![1],
        },
        raw_args: vec!["1".into()],
    };
    let matches = |active_skill_source_uid, extra_skill_kind| {
        conditions_match(
            std::slice::from_ref(&condition),
            10,
            &[11],
            None,
            &TargetPool::default(),
            TargetContext {
                active_skill_source_uid,
                extra_skill_kind,
                ..Default::default()
            },
        )
    };

    assert!(!matches(10, 1));
    assert!(!matches(11, 0));
    assert!(matches(11, 1));
    assert!(matches(11, 2));
}

#[test]
fn power_compare_codes_form_inclusive_config_ranges() {
    init_config();
    assert!(compare_resource(30, 1, 30));
    assert!(compare_resource(50, 1, 30));
    assert!(!compare_resource(29, 1, 30));

    assert!(compare_resource(50, 2, 69));
    assert!(compare_resource(69, 2, 69));
    assert!(!compare_resource(70, 2, 69));
}

#[test]
fn hurt_kind_reads_the_attacker_damage_type() {
    init_config();
    let reality = ParsedCondition {
        opcode: 20209,
        type_name: "HurtReal".into(),
        kind: ParsedConditionKind::AttackerDamageType(
            crate::engine::skill::target::EntityDamageType::Reality,
        ),
        raw_args: Vec::new(),
    };
    let mental = ParsedCondition {
        opcode: 21209,
        type_name: "HurtMagic".into(),
        kind: ParsedConditionKind::AttackerDamageType(
            crate::engine::skill::target::EntityDamageType::Mental,
        ),
        raw_args: Vec::new(),
    };
    let mut attacker = TargetEntity::default();
    attacker.uid = 10;
    attacker.damage_type = crate::engine::skill::target::EntityDamageType::Reality;
    let mut pool = TargetPool::default();
    pool.attacker_main.push(attacker.clone());
    pool.attacker_all.push(attacker);
    let context = TargetContext {
        hit_source_uid: 10,
        ..Default::default()
    };

    assert!(conditions_match(
        &[reality],
        -1,
        &[-1],
        None,
        &pool,
        context
    ));
    assert!(!conditions_match(
        &[mental],
        -1,
        &[-1],
        None,
        &pool,
        context
    ));
}

#[test]
fn alive_team_count_reads_manager_hp_not_the_fight_snapshot() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![10, 11]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    current_hp: Some(100),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.hp.lose(11, 100, -1);
    let condition = ParsedCondition {
        opcode: 616012,
        type_name: "TeammateAliveNumNoSp".into(),
        kind: ParsedConditionKind::EntityCount {
            scope: EntityCountScope::AliveTeammatesNoSp,
            compare: ConditionCompare::Equal,
            count: 1,
        },
        raw_args: vec!["1".into()],
    };

    assert!(conditions_match(
        &[condition],
        10,
        &[10],
        Some(&managers),
        &pool,
        TargetContext::default(),
    ));
}

#[test]
fn battle_tag_count_uses_alive_members_of_the_casters_team() {
    init_config();
    let entity = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        entity_type: Some(1),
        current_hp: Some(100),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 3127), entity(11, 3134)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 3139)],
            ..Default::default()
        }),
        ..Default::default()
    });
    let condition = ParsedCondition {
        opcode: 762021,
        type_name: "BattleTagNum".into(),
        kind: ParsedConditionKind::BattleTagCount {
            tag_id: 114,
            compare: ConditionCompare::Equal,
            threshold: 2,
        },
        raw_args: vec!["114".into(), "2".into(), "3".into()],
    };

    assert!(conditions_match(
        &[condition],
        10,
        &[10],
        None,
        &pool,
        TargetContext::default(),
    ));
}
