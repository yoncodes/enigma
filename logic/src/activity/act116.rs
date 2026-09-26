use crate::{error::AppError, reward};
use common::time::ServerTime;
use database::db::game::activity116;
use sonettobuf::{
    Act116Info, BuildTrapReply, Get116InfosReply, PutTrapReply, UpgradeElementReply,
    UserDungeonSpStatus,
};
use sqlx::SqlitePool;

pub struct Activity116Cost<T> {
    pub reply: T,
    pub item_ids: Vec<u32>,
    pub currency_ids: Vec<(i32, i32)>,
}

fn activity_id(
    activity_id: Option<i32>,
    tables: &config::GameDB,
    now_ms: i64,
) -> Result<i32, AppError> {
    activity_id
        .or_else(|| {
            let now_ms = u64::try_from(now_ms).ok()?;
            tables
                .activity
                .iter()
                .filter(|row| row.type_id == 116)
                .filter_map(|row| {
                    super::schedule::get(row.id)
                        .filter(|schedule| {
                            schedule.start_time <= now_ms && now_ms <= schedule.end_time
                        })
                        .map(|_| row.id)
                })
                .max()
        })
        .ok_or(AppError::InvalidRequest)
}

pub async fn info(
    db: &SqlitePool,
    player_id: i64,
    requested_id: Option<i32>,
    tables: &config::GameDB,
    now_ms: i64,
) -> Result<Get116InfosReply, AppError> {
    let activity_id = activity_id(requested_id, tables, now_ms)?;
    let state = activity116::get_or_create_state(db, player_id, activity_id).await?;
    let active_refresh_day = 2 - ServerTime::server_day(now_ms).rem_euclid(2) as i32;
    let refresh_time = u64::try_from(now_ms).unwrap_or_default();

    Ok(Get116InfosReply {
        activity_id: Some(activity_id),
        infos: state
            .elements
            .into_iter()
            .map(|(element_id, level)| Act116Info {
                element_id: Some(element_id),
                level: Some(level),
            })
            .collect(),
        trap_ids: state.trap_ids,
        put_trap: Some(state.put_trap),
        sp_status: tables
            .activity116_episode_sp
            .iter()
            .map(|episode| UserDungeonSpStatus {
                chapter_id: Some(episode.id / 100),
                episode_id: Some(episode.id),
                status: Some(if episode.refresh_day == active_refresh_day {
                    1
                } else {
                    2
                }),
                refresh_time: Some(refresh_time),
            })
            .collect(),
    })
}

pub async fn upgrade_element(
    db: &SqlitePool,
    player_id: i64,
    activity_id: i32,
    element_id: i32,
    tables: &config::GameDB,
) -> Result<Activity116Cost<UpgradeElementReply>, AppError> {
    let state = activity116::get_or_create_state(db, player_id, activity_id).await?;
    let current_level = state
        .elements
        .iter()
        .find_map(|(id, level)| (*id == element_id).then_some(*level))
        .unwrap_or_default();
    let next = tables
        .activity116_building
        .iter()
        .find(|row| row.element_id == element_id && row.level == current_level + 1)
        .ok_or(AppError::InvalidRequest)?;
    let mut tx = db.begin().await?;
    let consumed = reward::consume(&mut tx, player_id, &reward::parse(&next.cost)).await?;
    if !activity116::upgrade_element(&mut tx, player_id, activity_id, element_id, current_level)
        .await?
    {
        return Err(AppError::InvalidRequest);
    }
    tx.commit().await?;

    Ok(Activity116Cost {
        reply: UpgradeElementReply {
            activity_id: Some(activity_id),
            element_id: Some(element_id),
            level: Some(next.level),
        },
        item_ids: consumed.item_ids,
        currency_ids: consumed.currency_ids,
    })
}

pub async fn build_trap(
    db: &SqlitePool,
    player_id: i64,
    activity_id: i32,
    trap_id: i32,
    tables: &config::GameDB,
) -> Result<Activity116Cost<BuildTrapReply>, AppError> {
    let trap = tables
        .activity116_building
        .get(trap_id)
        .filter(|row| row.building_type == 3)
        .ok_or(AppError::InvalidRequest)?;
    let mut tx = db.begin().await?;
    let consumed = reward::consume(&mut tx, player_id, &reward::parse(&trap.cost)).await?;
    if !activity116::build_trap(&mut tx, player_id, activity_id, trap_id).await? {
        return Err(AppError::InvalidRequest);
    }
    tx.commit().await?;

    Ok(Activity116Cost {
        reply: BuildTrapReply {
            activity_id: Some(activity_id),
            trap_id: Some(trap_id),
        },
        item_ids: consumed.item_ids,
        currency_ids: consumed.currency_ids,
    })
}

pub async fn put_trap(
    db: &SqlitePool,
    player_id: i64,
    activity_id: i32,
    trap_id: i32,
) -> Result<PutTrapReply, AppError> {
    if !activity116::put_trap(db, player_id, activity_id, trap_id).await? {
        return Err(AppError::InvalidRequest);
    }
    Ok(PutTrapReply {
        activity_id: Some(activity_id),
        trap_id: Some(trap_id),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use database::db::game::currencies;

    #[tokio::test]
    async fn activity116_progress_persists_and_consumes_configured_costs() {
        let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("data/excel2json");
        config::init(data_dir.to_str().unwrap()).unwrap();
        let tables = config::configs::get();
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (1, 'act116-progress', 0, 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        currencies::add_currency(&pool, 1, 1202, 1_000)
            .await
            .unwrap();

        let captured_time = 1_790_424_560_190;
        let initial = info(&pool, 1, None, tables, captured_time).await.unwrap();
        assert_eq!(initial.activity_id, Some(11204));
        assert_eq!(
            initial
                .sp_status
                .iter()
                .filter(|status| status.status == Some(1))
                .filter_map(|status| status.episode_id)
                .collect::<Vec<_>>(),
            vec![1270104, 1270105, 1270106]
        );

        let upgrade = upgrade_element(&pool, 1, 11204, 12101021, tables)
            .await
            .unwrap();
        assert_eq!(upgrade.reply.level, Some(1));
        let built = build_trap(&pool, 1, 11204, 10301, tables).await.unwrap();
        assert_eq!(built.reply.trap_id, Some(10301));
        assert!(put_trap(&pool, 1, 11204, 10301).await.is_ok());

        let saved = info(&pool, 1, Some(11204), tables, captured_time)
            .await
            .unwrap();
        assert_eq!(saved.infos[0].level, Some(1));
        assert_eq!(saved.trap_ids, vec![10301]);
        assert_eq!(saved.put_trap, Some(10301));
        assert_eq!(
            currencies::get_currency(&pool, 1, 1202)
                .await
                .unwrap()
                .unwrap()
                .quantity,
            750
        );
        assert!(build_trap(&pool, 1, 11204, 10301, tables).await.is_err());
        assert_eq!(
            currencies::get_currency(&pool, 1, 1202)
                .await
                .unwrap()
                .unwrap()
                .quantity,
            750
        );
    }
}
