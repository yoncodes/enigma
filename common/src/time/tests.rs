use super::*;

fn ts_ms(year: i32, month: u32, day: u32, hour: u32, min: u32, sec: u32) -> i64 {
    Utc.with_ymd_and_hms(year, month, day, hour, min, sec)
        .single()
        .unwrap()
        .timestamp_millis()
}

#[test]
fn en_server_day_resets_at_five_server_local() {
    let before_reset = ts_ms(2026, 7, 3, 9, 59, 59);
    let at_reset = ts_ms(2026, 7, 3, 10, 0, 0);

    assert_eq!(ServerTime::day_of_month(before_reset), 2);
    assert_eq!(ServerTime::day_of_month(at_reset), 3);
    assert_ne!(
        ServerTime::server_day(before_reset),
        ServerTime::server_day(at_reset)
    );
}

#[test]
fn provided_sign_in_log_stays_on_july_third() {
    let logged_time = ts_ms(2026, 7, 3, 22, 14, 9);

    assert_eq!(ServerTime::day_of_month(logged_time), 3);
    assert_eq!(ServerTime::server_day(logged_time), 20_637);
}

#[test]
fn tower_config_dates_match_live_server_boundaries() {
    assert_eq!(
        ServerTime::config_date_start_ms("2025-09-08"),
        Some(1_757_325_600_000)
    );
    assert_eq!(
        ServerTime::config_date_end_ms("2025-10-20"),
        Some(1_760_954_399_000)
    );
}

#[test]
fn config_datetimes_use_server_local_time() {
    assert_eq!(
        ServerTime::config_datetime_sec("2026-08-13 05:00:00"),
        Some(1_786_615_200)
    );
    assert_eq!(
        ServerTime::config_datetime_sec("2024/05/19 05:00:00"),
        Some(1_716_112_800)
    );
    assert_eq!(ServerTime::config_datetime_sec(""), None);
}

#[test]
fn task_expiry_uses_the_next_daily_and_monday_resets() {
    let sunday = ts_ms(2026, 7, 19, 18, 0, 0);
    let monday = ts_ms(2026, 7, 20, 18, 0, 0);

    assert_eq!(
        ServerTime::next_daily_refresh_sec(monday),
        (ts_ms(2026, 7, 21, 10, 0, 0) / 1_000) as i32
    );
    assert_eq!(
        ServerTime::next_weekly_refresh_sec(sunday),
        (ts_ms(2026, 7, 20, 10, 0, 0) / 1_000) as i32
    );
    assert_eq!(
        ServerTime::next_weekly_refresh_sec(monday),
        (ts_ms(2026, 7, 27, 10, 0, 0) / 1_000) as i32
    );
}
