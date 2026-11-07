use sqlx::FromRow;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TurnbackDropKind {
    Progress = 1,
}

impl TurnbackDropKind {
    pub const fn id(self) -> i32 {
        self as i32
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct TurnbackState {
    pub user_id: i64,
    pub turnback_id: i32,
    pub bonus_point: i32,
    pub first_show: bool,
    pub has_get_task_bonus: String,
    pub sign_in_day: i32,
    pub once_bonus: bool,
    pub start_time: i32,
    pub end_time: i32,
    pub remain_addition_count: i32,
    pub leave_time: i32,
    pub month_card_added_buy_count: i32,
    pub version: i32,
    pub buy_double_bonus: bool,
    pub get_daily_bonus: i32,
    pub has_get_return_reward_ids: String,
    pub return_random_task_id: i32,
    pub return_left_task_ids: String,
    pub return_last_random_task_id: i32,
    pub return_random_refresh_day: i64,
    pub updated_at: i64,
}

impl TurnbackState {
    pub fn claimed_task_bonus_ids(&self) -> serde_json::Result<Vec<i32>> {
        serde_json::from_str(&self.has_get_task_bonus)
    }

    pub fn claimed_return_reward_ids(&self) -> serde_json::Result<Vec<i32>> {
        serde_json::from_str(&self.has_get_return_reward_ids)
    }

    pub fn return_left_task_ids(&self) -> serde_json::Result<Vec<i32>> {
        serde_json::from_str(&self.return_left_task_ids)
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct TurnbackSignInState {
    pub user_id: i64,
    pub turnback_id: i32,
    pub day: i32,
    pub state: i32,
    pub updated_at: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct TurnbackDropState {
    pub user_id: i64,
    pub turnback_id: i32,
    pub drop_id: i32,
    pub current_num: i32,
    pub updated_at: i64,
}
