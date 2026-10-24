use super::*;

impl ActivityManager {
    pub async fn act125_infos(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<GetAct125InfosReply, AppError> {
        let reply = act125_infos(db, self.player_id, activity_id).await?;
        let activity_id = reply.activity_id.ok_or(AppError::InvalidRequest)?;
        self.refresh_states(db, activity_id, ActivityStateKind::Act125Episode)
            .await?;
        Ok(reply)
    }

    pub async fn finish_act125_episode(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        episode_id: Option<i32>,
        target_frequency: Option<i32>,
    ) -> Result<act125::Act125Claim, AppError> {
        let reply = finish_act125_episode(
            db,
            self.player_id,
            activity_id,
            episode_id,
            target_frequency,
        )
        .await?;
        let activity_id = reply.reply.activity_id.ok_or(AppError::InvalidRequest)?;
        self.refresh_states(db, activity_id, ActivityStateKind::Act125Episode)
            .await?;
        Ok(reply)
    }

    pub async fn act136_info(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<Get136InfoReply, AppError> {
        act136_info(db, self.player_id, activity_id).await
    }

    pub async fn act136_select(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        hero_id: Option<i32>,
    ) -> Result<act136::Act136SelectClaim, AppError> {
        act136_select(db, self.player_id, activity_id, hero_id).await
    }

    pub async fn act146_infos(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<GetAct146InfosReply, AppError> {
        let reply = act146_infos(db, self.player_id, activity_id).await?;
        if let Some(activity_id) = reply.activity_id {
            self.refresh_states(db, activity_id, ActivityStateKind::Act146Episode)
                .await?;
        }
        Ok(reply)
    }

    pub async fn finish_act146_episode(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        episode_id: Option<i32>,
    ) -> Result<FinishAct146EpisodeReply, AppError> {
        let reply = finish_act146_episode(db, self.player_id, activity_id, episode_id).await?;
        if let Some(activity_id) = reply.activity_id {
            self.refresh_states(db, activity_id, ActivityStateKind::Act146Episode)
                .await?;
        }
        Ok(reply)
    }

    pub async fn act146_episode_bonus(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        episode_id: Option<i32>,
    ) -> Result<act146::Act146Claim, AppError> {
        let claim = act146_episode_bonus(db, self.player_id, activity_id, episode_id).await?;
        if let Some(activity_id) = claim.reply.activity_id {
            self.refresh_states(db, activity_id, ActivityStateKind::Act146Episode)
                .await?;
        }
        Ok(claim)
    }

    pub async fn act152_info(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<Get152InfoReply, AppError> {
        act152_info(db, self.player_id, activity_id).await
    }

    pub async fn accept_act152_present(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        present_id: Option<i32>,
    ) -> Result<act152::Act152PresentClaim, AppError> {
        accept_act152_present(db, self.player_id, activity_id, present_id).await
    }

    pub async fn act154_infos(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<Get154InfosReply, AppError> {
        act154_infos(db, self.player_id, activity_id).await
    }

    pub async fn answer154_puzzle(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        puzzle_id: Option<u32>,
        option_id: Option<u32>,
    ) -> Result<act154::Act154Claim, AppError> {
        answer154_puzzle(db, self.player_id, activity_id, puzzle_id, option_id).await
    }

    pub fn act158_infos(&self, activity_id: Option<i32>) -> Get158InfosReply {
        act158_infos(activity_id)
    }

    pub async fn act160_get_info(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<Act160GetInfoReply, AppError> {
        let reply = act160_get_info(db, self.player_id, activity_id).await?;
        let activity_id = reply.activity_id.unwrap_or_else(latest_act160_activity_id);
        self.refresh_states(db, activity_id, ActivityStateKind::Act160Mission)
            .await?;
        Ok(reply)
    }

    pub async fn act172_info(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<GetAct172InfoReply, AppError> {
        let reply = act172_info(db, self.player_id, activity_id).await?;
        if let Some(activity_id) = reply.activity_id {
            self.refresh_states(db, activity_id, ActivityStateKind::Act172UseItemTask)
                .await?;
        }
        Ok(reply)
    }

    pub async fn finish_act160_mission(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        id: Option<i32>,
    ) -> Result<act160::Act160Claim, AppError> {
        let claim = finish_act160_mission(db, self.player_id, activity_id, id).await?;
        let activity_id = claim
            .reply
            .activity_id
            .unwrap_or_else(latest_act160_activity_id);
        self.refresh_states(db, activity_id, ActivityStateKind::Act160Mission)
            .await?;
        Ok(claim)
    }

    pub async fn act165_get_info(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<Act165GetInfoReply, AppError> {
        let reply = act165_get_info(db, self.player_id, activity_id).await?;
        let activity_id = reply.activity_id.unwrap_or_else(latest_act165_activity_id);
        self.refresh_states(db, activity_id, ActivityStateKind::Act165Story)
            .await?;
        Ok(reply)
    }

    pub async fn act165_modify_keyword(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        story_id: Option<i32>,
        keyword_ids: Vec<i32>,
    ) -> Result<Act165ModifyKeywordReply, AppError> {
        let reply =
            act165_modify_keyword(db, self.player_id, activity_id, story_id, keyword_ids).await?;
        let activity_id = reply.activity_id.unwrap_or_else(latest_act165_activity_id);
        self.refresh_states(db, activity_id, ActivityStateKind::Act165Story)
            .await?;
        Ok(reply)
    }

    pub async fn act165_generate_ending(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        story_id: Option<i32>,
    ) -> Result<Act165GenerateEndingReply, AppError> {
        let reply = act165_generate_ending(db, self.player_id, activity_id, story_id).await?;
        let activity_id = reply.activity_id.unwrap_or_else(latest_act165_activity_id);
        self.refresh_states(db, activity_id, ActivityStateKind::Act165Story)
            .await?;
        Ok(reply)
    }

    pub async fn act165_restart(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        story_id: Option<i32>,
        step_id: Option<i32>,
    ) -> Result<Act165RestartReply, AppError> {
        let reply = act165_restart(db, self.player_id, activity_id, story_id, step_id).await?;
        let activity_id = reply.activity_id.unwrap_or_else(latest_act165_activity_id);
        self.refresh_states(db, activity_id, ActivityStateKind::Act165Story)
            .await?;
        Ok(reply)
    }

    pub async fn act165_gain_milestone_reward(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        story_id: Option<i32>,
    ) -> Result<act165::Act165RewardClaim, AppError> {
        let claim = act165_gain_milestone_reward(db, self.player_id, activity_id, story_id).await?;
        let activity_id = claim
            .reply
            .activity_id
            .unwrap_or_else(latest_act165_activity_id);
        self.refresh_states(db, activity_id, ActivityStateKind::Act165Story)
            .await?;
        Ok(claim)
    }

    pub fn act166_infos(&self, activity_id: Option<i32>) -> Get166InfosReply {
        act166_infos(activity_id)
    }

    pub async fn act208_info(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<GetAct208InfoReply, AppError> {
        let reply = act208_info(db, self.player_id, activity_id).await?;
        if let Some(activity_id) = reply.activity_id {
            self.refresh_states(db, activity_id, ActivityStateKind::Act208Bonus)
                .await?;
        }
        Ok(reply)
    }

    pub async fn receive_act208_bonus(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        id: Option<i32>,
    ) -> Result<act208::Act208Claim, AppError> {
        let claim = receive_act208_bonus(db, self.player_id, activity_id, id).await?;
        if let Some(activity_id) = claim.reply.activity_id {
            self.refresh_states(db, activity_id, ActivityStateKind::Act208Bonus)
                .await?;
        }
        Ok(claim)
    }

    pub async fn act209_info(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<GetAct209InfoReply, AppError> {
        let reply = act209_info(db, self.player_id, activity_id).await?;
        if let Some(activity_id) = reply.activity_id {
            self.refresh_states(db, activity_id, ActivityStateKind::Act209Layer)
                .await?;
        }
        Ok(reply)
    }

    pub async fn act212_info(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<GetAct212InfoReply, AppError> {
        let reply = act212_info(db, self.player_id, activity_id).await?;
        if let Some(info) = &reply.act212_info
            && let Some(activity_id) = info.activity_id
        {
            self.refresh_states(db, activity_id, ActivityStateKind::Act212Bonus)
                .await?;
        }
        Ok(reply)
    }

    pub async fn receive_act212_bonus(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        id: Option<i32>,
    ) -> Result<act212::Act212Claim, AppError> {
        let claim = receive_act212_bonus(db, self.player_id, activity_id, id).await?;
        if let Some(activity_id) = claim.reply.activity_id {
            self.refresh_states(db, activity_id, ActivityStateKind::Act212Bonus)
                .await?;
        }
        Ok(claim)
    }

    pub async fn act216_info(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<GetAct216InfoReply, AppError> {
        act216_info(db, self.player_id, activity_id).await
    }

    pub async fn finish_act216_task(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        task_id: Option<i32>,
    ) -> Result<act216::Act216TaskClaim, AppError> {
        finish_act216_task(db, self.player_id, activity_id, task_id).await
    }

    pub async fn get_act216_once_bonus(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<act216::Act216OnceBonusClaim, AppError> {
        get_act216_once_bonus(db, self.player_id, activity_id).await
    }

    pub async fn act225_info(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<GetAct225InfoReply, AppError> {
        act225_info(db, self.player_id, activity_id).await
    }

    pub async fn act218_info(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<Get218InfoReply, AppError> {
        act218_info(db, self.player_id, activity_id).await
    }

    pub async fn finish_act218_game(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        result: Option<i32>,
        game_record: Option<String>,
    ) -> Result<Act218FinishGameReply, AppError> {
        finish_act218_game(db, self.player_id, activity_id, result, game_record).await
    }

    pub async fn accept_act218_reward(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<act218::Act218RewardClaim, AppError> {
        accept_act218_reward(db, self.player_id, activity_id).await
    }
}
