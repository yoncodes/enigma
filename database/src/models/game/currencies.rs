use sonettobuf;
use sqlx::{FromRow, SqlitePool};

#[async_trait::async_trait]
pub trait CurrencyModel<T>: Send + Sync {
    async fn get_all(&self) -> Result<Vec<T>, sqlx::Error>;
    async fn get(&self, currency_id: i32) -> Result<Option<T>, sqlx::Error>;
    async fn create(&self, currency_id: i32, amount: i32) -> Result<Vec<i32>, sqlx::Error>;
    async fn update_quantity(&self, currency_id: i32, delta: i32) -> Result<bool, sqlx::Error>;
}

pub struct UserCurrencyModel {
    user_id: i64,
    pool: SqlitePool,
}

#[derive(Debug, Clone, FromRow)]
pub struct Currency {
    pub user_id: i64,
    pub currency_id: i32,
    pub quantity: i32,
    pub last_recover_time: Option<i64>,
    pub expired_time: Option<i64>,
}

impl From<Currency> for sonettobuf::Currency {
    fn from(c: Currency) -> Self {
        sonettobuf::Currency {
            currency_id: Some(c.currency_id as u32),
            quantity: Some(c.quantity),
            last_recover_time: c.last_recover_time.map(|t| t as u64),
            expired_time: c.expired_time.map(|t| t as u64),
        }
    }
}

impl UserCurrencyModel {
    pub fn new(user_id: i64, pool: SqlitePool) -> Self {
        Self { user_id, pool }
    }
}

#[async_trait::async_trait]
impl CurrencyModel<Currency> for UserCurrencyModel {
    async fn get_all(&self) -> Result<Vec<Currency>, sqlx::Error> {
        crate::db::game::currencies::settle_power_recovery(&self.pool, self.user_id).await?;
        sqlx::query_as::<_, Currency>(
            "SELECT user_id, currency_id, quantity, last_recover_time, expired_time
             FROM currencies
             WHERE user_id = ? ORDER BY currency_id",
        )
        .bind(self.user_id)
        .fetch_all(&self.pool)
        .await
    }

    async fn get(&self, currency_id: i32) -> Result<Option<Currency>, sqlx::Error> {
        crate::db::game::currencies::get_currency(&self.pool, self.user_id, currency_id).await
    }

    async fn create(&self, currency_id: i32, amount: i32) -> Result<Vec<i32>, sqlx::Error> {
        crate::db::game::currencies::add_currency(&self.pool, self.user_id, currency_id, amount)
            .await?;

        Ok(vec![currency_id])
    }

    async fn update_quantity(&self, currency_id: i32, delta: i32) -> Result<bool, sqlx::Error> {
        if delta < 0 {
            return crate::db::game::currencies::remove_currency(
                &self.pool,
                self.user_id,
                currency_id,
                delta.saturating_abs(),
            )
            .await;
        }

        CurrencyModel::<Currency>::create(self, currency_id, delta).await?;

        Ok(true)
    }
}

impl UserCurrencyModel {
    pub async fn get_currency(&self, currency_id: i32) -> Result<Option<Currency>, sqlx::Error> {
        CurrencyModel::<Currency>::get(self, currency_id).await
    }

    pub async fn get_all_currencies(&self) -> Result<Vec<Currency>, sqlx::Error> {
        CurrencyModel::<Currency>::get_all(self).await
    }

    pub async fn update_currency(&self, currency_id: i32, delta: i32) -> Result<bool, sqlx::Error> {
        CurrencyModel::<Currency>::update_quantity(self, currency_id, delta).await
    }

    pub async fn add_currency(&self, currency_id: i32, amount: i32) -> Result<bool, sqlx::Error> {
        self.update_currency(currency_id, amount).await
    }

    pub async fn remove_currency(
        &self,
        currency_id: i32,
        amount: i32,
    ) -> Result<bool, sqlx::Error> {
        self.update_currency(currency_id, -amount).await
    }

    pub async fn create_currencies(
        &self,
        currencies: &[(i32, i32)],
    ) -> Result<Vec<(i32, i32)>, sqlx::Error> {
        let mut changes = Vec::new();
        for (currency_id, amount) in currencies {
            CurrencyModel::<Currency>::create(self, *currency_id, *amount).await?;
            changes.push((*currency_id, *amount));
        }
        Ok(changes)
    }
}
