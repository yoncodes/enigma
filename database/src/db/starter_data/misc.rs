use super::*;

pub async fn load_dungeon_reward_points(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
) -> sqlx::Result<()> {
    let now = common::time::ServerTime::now_ms();
    sqlx::query(
        "INSERT INTO user_dungeon_reward_points
            (user_id, chapter_id, reward_point, created_at, updated_at)
         VALUES (?, 0, 0, ?, ?)
         ON CONFLICT(user_id, chapter_id) DO NOTHING",
    )
    .bind(user_id)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn load_starter_settings(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
) -> sqlx::Result<()> {
    for setting_type in crate::db::game::settings::DEFAULT_PUSH_SETTING_TYPES {
        sqlx::query("INSERT INTO user_setting_infos (user_id, type, param) VALUES (?, ?, '1')")
            .bind(user_id)
            .bind(setting_type)
            .execute(&mut **tx)
            .await?;
    }

    Ok(())
}

pub async fn load_starter_system_state(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
) -> sqlx::Result<()> {
    let trade_level = config::configs::get()
        .manufacture_building
        .iter()
        .map(|row| row.place_trade_level)
        .min()
        .unwrap_or_default();

    sqlx::query(
        "INSERT INTO user_manufacture_state (user_id, trade_level, updated_at) VALUES (?, ?, ?)",
    )
    .bind(user_id)
    .bind(trade_level)
    .bind(common::time::ServerTime::now_ms())
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        "INSERT INTO user_power_maker_state (user_id, next_remain_second, updated_at)
         VALUES (?, 43200, ?)",
    )
    .bind(user_id)
    .bind(common::time::ServerTime::now_ms())
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub async fn load_instruction_dungeon_info(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO user_instruction_dungeon_state (user_id, get_final_reward) VALUES (?, ?) ON CONFLICT(user_id) DO UPDATE SET get_final_reward = excluded.get_final_reward",
    )
    .bind(user_id)
    .bind(false)
    .execute(&mut **tx)
    .await?;

    tracing::info!("Loaded instruction dungeon info for user {}", user_id);
    Ok(())
}

