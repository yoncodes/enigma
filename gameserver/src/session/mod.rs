use crate::{
    error::{AppError, PacketError},
    net::context::ConnectionContext,
    player::Player,
};
use byteorder::{BE, ByteOrder};
use common::time::ServerTime;
use database::db::user::account;
use logic::task::UserTask;
use sqlx::SqlitePool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginRequest {
    pub account_id: String,
    pub token: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoginSession {
    pub user_id: i64,
}

pub fn parse_login_request(data: &[u8]) -> Result<LoginRequest, AppError> {
    if data.len() < 2 {
        return Err(AppError::Packet(PacketError::Custom(
            "Login request too short".into(),
        )));
    }

    let account_len = BE::read_u16(&data[..2]) as usize;
    if data.len() < 2 + account_len {
        return Err(AppError::Packet(PacketError::Custom(
            "Login request account length mismatch".into(),
        )));
    }

    let account = std::str::from_utf8(&data[2..2 + account_len])?;
    let rest = &data[2 + account_len..];
    let token = if rest.len() >= 2 {
        let token_len = BE::read_u16(&rest[..2]) as usize;
        if rest.len() < 2 + token_len {
            return Err(AppError::Packet(PacketError::Custom(
                "Login request token length mismatch".into(),
            )));
        }
        std::str::from_utf8(&rest[2..2 + token_len])?.to_string()
    } else if let Some((_, token)) = account.split_once(['#', '$']) {
        token.to_string()
    } else {
        String::new()
    };

    let account_id = account
        .split(['#', '$'])
        .next()
        .unwrap_or(account)
        .to_string();

    Ok(LoginRequest { account_id, token })
}

pub fn extract_user_id(account_id: &str) -> Result<i64, AppError> {
    account_id
        .split('_')
        .nth(1)
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or_else(|| AppError::Custom(format!("Invalid account_id format: {account_id}")))
}

pub async fn validate_login(
    pool: &SqlitePool,
    req: LoginRequest,
) -> Result<LoginSession, AppError> {
    let user_id = extract_user_id(&req.account_id)?;
    let token = account::get_login_token(pool, user_id)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::Custom("User not found".into()))?;

    if token.token != req.token {
        return Err(AppError::Custom("Invalid token".into()));
    }

    if token
        .token_expires_at
        .is_some_and(|expires_at| ServerTime::now_ms() > expires_at)
    {
        return Err(AppError::Custom("Token expired".into()));
    }

    Ok(LoginSession { user_id })
}

pub async fn start_session(
    conn: &mut ConnectionContext,
    session: LoginSession,
) -> Result<Vec<UserTask>, AppError> {
    conn.load_player(session.user_id).await?;
    let db = conn.state.db;
    if common::skip_tutorial() {
        conn.player()?.guide.skip_initial_tutorial(db).await?;
    }
    let now = ServerTime::now_ms();
    let today = ServerTime::server_day(now);
    let (is_new_day, _) = reconcile_periodic_resets_for_player(conn.player_mut()?, db, now).await?;
    let is_new_month = conn.player()?.state.is_new_month(now);
    if is_new_day {
        logic::profile::ProfileManager::new(session.user_id)
            .record_login_day(db)
            .await?;
    }
    conn.player()?
        .activity
        .sync_act101_login_progress(db, now)
        .await?;

    {
        let state = &mut conn.player_mut()?.state;
        state.last_login_timestamp = Some(now);
        if is_new_day {
            state.initial_login_complete = false;
            state.last_sign_in_day = today;
            state.month_card_claimed = false;
            state.last_month_card_claim_timestamp = None;
        }
        if is_new_month {
            state.last_monthly_reset_time = Some(now);
        }
        state.mark_login_complete(now);
        state.last_sign_in_time = Some(now);
    }

    let updated_tasks = conn.player_mut()?.tasks.sync_login(db, is_new_day).await?;
    logic::turnback::TurnbackManager::new(session.user_id)
        .sync_state(db, conn.state.tables)
        .await?;

    conn.save_player().await?;
    Ok(updated_tasks)
}

pub async fn reconcile_periodic_resets(
    conn: &mut ConnectionContext,
    now_ms: i64,
) -> Result<(bool, bool), AppError> {
    let db = conn.state.db;
    let periods = reconcile_periodic_resets_for_player(conn.player_mut()?, db, now_ms).await?;
    if periods.0 || periods.1 {
        conn.save_player().await?;
    }
    Ok(periods)
}

async fn reconcile_periodic_resets_for_player(
    player: &mut Player,
    db: &SqlitePool,
    now_ms: i64,
) -> Result<(bool, bool), AppError> {
    let is_new_day = player.state.is_new_server_day(now_ms);
    let is_new_week = player.state.is_new_week(now_ms);
    if !is_new_day && !is_new_week {
        return Ok((false, false));
    }

    player
        .sign_in
        .reset_counters(db, is_new_day, is_new_week)
        .await?;
    if is_new_day {
        player.state.last_daily_reset_time = Some(now_ms);
    }
    if is_new_week {
        player.state.last_weekly_reset_time = Some(now_ms);
    }
    player.state.updated_at = now_ms;
    Ok((is_new_day, is_new_week))
}

pub fn login_reply_payload(user_id: i64) -> Vec<u8> {
    login_payload("", user_id)
}

pub fn login_error_payload(reason: &str) -> Vec<u8> {
    login_payload(reason, 0)
}

fn login_payload(reason: &str, user_id: i64) -> Vec<u8> {
    let mut payload = Vec::with_capacity(2 + reason.len() + 8);
    payload.extend_from_slice(&(reason.len() as u16).to_be_bytes());
    payload.extend_from_slice(reason.as_bytes());
    payload.extend_from_slice(&user_id.to_be_bytes());
    payload
}

#[cfg(test)]
mod test;
