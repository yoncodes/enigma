use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct Rouge2State {
    pub user_id: i64,
    pub state: i32,
    pub difficulty: i32,
    pub coin: i32,
    pub end_id: i32,
    pub game_num: i32,
    pub genius_point: i32,
    pub genius_ids: String,
    pub reward_point: i32,
    pub max_difficulty: i32,
    pub pass_layer_ids: String,
    pub pass_event_ids: String,
    pub pass_end_ids: String,
    pub pass_entrust_ids: String,
    pub pass_collections: String,
    pub last_game_time: i64,
    pub hotfix_str: String,
    pub alchemy_formula: i32,
    pub alchemy_main_effect: i32,
    pub alchemy_sub_effects: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct Rouge2CareerLevel {
    pub user_id: i64,
    pub career_id: i32,
    pub exp: i32,
    pub updated_at: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct Rouge2RewardState {
    pub user_id: i64,
    pub reward_id: i32,
    pub buy_count: i32,
    pub updated_at: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct Rouge2MaterialState {
    pub user_id: i64,
    pub material_id: i32,
    pub num: i32,
    pub is_seen: bool,
    pub updated_at: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct Rouge2AttrState {
    pub user_id: i64,
    pub attr_id: i32,
    pub final_val: i32,
    pub updated_at: i64,
}