/// Seed config-backed activity state that cannot be inferred from an absent row.
pub async fn load_activity_state(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
) -> sqlx::Result<()> {
    let now = common::time::ServerTime::now_ms();
    let tables = config::configs::get();

    let mut states = tables
        .activity101
        .iter()
        .map(|row| StarterActivityState {
            activity_id: row.activity_id,
            kind: ActivityStateKind::Act101Day,
            entry_id: row.id,
            state: 0,
            progress: 0,
            ext: String::new(),
        })
        .collect::<Vec<_>>();

    let mut activity101_ids = tables
        .activity101
        .iter()
        .map(|row| row.activity_id)
        .collect::<Vec<_>>();
    activity101_ids.sort_unstable();
    activity101_ids.dedup();

    states.extend(
        activity101_ids
            .into_iter()
            .map(|activity_id| StarterActivityState {
                activity_id,
                kind: ActivityStateKind::Act101Once,
                entry_id: 0,
                state: 0,
                progress: 0,
                ext: String::new(),
            }),
    );

    states.extend(
        tables
            .activity101_sp_bonus
            .iter()
            .map(|row| StarterActivityState {
                activity_id: row.activity_id,
                kind: ActivityStateKind::Act101SpBonus,
                entry_id: row.id,
                state: 0,
                progress: 0,
                ext: String::new(),
            }),
    );

    states.extend(
        tables
            .activity104_episode
            .iter()
            .map(|row| StarterActivityState {
                activity_id: row.activity_id,
                kind: ActivityStateKind::Act104Episode,
                entry_id: row.layer,
                state: 0,
                progress: 0,
                ext: String::new(),
            }),
    );

    states.extend(
        tables
            .activity104_special
            .iter()
            .map(|row| StarterActivityState {
                activity_id: row.activity_id,
                kind: ActivityStateKind::Act104Special,
                entry_id: row.layer,
                state: 0,
                progress: 0,
                ext: String::new(),
            }),
    );

    states.extend(tables.activity125.iter().map(|row| StarterActivityState {
        activity_id: row.activity_id,
        kind: ActivityStateKind::Act125Episode,
        entry_id: row.id,
        state: 0,
        progress: 0,
        ext: String::new(),
    }));

    states.extend(tables.activity146.iter().map(|row| StarterActivityState {
        activity_id: row.activity_id,
        kind: ActivityStateKind::Act146Episode,
        entry_id: row.id,
        state: 0,
        progress: 0,
        ext: String::new(),
    }));

    states.extend(
        tables
            .activity172_task
            .iter()
            .filter(|row| row.item_id != 0)
            .map(|row| StarterActivityState {
                activity_id: row.activity_id,
                kind: ActivityStateKind::Act172UseItemTask,
                entry_id: row.id,
                state: 0,
                progress: 0,
                ext: String::new(),
            }),
    );

    states.extend(
        tables
            .actvity186_task
            .iter()
            .map(|row| StarterActivityState {
                activity_id: row.activity_id,
                kind: ActivityStateKind::Act186Task,
                entry_id: row.id,
                state: 0,
                progress: 0,
                ext: String::new(),
            }),
    );

    states.extend(
        tables
            .activity160_mission
            .iter()
            .map(|row| StarterActivityState {
                activity_id: row.activity_id,
                kind: ActivityStateKind::Act160Mission,
                entry_id: row.id,
                state: i32::from(row.pre_id == 0),
                progress: 0,
                ext: String::new(),
            }),
    );

    states.extend(
        tables
            .activity165_story
            .iter()
            .map(|row| StarterActivityState {
                activity_id: row.activity_id,
                kind: ActivityStateKind::Act165Story,
                entry_id: row.story_id,
                state: i32::from(row.pre_element_id1 == 0),
                progress: row.first_step_id,
                ext: String::new(),
            }),
    );

    states.extend(
        tables
            .activity208_bonus
            .iter()
            .map(|row| StarterActivityState {
                activity_id: row.activity_id,
                kind: ActivityStateKind::Act208Bonus,
                entry_id: row.id,
                state: 0,
                progress: 0,
                ext: String::new(),
            }),
    );

    states.extend(
        tables
            .activity212_bonus
            .iter()
            .map(|row| StarterActivityState {
                activity_id: row.activity_id,
                kind: ActivityStateKind::Act212Bonus,
                entry_id: row.id,
                state: i32::from(row.id == 1),
                progress: 0,
                ext: String::new(),
            }),
    );

    let mut activity209_ids = tables
        .activity209_task
        .iter()
        .map(|row| row.activity_id)
        .collect::<Vec<_>>();
    activity209_ids.sort_unstable();
    activity209_ids.dedup();

    states.extend(
        activity209_ids
            .into_iter()
            .map(|activity_id| StarterActivityState {
                activity_id,
                kind: ActivityStateKind::Act209Layer,
                entry_id: 0,
                state: 0,
                progress: 0,
                ext: String::new(),
            }),
    );

    for state in states {
        insert_activity_state(tx, user_id, now, state).await?;
    }

    tracing::info!("Loaded activity state for user {}", user_id);
    Ok(())
}

struct StarterActivityState {
    activity_id: i32,
    kind: ActivityStateKind,
    entry_id: i32,
    state: i32,
    progress: i32,
    ext: String,
}

async fn insert_activity_state(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    now: i64,
    state: StarterActivityState,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT OR IGNORE INTO user_activity_state
            (user_id, activity_id, kind, entry_id, state, progress, ext, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(state.activity_id)
    .bind(state.kind.id())
    .bind(state.entry_id)
    .bind(state.state)
    .bind(state.progress)
    .bind(state.ext)
    .bind(now)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub async fn load_starter_bgm(tx: &mut Transaction<'_, Sqlite>, user_id: i64) -> sqlx::Result<()> {
    let unlock_time = common::time::ServerTime::now_sec_i32();
    let default_bgms = configs::get()
        .bgm_switch
        .iter()
        .filter(|bgm| bgm.default_unlock != 0)
        .collect::<Vec<_>>();
    let use_bgm_id = default_bgms
        .iter()
        .min_by_key(|bgm| (bgm.sort, bgm.id))
        .map(|bgm| bgm.id)
        .unwrap_or_default();

    for bgm in default_bgms {
        sqlx::query(
            "INSERT INTO user_bgm (player_id, bgm_id, unlock_time, is_favorite, is_read)
             VALUES (?, ?, ?, 0, 0)",
        )
        .bind(user_id)
        .bind(bgm.id)
        .bind(unlock_time)
        .execute(&mut **tx)
        .await?;
    }

    sqlx::query(
        "INSERT INTO user_bgm_state (player_id, use_bgm_id)
         VALUES (?, ?)",
    )
    .bind(user_id)
    .bind(use_bgm_id)
    .execute(&mut **tx)
    .await?;

    tracing::info!("Loaded starter bgm for user {}", user_id);

    Ok(())
}
