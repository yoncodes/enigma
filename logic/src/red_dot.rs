use crate::{bp::BattlePassManager, error::AppError, types::red_dot_id::RedDotId};
use database::{
    db::game::{achievements, act233_bp, activity101, mail, red_dots, room_ob, tasks as task_db},
    models::game::red_dots::RedDotRecord,
};
use sonettobuf::{GetRedDotInfosReply, RedDotGroup, RedDotInfo, ShowRedDotReply};
use sqlx::SqlitePool;
use std::{
    collections::{BTreeSet, HashMap, HashSet, VecDeque},
    sync::OnceLock,
};

static RED_DOT_CHILDREN: OnceLock<HashMap<i32, Vec<i32>>> = OnceLock::new();
const ROOM_CHARACTER_FAITH_TOTAL_MINUTES_CONFIG_ID: i32 = 109;

#[derive(Clone, Copy, Debug)]
pub struct RedDotManager {
    player_id: i64,
}

impl RedDotManager {
    pub fn new(player_id: i64) -> Self {
        Self { player_id }
    }

    pub async fn infos(
        &self,
        db: &SqlitePool,
        ids: Vec<i32>,
    ) -> Result<GetRedDotInfosReply, AppError> {
        get_red_dot_infos(db, self.player_id, ids).await
    }

    pub async fn show(
        &self,
        db: &SqlitePool,
        define_id: i32,
        is_visible: bool,
    ) -> Result<(ShowRedDotReply, Vec<i32>), AppError> {
        show_red_dot(db, self.player_id, define_id, is_visible).await
    }

    pub async fn mail_state(&self, db: &SqlitePool) -> Result<(i32, i32), AppError> {
        sync_mail_red_dot(db, self.player_id).await
    }

    pub async fn trade_order_value(&self, db: &SqlitePool) -> Result<i32, AppError> {
        trade_order_red_dot_value(db, self.player_id).await
    }

    pub async fn battle_pass_groups(&self, db: &SqlitePool) -> Result<Vec<RedDotGroup>, AppError> {
        battle_pass_red_dot_groups(db, self.player_id).await
    }

    pub async fn act233_groups(
        &self,
        db: &SqlitePool,
        activity_id: i32,
    ) -> Result<Vec<RedDotGroup>, AppError> {
        act233_red_dot_groups(db, self.player_id, activity_id).await
    }

    pub async fn hide_infos(
        &self,
        db: &SqlitePool,
        define_id: i32,
        info_ids: Vec<i32>,
    ) -> Result<(), AppError> {
        red_dots::hide_red_dot_infos(db, self.player_id, define_id, info_ids).await?;
        Ok(())
    }

    pub async fn hide_visible_info(
        &self,
        db: &SqlitePool,
        define_id: i32,
        info_id: i32,
    ) -> Result<Option<RedDotRecord>, AppError> {
        Ok(red_dots::hide_visible_red_dot_info(db, self.player_id, define_id, info_id).await?)
    }
}

async fn get_red_dot_infos(
    db: &SqlitePool,
    player_id: i64,
    requested_ids: Vec<i32>,
) -> Result<GetRedDotInfosReply, AppError> {
    let children = red_dot_children()?;
    let ids = if requested_ids.is_empty() {
        loadable_leaf_ids(
            config::configs::get()
                .reddot
                .iter()
                .map(|row| (row.id, row.is_online, row.can_load)),
            children,
        )
    } else {
        expand_requested_ids(&requested_ids)?
    };
    let mut reply = GetRedDotInfosReply {
        red_dot_infos: Vec::new(),
    };

    apply_dynamic_red_dots(&mut reply, db, player_id, ids.clone()).await?;
    apply_state_overlay(&mut reply, db, player_id, ids.clone()).await?;

    if !ids.is_empty() {
        reply
            .red_dot_infos
            .retain(|group| ids.contains(&group.define_id));
    }
    add_missing_leaf_groups(&mut reply, &ids, children);

    Ok(reply)
}

async fn show_red_dot(
    db: &SqlitePool,
    player_id: i64,
    define_id: i32,
    is_visible: bool,
) -> Result<(ShowRedDotReply, Vec<i32>), AppError> {
    let changed_info_ids = if is_visible {
        Vec::new()
    } else {
        let info_ids = current_info_ids(db, player_id, define_id).await?;
        red_dots::hide_red_dot_infos(db, player_id, define_id, info_ids).await?
    };

    Ok((ShowRedDotReply {}, changed_info_ids))
}

async fn sync_mail_red_dot(db: &SqlitePool, player_id: i64) -> Result<(i32, i32), AppError> {
    let unread = mail::unread_count(db, player_id).await?;
    let value = if unread > 0 { 1 } else { 0 };
    let time = if unread > 0 {
        (common::time::ServerTime::now_ms() / 1000) as i32
    } else {
        0
    };

    Ok((value, time))
}

