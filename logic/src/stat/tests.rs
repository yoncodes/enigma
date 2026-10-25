#[test]
fn veteran_stat_push_omits_default_flags() {
    let push: sonettobuf::StatInfoPush = database::models::game::user_stats::UserStats {
        user_id: 1,
        first_charge: false,
        total_charge_amount: 594_408,
        is_first_login: false,
        user_tag: "用户类型7".into(),
    }
    .into();

    assert_eq!(push.frist_charge, None);
    assert_eq!(push.is_first_login, None);
    assert_eq!(push.total_charge_amount, Some(594_408));
}
