use super::*;

fn target_pool() -> TargetPool {
    TargetPool::from_fight(&Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    })
}

#[test]
fn natural_buff_expiry_rejects_other_removals() {
    init_config();
    let condition = exact_condition(515005, "BuffIdExpireOnly", &["4004"]);
    let pool = target_pool();
    let matches = |duration_expired| {
        conditions_match(
            std::slice::from_ref(&condition),
            10,
            &[-1],
            None,
            &pool,
            TargetContext {
                removed_buff_id: 4004,
                removed_buff_target_uid: -1,
                removed_buff_duration_expired: duration_expired,
                ..Default::default()
            },
        )
    };

    assert!(matches(true));
    assert!(!matches(false));
}

#[test]
fn buff_category_condition_matches_only_the_added_status() {
    init_config();
    let condition = exact_condition(88, "BuffTypeAdd", &["4"]);
    let pool = target_pool();
    let matches = |status_id| {
        conditions_match(
            std::slice::from_ref(&condition),
            10,
            &[-1],
            None,
            &pool,
            TargetContext {
                added_buff_amount: 1,
                added_buff_target_uid: -1,
                added_buff_status_id: status_id,
                ..Default::default()
            },
        )
    };

    assert!(matches(4));
    assert!(!matches(6));
}

#[test]
fn received_target_count_kind_matches_only_the_passive_owner() {
    init_config();
    let condition = exact_condition(53209, "HurtNumType", &["2"]);
    let pool = target_pool();
    let matches = |hit_target_uid| {
        conditions_match(
            std::slice::from_ref(&condition),
            -1,
            &[-1],
            None,
            &pool,
            TargetContext {
                damage_target_count_kind: 2,
                hit_target_uid,
                ..Default::default()
            },
        )
    };

    assert!(matches(-1));
    assert!(!matches(-2));
}

#[test]
fn damage_target_count_kind_rejects_non_attacks() {
    init_config();
    let condition = exact_condition(53208, "HurtNumType", &["1"]);
    let pool = target_pool();
    let matches = |active_skill_is_attack| {
        conditions_match(
            std::slice::from_ref(&condition),
            10,
            &[10],
            None,
            &pool,
            TargetContext {
                active_skill_is_attack,
                damage_target_count_kind: 1,
                ..Default::default()
            },
        )
    };

    assert!(matches(true));
    assert!(!matches(false));
}

#[test]
fn all_in_buff_type_requires_every_condition_target() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: [-1, -2]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        uid: Some(-uid),
                        buff_id: Some(117373041),
                        layer: Some(if uid == -1 { 2 } else { 1 }),
                        ..Default::default()
                    }],
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let condition = exact_condition(550102, "AllInBuffByTypeId", &["117373041", "1"]);

    assert!(conditions_match(
        std::slice::from_ref(&condition),
        -1,
        &[-1, -2],
        Some(&managers),
        &pool,
        TargetContext::default(),
    ));
    assert!(!conditions_match(
        &[condition],
        -1,
        &[-1, -2, -3],
        Some(&managers),
        &pool,
        TargetContext::default(),
    ));
}

#[test]
fn team_buff_type_layer_gate_sums_every_condition_target() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: [(-1, 2), (-2, 4)]
                .into_iter()
                .map(|(uid, layer)| FightEntityInfo {
                    uid: Some(uid),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        uid: Some(-uid),
                        buff_id: Some(117373041),
                        layer: Some(layer),
                        ..Default::default()
                    }],
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let matches = |threshold| {
        conditions_match(
            &[exact_condition(
                513102,
                "SelfTeamHasBuffTypeLayerMoreThan",
                &[threshold, "117373041"],
            )],
            -1,
            &[-1, -2],
            Some(&managers),
            &pool,
            TargetContext::default(),
        )
    };

    assert!(matches("6"));
    assert!(!matches("7"));
}