async fn apply_dynamic_red_dots(
    reply: &mut GetRedDotInfosReply,
    db: &SqlitePool,
    player_id: i64,
    requested_ids: Vec<i32>,
) -> Result<(), AppError> {
    let ids = if requested_ids.is_empty() {
        vec![
            RedDotId::AchievementFinish,
            RedDotId::ActivityNoviceTab,
            RedDotId::ActivityJieXiKaPhoto,
            RedDotId::Activity239Bonus,
            RedDotId::V3a7Anniversary3ActBpSubTask,
            RedDotId::V3a7Anniversary3ActBpBonus,
            RedDotId::BattlePassBonus,
            RedDotId::BattlePassSpBonus,
            RedDotId::BattlePassTask,
            RedDotId::DailyTask,
            RedDotId::MailBtn,
            RedDotId::TradeOrderFulfillable,
            RedDotId::WeeklyTask,
        ]
    } else {
        requested_ids
            .into_iter()
            .filter_map(RedDotId::from_id)
            .collect()
    };

    if ids.contains(&RedDotId::BattlePassBonus) || ids.contains(&RedDotId::BattlePassSpBonus) {
        let bonus = BattlePassManager::new(player_id).bonus_red_dots(db).await?;
        if ids.contains(&RedDotId::BattlePassBonus) {
            replace_group(
                reply,
                RedDotId::BattlePassBonus.id(),
                vec![red_dot_info(0, bonus.normal)],
            );
        }
        if ids.contains(&RedDotId::BattlePassSpBonus) {
            replace_group(
                reply,
                RedDotId::BattlePassSpBonus.id(),
                vec![red_dot_info(0, bonus.sp)],
            );
        }
    }

    if ids.contains(&RedDotId::V3a7Anniversary3ActBpSubTask)
        || ids.contains(&RedDotId::V3a7Anniversary3ActBpBonus)
    {
        let (task_infos, bonus) = act233_red_dot_state(db, player_id, None).await?;
        if ids.contains(&RedDotId::V3a7Anniversary3ActBpSubTask) {
            replace_group(
                reply,
                RedDotId::V3a7Anniversary3ActBpSubTask.id(),
                task_infos,
            );
        }
        if ids.contains(&RedDotId::V3a7Anniversary3ActBpBonus) {
            replace_group(
                reply,
                RedDotId::V3a7Anniversary3ActBpBonus.id(),
                vec![red_dot_info(0, bonus)],
            );
        }
    }

    for id in ids {
        match id {
            RedDotId::AchievementFinish => {
                apply_achievement_finish_red_dot(reply, db, player_id).await?
            }
            RedDotId::ActivityNoviceTab => apply_activity101_red_dot(reply, db, player_id).await?,
            RedDotId::ActivityJieXiKaPhoto => {}
            RedDotId::Activity239Bonus => {
                for group in act239_red_dot_groups(db, player_id).await? {
                    replace_group(reply, group.define_id, group.infos);
                }
            }
            RedDotId::V3a7Anniversary3ActBpSubTask | RedDotId::V3a7Anniversary3ActBpBonus => {}
            RedDotId::BattlePassBonus | RedDotId::BattlePassSpBonus => {}
            RedDotId::BattlePassTask => apply_bp_task_red_dot(reply, db, player_id).await?,
            RedDotId::BossRushRankBonus => {}
            RedDotId::CommandStationBonus => {}
            RedDotId::DailyTask => {
                apply_task_red_dot(reply, db, player_id, task_db::TaskType::Daily).await?
            }
            RedDotId::MailBtn => apply_mail_red_dot(reply, db, player_id).await?,
            RedDotId::PlayerChangeBgItemNew => {}
            RedDotId::RoomCharacterFaithGetFull => {
                apply_room_faith_get_full_red_dot(reply, db, player_id).await?
            }
            RedDotId::RoomBlockPackage
            | RedDotId::RoomBuildingPlace
            | RedDotId::RoomCharacterFaithFull => {}
            RedDotId::RoomProductionFull => {
                apply_room_production_full_red_dot(reply, db, player_id).await?
            }
            RedDotId::StoreChargeGoodsRead => {}
            RedDotId::StoreGoodsRead => {}
            RedDotId::StoreTab => {}
            RedDotId::TeachingSystem => {}
            RedDotId::TradeOrderFulfillable => {
                apply_trade_order_red_dot(reply, db, player_id).await?
            }
            RedDotId::TurnbackBattlePassBonus
            | RedDotId::TurnbackBattlePassTask
            | RedDotId::TurnbackDailyBonus
            | RedDotId::TurnbackLegacyTask
            | RedDotId::TurnbackOnceBonus
            | RedDotId::TurnbackSignIn => {}
            RedDotId::WeeklyTask => {
                apply_task_red_dot(reply, db, player_id, task_db::TaskType::Weekly).await?
            }
        }
    }

    Ok(())
}

