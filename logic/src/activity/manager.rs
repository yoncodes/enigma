use super::*;

#[derive(Clone, Debug)]
pub struct ActivityManager {
    pub(super) player_id: i64,
    states: HashMap<(i32, i32), activity_state::ActivityStates>,
}

impl ActivityManager {
    pub fn new(player_id: i64) -> Self {
        Self {
            player_id,
            states: HashMap::new(),
        }
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

    pub async fn infos(&mut self, db: &SqlitePool) -> Result<GetActivityInfosReply, AppError> {
        let mut infos = catalog_infos();
        apply_bp_activity(&mut infos);
        apply_act125_activity(&mut infos);
        apply_activity_state(db, self.player_id, &mut infos).await?;

        Ok(GetActivityInfosReply {
            activity_infos: infos,
        })
    }

    pub async fn infos_with_param(
        &mut self,
        db: &SqlitePool,
        activity_ids: &[i32],
    ) -> Result<GetActivityInfosWithParamReply, AppError> {
        let requested = activity_ids.iter().copied().collect::<HashSet<_>>();
        let mut infos = catalog_infos();
        apply_bp_activity(&mut infos);
        apply_act125_activity(&mut infos);
        if !requested.is_empty() {
            infos.retain(|info| info.id.is_some_and(|id| requested.contains(&(id as i32))));
        }
        apply_activity_state(db, self.player_id, &mut infos).await?;

        Ok(GetActivityInfosWithParamReply {
            activity_infos: infos,
        })
    }

    pub async fn mark_new_stages_read(
        &mut self,
        db: &SqlitePool,
        mut ids: Vec<u32>,
    ) -> Result<ActivityNewStageReadReply, AppError> {
        ids.sort_unstable();
        ids.dedup();

        for id in &ids {
            activity_state::set_activity_flag(
                db,
                self.player_id,
                *id as i32,
                ActivityStateKind::ActivityNewStage,
                false,
            )
            .await?;
        }

        Ok(ActivityNewStageReadReply { id: ids })
    }

    pub async fn unlock_permanent(
        &mut self,
        db: &SqlitePool,
        id: Option<u32>,
    ) -> Result<UnlockPermanentReply, AppError> {
        if let Some(id) = id {
            activity_state::set_activity_flag(
                db,
                self.player_id,
                id as i32,
                ActivityStateKind::ActivityPermanentUnlock,
                true,
            )
            .await?;
        }

        Ok(UnlockPermanentReply { id })
    }

    pub fn act123_infos(&self, activity_id: Option<i32>) -> Get123InfosReply {
        act123_infos(activity_id)
    }

    pub fn act153_infos(&self, activity_id: Option<i32>) -> Get153InfosReply {
        act153_infos(activity_id)
    }

    pub async fn act217_infos(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<Get217InfosReply, AppError> {
        act217_infos(db, self.player_id, activity_id).await
    }

    pub fn act228_info(&self, activity_id: Option<i32>) -> GetAct228InfoReply {
        act228_info(activity_id)
    }

    pub fn act228_flip_grid(&self, activity_id: Option<i32>) -> Act228FlipGridGridReply {
        act228_flip_grid(activity_id)
    }

    pub fn act228_get_final_bonus(&self, activity_id: Option<i32>) -> Act228GetFinalBonusReply {
        act228_get_final_bonus(activity_id)
    }

    pub async fn act229_info(
        &mut self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<GetAct229InfoReply, AppError> {
        act229_info(db, self.player_id, activity_id).await
    }

    pub fn act229_battle_episode(&self, activity_id: i32, stage_id: i32) -> Result<i32, AppError> {
        act229_battle_episode(activity_id, stage_id)
    }

    pub async fn ensure_act229_heroes_available(
        &self,
        db: &SqlitePool,
        activity_id: i32,
        stage_id: i32,
        heroes: &[Act229HeroNo],
    ) -> Result<(), AppError> {
        act229_heroes_available(db, self.player_id, activity_id, stage_id, heroes).await
    }

    pub async fn finish_act229_battle(
        &self,
        db: &SqlitePool,
        activity_id: i32,
        stage_id: i32,
        round: i32,
        star: i32,
        heroes: &[Act229HeroNo],
    ) -> Result<Act229BattleFinishPush, AppError> {
        finish_act229_battle(
            db,
            self.player_id,
            activity_id,
            stage_id,
            round,
            star,
            heroes,
        )
        .await
    }

    pub async fn reset_act229_stage(
        &self,
        db: &SqlitePool,
        activity_id: i32,
        stage_id: i32,
    ) -> Result<Act229ResetStageReply, AppError> {
        reset_act229_stage(db, self.player_id, activity_id, stage_id).await
    }

    async fn refresh_states(
        &mut self,
        db: &SqlitePool,
        activity_id: i32,
        kind: ActivityStateKind,
    ) -> Result<(), AppError> {
        let states = activity_state::get(db, self.player_id, activity_id, kind).await?;
        self.states.insert((activity_id, kind.id()), states);
        Ok(())
    }
}

async fn apply_activity_state(
    db: &SqlitePool,
    player_id: i64,
    infos: &mut [ActivityInfo],
) -> Result<(), AppError> {
    let new_stage =
        activity_state::get_activity_flags(db, player_id, ActivityStateKind::ActivityNewStage)
            .await?;
    let permanent_unlock = activity_state::get_activity_flags(
        db,
        player_id,
        ActivityStateKind::ActivityPermanentUnlock,
    )
    .await?;

    for info in infos {
        let Some(activity_id) = info.id.map(|id| id as i32) else {
            continue;
        };

        info.is_new_stage = Some(new_stage.contains(&activity_id));
        info.is_unlock =
            Some(is_unlocked_by_default(activity_id) || permanent_unlock.contains(&activity_id));
    }

    Ok(())
}
