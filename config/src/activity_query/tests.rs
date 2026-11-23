#[test]
fn act236_reward_rows_load_for_the_configured_activity() {
    crate::init_test_config();
    let tables = crate::configs::get();
    let activity_id = tables.latest_open_activity_id(236).unwrap();
    let rows = tables
        .activity236
        .iter()
        .filter(|row| row.activity_id == activity_id)
        .collect::<Vec<_>>();

    assert_eq!(rows.len(), 9);
    assert_eq!((rows[0].cost, rows[0].reward.as_str()), (0, "2#2#100"));
    assert!(rows.windows(2).all(|rows| rows[0].id < rows[1].id));
}

#[test]
fn act236_charge_score_uses_configured_price_and_conversion_rate() {
    crate::init_test_config();
    let tables = crate::configs::get();
    let activity_id = tables.latest_open_activity_id(236).unwrap();

    assert_eq!(
        tables
            .activity236_control
            .get(activity_id)
            .unwrap()
            .conversion_rate,
        "CNY#10000"
    );
    for (goods_id, score) in [
        (837029, 4880),
        (837022, 3280),
        (811327, 60),
        (837008, 1980),
        (811390, 5980),
    ] {
        assert_eq!(
            tables.activity236_charge_score(activity_id, goods_id),
            Some(score)
        );
    }
}

#[test]
fn act128_rank_config_maps_currency_thresholds_and_captured_rewards() {
    crate::init_test_config();
    let tables = crate::configs::get();

    assert_eq!(tables.activity128_rank_currency_id(), Some(3206));
    assert_eq!(tables.activity128_rank(700), Some(7));

    let levels = tables.activity128_milestone_levels(2, 7).unwrap();
    let bonuses = levels
        .into_iter()
        .filter(|row| !row.bonus.is_empty())
        .map(|row| row.bonus.as_str())
        .collect::<Vec<_>>();
    assert_eq!(bonuses, vec!["1#120013#2", "1#110404#1"]);
}