async fn apply_trade_order_red_dot(
    reply: &mut GetRedDotInfosReply,
    db: &SqlitePool,
    player_id: i64,
) -> Result<(), AppError> {
    replace_group(
        reply,
        RedDotId::TradeOrderFulfillable.id(),
        vec![RedDotInfo {
            id: 0,
            value: trade_order_red_dot_value(db, player_id).await?,
            time: Some(0),
            ext: None,
        }],
    );
    Ok(())
}

async fn trade_order_red_dot_value(db: &SqlitePool, player_id: i64) -> Result<i32, AppError> {
    Ok(i32::from(
        database::db::game::room_orders::has_fulfillable_purchase_order(
            db,
            player_id,
            config::configs::get(),
        )
        .await?,
    ))
}

async fn apply_task_red_dot(
    reply: &mut GetRedDotInfosReply,
    db: &SqlitePool,
    player_id: i64,
    task_type: task_db::TaskType,
) -> Result<(), AppError> {
    let expiry = task_db::claimable_expiry(db, player_id, task_type).await?;
    let define_id = match task_type {
        task_db::TaskType::Daily => RedDotId::DailyTask.id(),
        task_db::TaskType::Weekly => RedDotId::WeeklyTask.id(),
        _ => return Ok(()),
    };
    replace_group(
        reply,
        define_id,
        vec![RedDotInfo {
            id: 0,
            value: i32::from(expiry.is_some()),
            time: Some(expiry.unwrap_or_default()),
            ext: None,
        }],
    );
    Ok(())
}

async fn apply_room_faith_get_full_red_dot(
    reply: &mut GetRedDotInfosReply,
    db: &SqlitePool,
    player_id: i64,
) -> Result<(), AppError> {
    let total_minutes = config::configs::get()
        .r#const
        .get(ROOM_CHARACTER_FAITH_TOTAL_MINUTES_CONFIG_ID)
        .and_then(|row| row.value.parse().ok())
        .ok_or(AppError::InvalidRequest)?;
    let hero_ids = room_ob::full_faith_hero_ids(db, player_id, total_minutes).await?;
    let infos = if hero_ids.is_empty() {
        vec![red_dot_info(0, 0)]
    } else {
        hero_ids
            .into_iter()
            .map(|hero_id| red_dot_info(i64::from(hero_id), 1))
            .collect()
    };
    replace_group(reply, RedDotId::RoomCharacterFaithGetFull.id(), infos);
    Ok(())
}

async fn apply_room_production_full_red_dot(
    reply: &mut GetRedDotInfosReply,
    db: &SqlitePool,
    player_id: i64,
) -> Result<(), AppError> {
    let lines = room_ob::get_production_lines(db, player_id, &[]).await?;
    let infos = if lines.is_empty() {
        vec![red_dot_info(0, 0)]
    } else {
        lines
            .into_iter()
            .map(|line| {
                let value = i32::from(room_ob::production_line_is_full(&line));
                red_dot_info(i64::from(line.line_id), value)
            })
            .collect()
    };
    replace_group(reply, RedDotId::RoomProductionFull.id(), infos);
    Ok(())
}

async fn apply_achievement_finish_red_dot(
    reply: &mut GetRedDotInfosReply,
    db: &SqlitePool,
    player_id: i64,
) -> Result<(), AppError> {
    let tables = config::configs::get();
    let categories = achievements::get_achievements(db, player_id)
        .await?
        .into_iter()
        .filter(|achievement| achievement.has_finish && achievement.is_new)
        .filter_map(|achievement| {
            let achievement_id = tables
                .achievement_task
                .get(achievement.achievement_id)?
                .achievement_id;
            tables
                .achievement
                .get(achievement_id)
                .map(|achievement| achievement.category)
        })
        .collect::<BTreeSet<_>>();
    let group = get_or_add_group(reply, RedDotId::AchievementFinish.id(), true);
    group.infos = if categories.is_empty() {
        vec![RedDotInfo {
            id: 0,
            value: 0,
            time: Some(0),
            ext: None,
        }]
    } else {
        categories
            .into_iter()
            .map(|category| RedDotInfo {
                id: i64::from(category),
                value: 1,
                time: Some(0),
                ext: None,
            })
            .collect()
    };
    group.replace_all = Some(true);
    Ok(())
}

async fn apply_activity101_red_dot(
    reply: &mut GetRedDotInfosReply,
    db: &SqlitePool,
    player_id: i64,
) -> Result<(), AppError> {
    let mut activity_ids = config::configs::get()
        .activity101
        .iter()
        .map(|row| row.activity_id)
        .collect::<Vec<_>>();
    activity_ids.sort_unstable();
    activity_ids.dedup();

    let group = get_or_add_group(reply, RedDotId::ActivityNoviceTab.id(), true);
    let activity_set = activity_ids
        .iter()
        .map(|id| *id as i64)
        .collect::<HashSet<_>>();
    group.infos.retain(|info| !activity_set.contains(&info.id));

    for activity_id in activity_ids {
        let (infos, _, _) = activity101::get_activity101_info(db, player_id, activity_id).await?;
        group.infos.push(RedDotInfo {
            id: activity_id as i64,
            value: if infos.iter().any(|(_, state)| *state == 1) {
                1
            } else {
                0
            },
            time: Some(0),
            ext: None,
        });
    }

    Ok(())
}

