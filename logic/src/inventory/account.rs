use super::InventoryManager;
use crate::error::AppError;
use database::db::game::{power_maker, unlock_voucher};
use sonettobuf::{GetPowerMakerInfoReply, GetUnlockVoucherInfoReply};
use sqlx::SqlitePool;

impl InventoryManager {
    pub async fn unlock_voucher_info(
        self,
        db: &SqlitePool,
    ) -> Result<GetUnlockVoucherInfoReply, AppError> {
        Ok(GetUnlockVoucherInfoReply {
            vouchers: unlock_voucher::get_unlock_vouchers(db, self.player_id).await?,
        })
    }

    pub async fn power_maker_info(
        self,
        db: &SqlitePool,
        is_login: bool,
    ) -> Result<GetPowerMakerInfoReply, AppError> {
        let state = power_maker::take_state(db, self.player_id, is_login).await?;
        Ok(GetPowerMakerInfoReply {
            status: Some(state.status),
            next_remain_second: Some(state.next_remain_second),
            make_count: Some(state.make_count),
            logout_second: Some(state.logout_second),
            power_maker_items: power_maker::get_maker_items(db, self.player_id).await?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn power_maker_only_reports_offline_progress_during_login() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, created_at, updated_at) VALUES (12, 'power', 0, 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO user_power_maker_state
             (user_id, status, next_remain_second, make_count, logout_second)
             VALUES (12, 1, 36123, 28, 5866693)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let manager = InventoryManager::new(12);
        let login = manager.power_maker_info(&pool, true).await.unwrap();
        let refresh = manager.power_maker_info(&pool, false).await.unwrap();
        assert_eq!((login.make_count, refresh.make_count), (Some(28), Some(0)));
    }
}
