use super::*;

pub(super) async fn get_bp_info(
    db: &SqlitePool,
    player_id: i64,
    include_tasks: bool,
) -> Result<GetBpInfoReply, AppError> {
    let Some(bp_id) = task_db::current_battle_pass_id() else {
        return Ok(GetBpInfoReply::default());
    };

    get_bp_info_for_id(db, player_id, bp_id, include_tasks).await
}

async fn get_bp_info_for_id(
    db: &SqlitePool,
    player_id: i64,
    bp_id: i32,
    include_tasks: bool,
) -> Result<GetBpInfoReply, AppError> {
    let state = battle_pass::get_or_create_state(db, player_id, bp_id).await?;

    let tasks = if include_tasks {
        task_db::ensure_battle_pass_tasks(db, player_id, bp_id).await?;
        task_db::ensure_bp_oper_act_tasks(db, player_id, bp_id).await?;
        let mut tasks = task_db::list_battle_pass(db, player_id, bp_id).await?;
        tasks.extend(
            task_db::list_by_types(db, player_id, vec![task_db::TaskType::BpOperAct.id()])
                .await?
                .into_iter()
                .filter(|task| {
                    config::configs::get()
                        .activity214_task
                        .get(task.task_id)
                        .is_some_and(|config| config.bp_id == bp_id)
                }),
        );
        tasks
    } else {
        Vec::new()
    };
    let (start_time, end_time) = bp_time_range(bp_id);

    Ok(GetBpInfoReply {
        id: Some(bp_id),
        score: Some(state.score),
        pay_status: Some(state.pay_status),
        start_time,
        end_time,
        task_info: tasks.into_iter().map(Into::into).collect(),
        score_bonus_info: score_bonus_info(bp_id, Some(&state)),
        weekly_score: Some(state.weekly_score),
        first_show: Some(state.first_show),
        has_get_self_select_bonus: state.has_get_self_select_bonus,
        sp_first_show: Some(state.sp_first_show),
    })
}

#[cfg(test)]
mod tests;

pub struct BpBonusClaim {
    pub reply: GetBpBonusReply,
    pub rewards: reward::AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
}

pub struct BpSelfSelectClaim {
    pub reply: GetSelfSelectBonusReply,
    pub rewards: reward::AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
}

pub struct BpLevelPurchase {
    pub reply: BpBuyLevelReply,
    pub currency_change: (i32, i32),
    pub material_change: (u32, u32, i32),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BpBonusRedDots {
    pub normal: i32,
    pub sp: i32,
}

pub(super) async fn bonus_red_dots(
    db: &SqlitePool,
    player_id: i64,
) -> Result<BpBonusRedDots, AppError> {
    let Some(bp) = task_db::current_battle_pass() else {
        return Ok(BpBonusRedDots::default());
    };

    bonus_red_dots_for(db, player_id, bp).await
}

pub(super) async fn bonus_red_dots_for(
    db: &SqlitePool,
    player_id: i64,
    bp: &config::bp::Bp,
) -> Result<BpBonusRedDots, AppError> {
    let state = battle_pass::get_or_create_state(db, player_id, bp.bp_id).await?;

    Ok(bonus_red_dots_for_state(bp.bp_id, bp.exp_level_up, &state))
}

pub(super) fn bonus_red_dots_for_state(
    bp_id: i32,
    exp_level_up: i32,
    state: &battle_pass::BattlePassState,
) -> BpBonusRedDots {
    let level = state.score / exp_level_up.max(1);
    let mut result = BpBonusRedDots::default();

    for bonus in config::configs::get()
        .battle_pass_bonuses(bp_id)
        .filter(|bonus| bonus.level <= level)
    {
        let normal_free =
            !bonus.free_bonus.is_empty() && !state.has_get_free_bonus.contains(&bonus.level);
        let normal_paid = state.pay_status > 0
            && !bonus.pay_bonus.is_empty()
            && !state.has_get_pay_bonus.contains(&bonus.level);
        let sp_free =
            !bonus.sp_free_bonus.is_empty() && !state.has_get_sp_free_bonus.contains(&bonus.level);
        let sp_paid =
            !bonus.sp_pay_bonus.is_empty() && !state.has_get_sp_pay_bonus.contains(&bonus.level);
        let sp_select = !bonus.self_select_pay_bonus.is_empty()
            && state
                .has_get_self_select_bonus
                .iter()
                .all(|claimed| claimed.level != Some(bonus.level));

        result.normal |= i32::from(normal_free || normal_paid);
        result.sp |= i32::from(sp_free || sp_paid || sp_select);
    }

    result
}