async fn act239_red_dot_groups(
    db: &SqlitePool,
    player_id: i64,
) -> Result<Vec<RedDotGroup>, AppError> {
    Ok(crate::activity::act239_red_dot_entries(db, player_id)
        .await?
        .into_iter()
        .map(|(define_id, info_ids)| RedDotGroup {
            define_id,
            infos: if info_ids.is_empty() {
                vec![red_dot_info(0, 0)]
            } else {
                info_ids
                    .into_iter()
                    .map(|info_id| red_dot_info(i64::from(info_id), 1))
                    .collect()
            },
            replace_all: Some(true),
        })
        .collect())
}

async fn apply_mail_red_dot(
    reply: &mut GetRedDotInfosReply,
    db: &SqlitePool,
    player_id: i64,
) -> Result<(), AppError> {
    let (value, time) = sync_mail_red_dot(db, player_id).await?;
    replace_group(
        reply,
        RedDotId::MailBtn.id(),
        vec![RedDotInfo {
            id: 0,
            value,
            time: Some(time),
            ext: None,
        }],
    );
    Ok(())
}

async fn apply_bp_task_red_dot(
    reply: &mut GetRedDotInfosReply,
    db: &SqlitePool,
    player_id: i64,
) -> Result<(), AppError> {
    replace_group(
        reply,
        RedDotId::BattlePassTask.id(),
        BattlePassManager::new(player_id)
            .task_red_dot_infos(db)
            .await?,
    );
    Ok(())
}

async fn act233_red_dot_groups(
    db: &SqlitePool,
    player_id: i64,
    activity_id: i32,
) -> Result<Vec<RedDotGroup>, AppError> {
    let (task_infos, bonus) = act233_red_dot_state(db, player_id, Some(activity_id)).await?;

    Ok(vec![
        RedDotGroup {
            define_id: RedDotId::V3a7Anniversary3ActBpSubTask.id(),
            infos: task_infos,
            replace_all: Some(true),
        },
        RedDotGroup {
            define_id: RedDotId::V3a7Anniversary3ActBpBonus.id(),
            infos: vec![red_dot_info(0, bonus)],
            replace_all: Some(true),
        },
    ])
}

async fn act233_red_dot_state(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
) -> Result<(Vec<RedDotInfo>, i32), AppError> {
    let tables = config::configs::get();
    let passes = tables
        .activity233_bp
        .iter()
        .filter(|bp| activity_id.is_none_or(|activity_id| bp.activity_id == activity_id))
        .filter(|bp| bp.exp_level_up > 0)
        .map(|bp| (bp.activity_id, bp.bp_id, bp.exp_level_up))
        .collect::<Vec<_>>();
    if passes.is_empty() {
        return Ok((vec![red_dot_info(0, 0)], 0));
    }

    task_db::ensure_tasks_for_type(db, player_id, task_db::TaskType::ActBp).await?;
    let mut task_counts = HashMap::<i64, i32>::new();
    let mut bonus_ready = false;

    for (activity_id, bp_id, exp_level_up) in passes {
        let state = act233_bp::get_or_create_state(db, player_id, activity_id, bp_id).await?;
        let unlocked_level = (state.score / exp_level_up).max(0);
        bonus_ready |= tables
            .activity233_lv_bonus
            .iter()
            .filter(|bonus| bonus.bp_id == bp_id && bonus.level <= unlocked_level)
            .any(|bonus| {
                (!bonus.free_bonus.is_empty() && !state.has_get_free_bonus.contains(&bonus.level))
                    || (state.pay_status > 0
                        && !bonus.pay_bonus.is_empty()
                        && !state.has_get_pay_bonus.contains(&bonus.level))
            });

        let mut task_configs = HashMap::new();
        for task in tables.activity233_task.iter().filter(|task| {
            task.activity_id == activity_id && task.bp_id == bp_id && task.is_online != 0
        }) {
            let prepose = if task.prepose.trim().is_empty() {
                None
            } else {
                Some(
                    task.prepose
                        .trim()
                        .parse::<i32>()
                        .map_err(|_| AppError::InvalidRequest)?,
                )
            };
            task_configs.insert(task.id, (task.loop_type, task.max_progress, prepose));
        }
        let tasks = task_db::list_act_bp(db, player_id, activity_id).await?;
        for task in &tasks {
            let Some((loop_type, max_progress, prepose)) = task_configs.get(&task.task_id) else {
                continue;
            };
            if task.progress < *max_progress || task.finish_count != 0 {
                continue;
            }
            if prepose.is_some_and(|prepose| {
                !tasks
                    .iter()
                    .any(|task| task.task_id == prepose && task.finish_count > 0)
            }) {
                continue;
            }
            let loop_type = task_db::TaskLoopType::from_id(*loop_type)
                .unwrap_or(task_db::TaskLoopType::Permanent);
            let info_id = match loop_type {
                task_db::TaskLoopType::Appoint => task_db::TaskLoopType::Permanent.id() as i64,
                other => other.id() as i64,
            };
            *task_counts.entry(info_id).or_default() += 1;
        }
    }

    let mut task_infos = task_counts
        .into_iter()
        .map(|(id, value)| red_dot_info(id, value))
        .collect::<Vec<_>>();
    task_infos.sort_unstable_by_key(|info| info.id);
    if task_infos.is_empty() {
        task_infos.push(red_dot_info(0, 0));
    }

    Ok((task_infos, i32::from(bonus_ready)))
}

