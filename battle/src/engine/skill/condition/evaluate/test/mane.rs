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
