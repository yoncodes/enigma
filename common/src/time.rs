use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime, TimeZone, Utc};

pub struct ServerTime;

const DAY_MS: i64 = 86_400_000;
const DAY_SEC: i64 = 86_400;

pub const SERVER_UTC_OFFSET_MS: i64 = -5 * 60 * 60 * 1000;
pub const DAILY_REFRESH_HOUR: i64 = 5;

const DAILY_REFRESH_OFFSET_MS: i64 = DAILY_REFRESH_HOUR * 60 * 60 * 1000;
const SERVER_DAY_SHIFT_MS: i64 = SERVER_UTC_OFFSET_MS - DAILY_REFRESH_OFFSET_MS;

impl ServerTime {
    #[inline]
    pub fn now_ms() -> i64 {
        Utc::now().timestamp_millis()
    }

    #[inline]
    pub const fn server_utc_offset_ms() -> i64 {
        SERVER_UTC_OFFSET_MS
    }

    #[inline]
    pub fn adjusted_datetime(timestamp_ms: i64) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(timestamp_ms + SERVER_DAY_SHIFT_MS)
            .single()
            .expect("invalid UTC timestamp")
    }

    #[inline]
    pub fn server_day(now_ms: i64) -> i64 {
        (now_ms + SERVER_DAY_SHIFT_MS).div_euclid(DAY_MS)
    }

    #[inline]
    pub fn server_day_start_ms(now_ms: i64) -> i64 {
        Self::server_day(now_ms) * DAY_MS - SERVER_DAY_SHIFT_MS
    }

    #[inline]
    pub fn next_daily_refresh_sec(now_ms: i64) -> i32 {
        ((Self::server_day_start_ms(now_ms) + DAY_MS) / 1_000) as i32
    }

    pub fn next_weekly_refresh_sec(now_ms: i64) -> i32 {
        let days_until_monday = match Self::server_weekday(now_ms) {
            0 => 1,
            weekday => 8 - i64::from(weekday),
        };
        ((Self::server_day_start_ms(now_ms) + days_until_monday * DAY_MS) / 1_000) as i32
    }

    #[inline]
    pub fn day_of_month(timestamp_ms: i64) -> u32 {
        Self::adjusted_datetime(timestamp_ms).day()
    }

    #[inline]
    pub fn is_same_day(t1: i64, t2: i64) -> bool {
        Self::server_day(t1) == Self::server_day(t2)
    }

    #[inline]
    pub fn is_new_day(last: i64, now: i64) -> bool {
        !Self::is_same_day(last, now)
    }

    #[inline]
    pub fn server_week(timestamp_ms: i64) -> i32 {
        let adjusted = Self::adjusted_datetime(timestamp_ms);
        let days = adjusted.timestamp().div_euclid(DAY_SEC);
        ((days + 3) / 7) as i32
    }

    #[inline]
    pub fn is_same_week(t1: i64, t2: i64) -> bool {
        Self::server_week(t1) == Self::server_week(t2)
    }

    #[inline]
    pub fn server_weekday(timestamp_ms: i64) -> i32 {
        Self::adjusted_datetime(timestamp_ms)
            .weekday()
            .num_days_from_sunday() as i32
    }

    #[inline]
    pub fn server_month(timestamp_ms: i64) -> i32 {
        let dt = Self::adjusted_datetime(timestamp_ms);
        dt.year() * 100 + dt.month() as i32
    }

    #[inline]
    pub fn is_same_month(t1: i64, t2: i64) -> bool {
        Self::server_month(t1) == Self::server_month(t2)
    }

    pub fn server_date() -> DateTime<Utc> {
        Self::adjusted_datetime(Self::now_ms())
    }

    pub fn config_date_start_ms(value: &str) -> Option<i64> {
        let local = NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
            .ok()?
            .and_hms_opt(DAILY_REFRESH_HOUR as u32, 0, 0)?
            .and_utc()
            .timestamp_millis();
        Some(local - SERVER_UTC_OFFSET_MS)
    }

    pub fn config_date_end_ms(value: &str) -> Option<i64> {
        Self::config_date_start_ms(value).map(|time| time - 1_000)
    }

    pub fn config_datetime_sec(value: &str) -> Option<i32> {
        let value = value.trim();
        let local = NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
            .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y/%m/%d %H:%M:%S"))
            .ok()?
            .and_utc()
            .timestamp();
        i32::try_from(local - SERVER_UTC_OFFSET_MS / 1_000).ok()
    }

    #[inline]
    pub fn now_sec_i32() -> i32 {
        (Self::now_ms() / 1000) as i32
    }
}

#[cfg(test)]
mod tests {
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
}