async fn battle_pass_red_dot_groups(
    db: &SqlitePool,
    player_id: i64,
) -> Result<Vec<RedDotGroup>, AppError> {
    let manager = BattlePassManager::new(player_id);
    let bonus = manager.bonus_red_dots(db).await?;
    let task_infos = manager.task_red_dot_infos(db).await?;

    Ok(vec![
        RedDotGroup {
            define_id: RedDotId::BattlePassTask.id(),
            infos: if task_infos.is_empty() {
                vec![red_dot_info(0, 0)]
            } else {
                task_infos
            },
            replace_all: Some(true),
        },
        RedDotGroup {
            define_id: RedDotId::BattlePassBonus.id(),
            infos: vec![red_dot_info(0, bonus.normal)],
            replace_all: Some(true),
        },
        RedDotGroup {
            define_id: RedDotId::BattlePassSpBonus.id(),
            infos: vec![red_dot_info(0, bonus.sp)],
            replace_all: Some(true),
        },
    ])
}

fn red_dot_info(id: i64, value: i32) -> RedDotInfo {
    RedDotInfo {
        id,
        value,
        time: Some(0),
        ext: None,
    }
}

async fn apply_state_overlay(
    reply: &mut GetRedDotInfosReply,
    db: &SqlitePool,
    player_id: i64,
    requested_ids: Vec<i32>,
) -> Result<(), AppError> {
    let define_ids = if requested_ids.is_empty() {
        reply
            .red_dot_infos
            .iter()
            .map(|group| group.define_id)
            .collect::<Vec<_>>()
    } else {
        requested_ids
    };
    let states = red_dots::get_red_dots_by_defines(db, player_id, define_ids).await?;

    for state in states {
        apply_state(reply, state);
    }

    Ok(())
}

fn apply_state(reply: &mut GetRedDotInfosReply, state: RedDotRecord) {
    let Some(group_index) = reply
        .red_dot_infos
        .iter()
        .position(|group| group.define_id == state.define_id)
    else {
        if state.value != 0 {
            let replace_all = state.replace_all;
            reply.red_dot_infos.push(RedDotGroup {
                define_id: state.define_id,
                infos: vec![state.into()],
                replace_all: Some(replace_all),
            });
        }
        return;
    };

    let group = &mut reply.red_dot_infos[group_index];
    if state.value == 0 {
        group.infos.retain(|info| info.id != state.info_id as i64);
    } else if let Some(info) = group
        .infos
        .iter_mut()
        .find(|info| info.id == state.info_id as i64)
    {
        *info = state.into();
    } else {
        group.infos.push(state.into());
    }

    if group.infos.is_empty() {
        reply.red_dot_infos.remove(group_index);
    }
}

fn replace_group(reply: &mut GetRedDotInfosReply, define_id: i32, infos: Vec<RedDotInfo>) {
    reply
        .red_dot_infos
        .retain(|group| group.define_id != define_id);
    if !infos.is_empty() {
        reply.red_dot_infos.push(RedDotGroup {
            define_id,
            infos,
            replace_all: Some(true),
        });
    }
}

fn get_or_add_group(
    reply: &mut GetRedDotInfosReply,
    define_id: i32,
    replace_all: bool,
) -> &mut RedDotGroup {
    if let Some(index) = reply
        .red_dot_infos
        .iter()
        .position(|group| group.define_id == define_id)
    {
        return &mut reply.red_dot_infos[index];
    }

    reply.red_dot_infos.push(RedDotGroup {
        define_id,
        infos: Vec::new(),
        replace_all: Some(replace_all),
    });

    reply.red_dot_infos.last_mut().expect("red dot group added")
}

async fn current_info_ids(
    db: &SqlitePool,
    player_id: i64,
    define_id: i32,
) -> Result<Vec<i32>, AppError> {
    let mut reply = GetRedDotInfosReply {
        red_dot_infos: Vec::new(),
    };
    apply_dynamic_red_dots(&mut reply, db, player_id, vec![define_id]).await?;

    let ids = reply
        .red_dot_infos
        .into_iter()
        .find(|group| group.define_id == define_id)
        .map(|group| {
            group
                .infos
                .into_iter()
                .filter(|info| info.value > 0)
                .map(|info| info.id as i32)
                .collect::<Vec<_>>()
        })
        .filter(|ids| !ids.is_empty())
        .unwrap_or_else(|| vec![0]);

    Ok(ids)
}

