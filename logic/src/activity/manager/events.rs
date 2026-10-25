use super::*;

impl ActivityManager {
    pub async fn sync_act101_login_progress(
        &self,
        db: &SqlitePool,
        now_ms: i64,
    ) -> Result<(), AppError> {
        let Ok(schedule_time) = u64::try_from(now_ms) else {
            return Ok(());
        };
        let activity_ids = active_act101_activity_ids_at(schedule_time);
        activity101::record_login_progress(
            db,
            self.player_id,
            &activity_ids,
            common::time::ServerTime::server_day(now_ms),
        )
        .await?;
        Ok(())
    }

    pub async fn get101_infos(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<Get101InfosReply, AppError> {
        let reply = get101_infos(db, self.player_id, activity_id).await?;
        let activity_id = reply.activity_id.unwrap_or_else(latest_act101_activity_id);
        self.refresh_states(db, activity_id, ActivityStateKind::Act101Day)
            .await?;
        self.refresh_states(db, activity_id, ActivityStateKind::Act101Once)
            .await?;
        self.refresh_states(db, activity_id, ActivityStateKind::Act101SpBonus)
            .await?;
        Ok(reply)
    }

    pub async fn get101_bonus(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        day_id: Option<u32>,
    ) -> Result<act101::Activity101Claim, AppError> {
        let claim = get101_bonus(db, self.player_id, activity_id, day_id).await?;
        let activity_id = claim
            .reply
            .activity_id
            .unwrap_or_else(latest_act101_activity_id);
        self.refresh_states(db, activity_id, ActivityStateKind::Act101Day)
            .await?;
        self.refresh_states(db, activity_id, ActivityStateKind::Act101Once)
            .await?;
        Ok(claim)
    }

    pub async fn get101_bonus_list(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        day_ids: Vec<u32>,
    ) -> Result<act101::Activity101ListClaim, AppError> {
        let claim = get101_bonus_list(db, self.player_id, activity_id, day_ids).await?;
        let activity_id = claim
            .reply
            .activity_id
            .unwrap_or_else(latest_act101_activity_id);
        self.refresh_states(db, activity_id, ActivityStateKind::Act101Day)
            .await?;
        self.refresh_states(db, activity_id, ActivityStateKind::Act101Once)
            .await?;
        Ok(claim)
    }

    pub async fn get101_sp_bonus(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        id: Option<i32>,
    ) -> Result<act101::Activity101SpClaim, AppError> {
        let claim = get101_sp_bonus(db, self.player_id, activity_id, id).await?;
        let activity_id = claim
            .reply
            .activity_id
            .unwrap_or_else(latest_act101_activity_id);
        self.refresh_states(db, activity_id, ActivityStateKind::Act101SpBonus)
            .await?;
        Ok(claim)
    }

    pub async fn act104_infos(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<Get104InfosReply, AppError> {
        let reply = act104_infos(db, self.player_id, activity_id).await?;
        if let Some(activity_id) = reply.activity_id {
            self.refresh_states(db, activity_id, ActivityStateKind::Act104Episode)
                .await?;
            self.refresh_states(db, activity_id, ActivityStateKind::Act104Special)
                .await?;
            self.refresh_states(db, activity_id, ActivityStateKind::Act104AfterStory)
                .await?;
            self.refresh_states(db, activity_id, ActivityStateKind::Act104Story)
                .await?;
            self.refresh_states(db, activity_id, ActivityStateKind::Act104PopSummary)
                .await?;
        }
        Ok(reply)
    }

    pub async fn mark_activity104_story(
        &mut self,
        db: &SqlitePool,
        activity_id: i32,
    ) -> Result<MarkActivity104StoryReply, AppError> {
        let reply = mark_activity104_story(db, self.player_id, activity_id).await?;
        self.refresh_states(db, activity_id, ActivityStateKind::Act104Story)
            .await?;
        Ok(reply)
    }

    pub async fn mark_pop_summary(
        &mut self,
        db: &SqlitePool,
        activity_id: i32,
    ) -> Result<MarkPopSummaryReply, AppError> {
        let reply = mark_pop_summary(db, self.player_id, activity_id).await?;
        self.refresh_states(db, activity_id, ActivityStateKind::Act104PopSummary)
            .await?;
        Ok(reply)
    }

    pub async fn mark_episode_after_story(
        &mut self,
        db: &SqlitePool,
        activity_id: i32,
        layer: i32,
    ) -> Result<MarkEpisodeAfterStoryReply, AppError> {
        let reply = mark_episode_after_story(db, self.player_id, activity_id, layer).await?;
        self.refresh_states(db, activity_id, ActivityStateKind::Act104AfterStory)
            .await?;
        Ok(reply)
    }

    pub async fn get_act186_sp_bonus_info(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        act186_activity_id: Option<i32>,
    ) -> Result<GetAct186SpBonusInfoReply, AppError> {
        let reply =
            get_act186_sp_bonus_info(db, self.player_id, activity_id, act186_activity_id).await?;
        if let Some(activity_id) = reply.activity_id {
            self.refresh_states(db, activity_id, ActivityStateKind::Act186SpBonus)
                .await?;
        }
        Ok(reply)
    }

    pub async fn act186_info(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<GetAct186InfoReply, AppError> {
        let reply = act186_info(db, self.player_id, activity_id).await?;
        if let Some(activity_id) = reply.activity_id {
            self.refresh_states(db, activity_id, ActivityStateKind::Act186Task)
                .await?;
        }
        Ok(reply)
    }

    pub async fn accept_act186_sp_bonus(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        act186_activity_id: Option<i32>,
    ) -> Result<AcceptAct186SpBonusReply, AppError> {
        let reply =
            accept_act186_sp_bonus(db, self.player_id, activity_id, act186_activity_id).await?;
        if let Some(activity_id) = reply.activity_id {
            self.refresh_states(db, activity_id, ActivityStateKind::Act186SpBonus)
                .await?;
        }
        Ok(reply)
    }

    pub async fn act189_info(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<GetAct189InfoReply, AppError> {
        let reply = act189_info(db, self.player_id, activity_id).await?;
        if let Some(activity_id) = reply.activity_id {
            self.refresh_states(db, activity_id, ActivityStateKind::Act189OnceBonus)
                .await?;
        }
        Ok(reply)
    }

    pub async fn get_act189_once_bonus(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<act189::Act189OnceBonusClaim, AppError> {
        let claim = get_act189_once_bonus(db, self.player_id, activity_id).await?;
        if let Some(activity_id) = claim.reply.activity_id {
            self.refresh_states(db, activity_id, ActivityStateKind::Act189OnceBonus)
                .await?;
        }
        Ok(claim)
    }

    pub async fn act199_info(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<Get199InfoReply, AppError> {
        act199_info(db, self.player_id, activity_id).await
    }

    pub async fn act199_gain(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        hero_id: Option<i32>,
    ) -> Result<act199::Act199GainClaim, AppError> {
        act199_gain(db, self.player_id, activity_id, hero_id).await
    }

    pub async fn act196_info(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<Get196InfoReply, AppError> {
        act196_info(db, self.player_id, activity_id).await
    }

    pub async fn act196_gain(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        id: Option<i32>,
    ) -> Result<act196::Act196Claim, AppError> {
        act196_gain(db, self.player_id, activity_id, id).await
    }

    pub async fn act197_info(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<Get197InfoReply, AppError> {
        act197_info(db, self.player_id, activity_id).await
    }

    pub async fn act197_rummage(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        pool_id: Option<i32>,
    ) -> Result<act197::Act197Claim, AppError> {
        act197_rummage(db, self.player_id, activity_id, pool_id).await
    }

    pub async fn act197_explore(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        find_type: Option<i32>,
    ) -> Result<act197::Act197Explore, AppError> {
        act197_explore(db, self.player_id, activity_id, find_type).await
    }

    pub async fn act198_gain(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<act198::Act198Claim, AppError> {
        act198_gain(db, self.player_id, activity_id).await
    }

    pub async fn act205_get_info(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<Act205GetInfoReply, AppError> {
        act205_get_info(db, self.player_id, activity_id).await
    }

    pub async fn act205_get_game_info(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<Act205GetGameInfoReply, AppError> {
        act205_get_game_info(db, self.player_id, activity_id).await
    }

    pub async fn act205_finish_game(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        game_type: Option<i32>,
        game_info: Option<String>,
        reward_id: Option<i32>,
    ) -> Result<act205::Act205Claim, AppError> {
        act205_finish_game(
            db,
            self.player_id,
            activity_id,
            game_type,
            game_info,
            reward_id,
        )
        .await
    }

    pub async fn act206_get_info(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<Act206GetInfoReply, AppError> {
        act206_get_info(db, self.player_id, activity_id).await
    }

    pub async fn act206_choose_direction(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        direction_id: Option<i32>,
    ) -> Result<Act206ChooseDirectionReply, AppError> {
        act206_choose_direction(db, self.player_id, activity_id, direction_id).await
    }

    pub async fn act206_get_bonus(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<act206::Act206Claim, AppError> {
        act206_get_bonus(db, self.player_id, activity_id).await
    }

    pub async fn act221_info(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<Get221InfoReply, AppError> {
        act221_info(db, self.player_id, activity_id).await
    }

    pub async fn act221_summon(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<Act221SummonReply, AppError> {
        act221_summon(db, self.player_id, activity_id).await
    }

    pub async fn act221_select(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        select_index: Option<i32>,
    ) -> Result<act221::Act221Claim, AppError> {
        act221_select(db, self.player_id, activity_id, select_index).await
    }
}
