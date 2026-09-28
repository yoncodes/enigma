use sonettobuf;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct UserSummonStats {
    pub user_id: i64,
    pub free_equip_summon: bool,
    pub is_show_new_summon: bool,
    pub new_summon_count: i32,
    pub total_summon_count: i32,
}

#[derive(Debug, Clone, FromRow)]
pub struct UserSummonPool {
    pub id: i64,
    pub user_id: i64,
    pub pool_id: i32,
    pub online_time: i32,
    pub offline_time: i32,
    pub have_free: bool,
    pub used_free_count: i32,
    pub discount_time: i32,
    pub can_get_guarantee_sr_count: i32,
    pub guarantee_sr_countdown: i32,
    pub summon_count: i32,
    pub have_free10_count: i32,
    pub not_ssr_count: i32,
    pub total_free10_use_count: i32,

    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct SummonPoolWindow {
    pub pool_id: i32,
    pub online_time: i32,
    pub offline_time: i32,
    pub discount_time: i32,
}

#[derive(Debug, Clone)]
pub struct SingleBagInfo {
    pub bag_id: i32,
    pub is_open: bool,
}

impl From<SingleBagInfo> for sonettobuf::SingleBagInfo {
    fn from(b: SingleBagInfo) -> Self {
        sonettobuf::SingleBagInfo {
            bag_id: Some(b.bag_id),
            is_open: Some(b.is_open),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LuckyBagInfo {
    pub count: i32,
    pub single_bag_infos: Vec<SingleBagInfo>,
    pub not_ssr_count: i32,
}

impl From<LuckyBagInfo> for sonettobuf::LuckyBagInfo {
    fn from(info: LuckyBagInfo) -> Self {
        sonettobuf::LuckyBagInfo {
            count: Some(info.count),
            single_bag_infos: info.single_bag_infos.into_iter().map(Into::into).collect(),
            not_ssr_count: Some(info.not_ssr_count),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SpPoolInfo {
    pub sp_type: i32,
    pub up_hero_ids: Vec<i32>,
    pub limited_ticket_id: i32,
    pub limited_ticket_num: i32,
    pub open_time: u64,
    pub used_first_ssr_guarantee: bool,
    pub has_get_reward_progresses: Vec<i32>,
    pub infallible_item_status: i32,
}

impl From<SpPoolInfo> for sonettobuf::SpPoolInfo {
    fn from(info: SpPoolInfo) -> Self {
        sonettobuf::SpPoolInfo {
            r#type: Some(info.sp_type),
            up_hero_ids: info.up_hero_ids,
            limited_ticket_id: Some(info.limited_ticket_id),
            limited_ticket_num: Some(info.limited_ticket_num),
            open_time: Some(info.open_time),
            used_first_ssr_guarantee: Some(info.used_first_ssr_guarantee),
            has_get_reward_progresses: info.has_get_reward_progresses,
            infallible_item_status: Some(info.infallible_item_status),
            choose_reward_infos: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PopUpInfo {
    pub order_id: i32,
    pub recommend_pop_up_count: i32,
}

impl From<PopUpInfo> for sonettobuf::PopUpInfo {
    fn from(info: PopUpInfo) -> Self {
        sonettobuf::PopUpInfo {
            order_id: Some(info.order_id),
            recommend_pop_up_count: Some(info.recommend_pop_up_count),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SummonPoolInfo {
    pub pool: UserSummonPool,
    pub pity_6: i32,
    pub lucky_bag: Option<LuckyBagInfo>,
    pub sp_pool: Option<SpPoolInfo>,
    pub pop_up_infos: Vec<PopUpInfo>,
}

impl From<SummonPoolInfo> for sonettobuf::SummonPoolInfo {
    fn from(info: SummonPoolInfo) -> Self {
        sonettobuf::SummonPoolInfo {
            pool_id: Some(info.pool.pool_id),
            online_time: Some(info.pool.online_time),
            offline_time: Some(info.pool.offline_time),
            have_free: Some(info.pool.have_free),
            used_free_count: Some(info.pool.used_free_count),
            lucky_bag_info: info.lucky_bag.map(Into::into),
            sp_pool_info: info.sp_pool.map(Into::into),
            discount_time: Some(info.pool.discount_time),
            can_get_guarantee_sr_count: Some(info.pool.can_get_guarantee_sr_count),
            guarantee_sr_count_down: Some(info.pool.guarantee_sr_countdown),
            summon_count: Some(info.pool.summon_count),
            have_free10_count: Some(info.pool.have_free10_count),
            not_ssr_count: Some(info.pool.not_ssr_count),
            total_free10_use_count: Some(info.pool.total_free10_use_count),
            pop_up_infos: info.pop_up_infos.into_iter().map(Into::into).collect(),
        }
    }
}