fn expand_requested_ids(ids: &[i32]) -> Result<Vec<i32>, AppError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let children = red_dot_children()?;
    let mut expanded = ids.iter().copied().collect::<HashSet<_>>();
    let mut pending = ids.iter().copied().collect::<VecDeque<_>>();

    while let Some(id) = pending.pop_front() {
        let Some(child_ids) = children.get(&id) else {
            continue;
        };

        for child_id in child_ids {
            if expanded.insert(*child_id) {
                pending.push_back(*child_id);
            }
        }
    }

    Ok(expanded.into_iter().collect())
}

fn red_dot_children() -> Result<&'static HashMap<i32, Vec<i32>>, AppError> {
    if let Some(children) = RED_DOT_CHILDREN.get() {
        return Ok(children);
    }

    let children = build_red_dot_children(
        config::configs::get()
            .reddot
            .iter()
            .map(|row| (row.id, row.parent.as_str())),
    );
    let _ = RED_DOT_CHILDREN.set(children);

    Ok(RED_DOT_CHILDREN
        .get()
        .expect("red dot children initialized"))
}

fn build_red_dot_children<'a>(
    rows: impl IntoIterator<Item = (i32, &'a str)>,
) -> HashMap<i32, Vec<i32>> {
    let mut children = HashMap::<i32, Vec<i32>>::new();
    for (id, parent) in rows {
        for parent_id in parent
            .split('#')
            .filter_map(|part| part.parse::<i32>().ok())
        {
            children.entry(parent_id).or_default().push(id);
        }
    }

    children
}

fn loadable_leaf_ids(
    rows: impl IntoIterator<Item = (i32, i32, i32)>,
    children: &HashMap<i32, Vec<i32>>,
) -> Vec<i32> {
    rows.into_iter()
        .filter_map(|(id, is_online, can_load)| {
            (is_online != 0 && can_load != 0 && !children.contains_key(&id)).then_some(id)
        })
        .collect()
}

fn add_missing_leaf_groups(
    reply: &mut GetRedDotInfosReply,
    requested_ids: &[i32],
    children: &HashMap<i32, Vec<i32>>,
) {
    for define_id in requested_ids {
        if !children.contains_key(define_id)
            && !reply
                .red_dot_infos
                .iter()
                .any(|group| group.define_id == *define_id)
        {
            replace_group(reply, *define_id, vec![red_dot_info(0, 0)]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{add_missing_leaf_groups, apply_state, build_red_dot_children, loadable_leaf_ids};
    use crate::types::red_dot_id::RedDotId;
    use database::models::game::red_dots::RedDotRecord;
    use sonettobuf::{GetRedDotInfosReply, RedDotGroup, RedDotInfo};
    use sqlx::sqlite::SqlitePoolOptions;

    #[test]
    fn parses_parent_links_from_reddot_table() {
        let children = build_red_dot_children([
            (13, ""),
            (1013, "13#1012#1052"),
            (1042, "13#1051"),
            (1902, "13"),
        ]);

        assert_eq!(children.get(&13).unwrap(), &vec![1013, 1042, 1902]);

        let mut reply = GetRedDotInfosReply {
            red_dot_infos: Vec::new(),
        };
        add_missing_leaf_groups(&mut reply, &[13, 1013, 1042, 1902, 2307], &children);
        assert_eq!(
            reply
                .red_dot_infos
                .iter()
                .map(|group| group.define_id)
                .collect::<Vec<_>>(),
            vec![1013, 1042, 1902, 2307]
        );
        assert!(
            reply
                .red_dot_infos
                .iter()
                .all(|group| group.infos[0].value == 0)
        );
    }

    #[test]
    fn full_catalog_only_loads_online_server_leaf_nodes() {
        let children = build_red_dot_children([(1, ""), (2, "1"), (3, "")]);

        assert_eq!(
            loadable_leaf_ids([(1, 1, 1), (2, 1, 1), (3, 0, 1), (4, 1, 0)], &children),
            vec![2]
        );
    }

    #[test]
    fn state_zero_removes_static_info() {
        let mut reply = GetRedDotInfosReply {
            red_dot_infos: vec![RedDotGroup {
                define_id: 1002,
                infos: vec![RedDotInfo {
                    id: 0,
                    value: 1,
                    time: Some(1),
                    ext: Some(String::new()),
                }],
                replace_all: Some(true),
            }],
        };

        apply_state(
            &mut reply,
            RedDotRecord {
                id: 1,
                player_id: 1,
                define_id: 1002,
                info_id: 0,
                value: 0,
                time: 0,
                ext: String::new(),
                replace_all: false,
                created_at: 0,
                updated_at: 0,
            },
        );

        assert!(reply.red_dot_infos.is_empty());
    }

    #[test]
    fn empty_replacement_removes_bp_task_red_dot() {
        let mut reply = GetRedDotInfosReply {
            red_dot_infos: vec![RedDotGroup {
                define_id: RedDotId::BattlePassTask.id(),
                infos: vec![RedDotInfo {
                    id: 0,
                    value: 1,
                    time: Some(0),
                    ext: Some(String::new()),
                }],
                replace_all: Some(true),
            }],
        };

        super::replace_group(&mut reply, RedDotId::BattlePassTask.id(), vec![]);

        assert!(reply.red_dot_infos.is_empty());
    }

    #[tokio::test]
    async fn achievement_red_dot_uses_remaining_new_categories() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        database::run_migrations(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, created_at, updated_at) VALUES (1, 'red-dot', 0, 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        for (id, is_new) in [(41040201, false), (41170101, true)] {
            sqlx::query(
                "INSERT INTO user_achievements
                 (user_id, achievement_id, progress, has_finish, is_new, finish_time, created_at, updated_at)
                 VALUES (1, ?, 1, 1, ?, 1, 1, 1)",
            )
            .bind(id)
            .bind(is_new)
            .execute(&pool)
            .await
            .unwrap();
        }

        let manager = super::RedDotManager::new(1);
        let reply = manager
            .infos(&pool, vec![RedDotId::AchievementFinish.id()])
            .await
            .unwrap();
        assert_eq!(reply.red_dot_infos[0].infos[0].id, 4);

        database::db::game::achievements::clear_new_flags(&pool, 1, vec![41170101])
            .await
            .unwrap();
        let reply = manager
            .infos(&pool, vec![RedDotId::AchievementFinish.id()])
            .await
            .unwrap();
        assert_eq!(
            (
                reply.red_dot_infos[0].infos[0].id,
                reply.red_dot_infos[0].infos[0].value
            ),
            (0, 0)
        );
    }

    #[tokio::test]
    async fn act233_red_dots_follow_claimable_tasks_and_bonus_state() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        database::run_migrations(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (6, 'act233-dots', 0, 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        let pass = config::configs::get().activity233_bp.iter().next().unwrap();

        let (task_infos, bonus) = super::act233_red_dot_state(&pool, 6, Some(pass.activity_id))
            .await
            .unwrap();
        assert_eq!((task_infos[0].id, bonus), (0, 0));

        sqlx::query(
            "UPDATE user_act233_bp_state SET score = ?
             WHERE user_id = 6 AND activity_id = ? AND bp_id = ?",
        )
        .bind(pass.exp_level_up * 2)
        .bind(pass.activity_id)
        .bind(pass.bp_id)
        .execute(&pool)
        .await
        .unwrap();
        let (_, bonus) = super::act233_red_dot_state(&pool, 6, Some(pass.activity_id))
            .await
            .unwrap();
        assert_eq!(bonus, 1);

        sqlx::query(
            "UPDATE user_act233_bp_state SET has_get_free_bonus = '[1,2]'
             WHERE user_id = 6 AND activity_id = ? AND bp_id = ?",
        )
        .bind(pass.activity_id)
        .bind(pass.bp_id)
        .execute(&pool)
        .await
        .unwrap();
        let (_, bonus) = super::act233_red_dot_state(&pool, 6, Some(pass.activity_id))
            .await
            .unwrap();
        assert_eq!(bonus, 0);

        let task = config::configs::get()
            .activity233_task
            .iter()
            .find(|task| {
                task.activity_id == pass.activity_id
                    && task.bp_id == pass.bp_id
                    && task.is_online != 0
            })
            .unwrap();
        sqlx::query(
            "UPDATE user_tasks SET progress = 0, finish_count = 0
             WHERE user_id = 6 AND type_id = ? AND activity_id = ?",
        )
        .bind(database::db::game::tasks::TaskType::ActBp.id())
        .bind(pass.activity_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE user_tasks SET progress = ?
             WHERE user_id = 6 AND type_id = ? AND task_id = ?",
        )
        .bind(task.max_progress)
        .bind(database::db::game::tasks::TaskType::ActBp.id())
        .bind(task.id)
        .execute(&pool)
        .await
        .unwrap();

        let (task_infos, _) = super::act233_red_dot_state(&pool, 6, Some(pass.activity_id))
            .await
            .unwrap();
        let loop_type = database::db::game::tasks::TaskLoopType::from_id(task.loop_type)
            .unwrap_or(database::db::game::tasks::TaskLoopType::Permanent);
        let expected_id = match loop_type {
            database::db::game::tasks::TaskLoopType::Appoint => {
                database::db::game::tasks::TaskLoopType::Permanent.id() as i64
            }
            other => other.id() as i64,
        };
        assert_eq!(task_infos, vec![super::red_dot_info(expected_id, 1)]);

        let successor = config::configs::get()
            .activity233_task
            .iter()
            .find(|task| {
                task.activity_id == pass.activity_id
                    && task.bp_id == pass.bp_id
                    && !task.prepose.is_empty()
            })
            .unwrap();
        let predecessor_id = successor.prepose.parse::<i32>().unwrap();
        let successor_loop = database::db::game::tasks::TaskLoopType::from_id(successor.loop_type)
            .unwrap_or(database::db::game::tasks::TaskLoopType::Permanent);
        let successor_info_id = match successor_loop {
            database::db::game::tasks::TaskLoopType::Appoint => {
                database::db::game::tasks::TaskLoopType::Permanent.id() as i64
            }
            other => other.id() as i64,
        };
        sqlx::query(
            "UPDATE user_tasks SET progress = 0, finish_count = 0
             WHERE user_id = 6 AND type_id = ? AND activity_id = ?",
        )
        .bind(database::db::game::tasks::TaskType::ActBp.id())
        .bind(pass.activity_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE user_tasks SET progress = ?
             WHERE user_id = 6 AND type_id = ? AND task_id = ?",
        )
        .bind(successor.max_progress)
        .bind(database::db::game::tasks::TaskType::ActBp.id())
        .bind(successor.id)
        .execute(&pool)
        .await
        .unwrap();

        let (task_infos, _) = super::act233_red_dot_state(&pool, 6, Some(pass.activity_id))
            .await
            .unwrap();
        assert_eq!(task_infos, vec![super::red_dot_info(0, 0)]);

        sqlx::query(
            "UPDATE user_tasks SET finish_count = 1
             WHERE user_id = 6 AND type_id = ? AND task_id = ?",
        )
        .bind(database::db::game::tasks::TaskType::ActBp.id())
        .bind(predecessor_id)
        .execute(&pool)
        .await
        .unwrap();
        let (task_infos, _) = super::act233_red_dot_state(&pool, 6, Some(pass.activity_id))
            .await
            .unwrap();
        assert_eq!(task_infos, vec![super::red_dot_info(successor_info_id, 1)]);
    }

    #[tokio::test]
    async fn room_faith_red_dot_is_derived_from_accumulated_minutes() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        database::run_migrations(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, created_at, updated_at) VALUES (2, 'faith-dot', 0, 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO user_room_heroes (user_id, hero_id, current_minute)
             VALUES (2, 3023, 1199)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let define_id = RedDotId::RoomCharacterFaithGetFull.id();
        let manager = super::RedDotManager::new(2);
        let before = manager.infos(&pool, vec![define_id]).await.unwrap();
        assert_eq!(before.red_dot_infos[0].infos[0].id, 0);
        assert_eq!(before.red_dot_infos[0].infos[0].value, 0);

        sqlx::query(
            "UPDATE user_room_heroes SET current_minute = 1200
             WHERE user_id = 2 AND hero_id = 3023",
        )
        .execute(&pool)
        .await
        .unwrap();
        let full = manager.infos(&pool, vec![define_id]).await.unwrap();
        assert_eq!(full.red_dot_infos[0].infos[0].id, 3023);
        assert_eq!(full.red_dot_infos[0].infos[0].value, 1);
        assert_eq!(full.red_dot_infos[0].replace_all, Some(true));

        sqlx::query(
            "INSERT INTO user_room_production_lines
             (user_id, line_id, formula_id, finish_count, level)
             VALUES (2, 1, 2002001, 999, 1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        let production = manager
            .infos(&pool, vec![RedDotId::RoomProductionFull.id()])
            .await
            .unwrap();
        assert_eq!(production.red_dot_infos[0].infos[0].id, 1);
        assert_eq!(production.red_dot_infos[0].infos[0].value, 1);
    }

    #[tokio::test]
    async fn trade_order_red_dot_is_derived_from_saved_orders_and_inventory() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        database::run_migrations(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, created_at, updated_at) VALUES (3, 'trade-dot', 0, 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        database::db::starter_data::load_all_starter_data(&pool, 3)
            .await
            .unwrap();

        let define_id = RedDotId::TradeOrderFulfillable.id();
        let manager = super::RedDotManager::new(3);
        let before = manager.infos(&pool, vec![define_id]).await.unwrap();
        assert_eq!(before.red_dot_infos[0].infos[0].value, 0);

        let goods = sqlx::query_as::<_, (i32, i32)>(
            "SELECT production_id, quantity FROM user_room_purchase_order_goods
             WHERE user_id = 3 AND order_id = 1",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        for (production_id, quantity) in goods {
            let item_id = config::configs::get()
                .manufacture_item
                .get(production_id)
                .unwrap()
                .item_id;
            database::db::game::items::add_item_quantity(&pool, 3, item_id as u32, quantity)
                .await
                .unwrap();
        }

        let ready = manager.infos(&pool, vec![define_id]).await.unwrap();
        assert_eq!(ready.red_dot_infos[0].infos[0].value, 1);
        assert_eq!(manager.trade_order_value(&pool).await.unwrap(), 1);
    }
}
