use super::*;

impl ActivityManager {
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

    pub async fn act236_info(
        &self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<GetAct236InfoReply, AppError> {
        act236_info(db, self.player_id, activity_id).await
    }

    pub async fn act236_get_auto_gain_reward(
        &self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        reward_ids: Vec<i32>,
    ) -> Result<act236::Act236RewardClaim, AppError> {
        act236_get_auto_gain_reward(db, self.player_id, activity_id, reward_ids).await
    }

    pub async fn act239_info(
        &self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<GetAct239InfoReply, AppError> {
        act239_info(db, self.player_id, activity_id).await
    }

    pub async fn act239_bonus(
        &self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        id: Option<i32>,
    ) -> Result<act239::Act239Claim, AppError> {
        act239_bonus(db, self.player_id, activity_id, id).await
    }

    pub async fn get_gallery_infos(
        &self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<sonettobuf::GetGalleryInfosReply, AppError> {
        get_gallery_infos(db, self.player_id, activity_id).await
    }

    pub async fn get_act128_exp(
        &self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        boss_type: Option<i32>,
    ) -> Result<act128::Act128ExpClaim, AppError> {
        get_act128_exp(db, self.player_id, activity_id, boss_type).await
    }

    pub async fn act128_info(
        &self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<Get128InfosReply, AppError> {
        act128_info(db, self.player_id, activity_id).await
    }

    pub async fn act116_info(
        &self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        tables: &config::GameDB,
    ) -> Result<Get116InfosReply, AppError> {
        act116_info(
            db,
            self.player_id,
            activity_id,
            tables,
            common::time::ServerTime::now_ms(),
        )
        .await
    }

    pub async fn upgrade_act116_element(
        &self,
        db: &SqlitePool,
        activity_id: i32,
        element_id: i32,
        tables: &config::GameDB,
    ) -> Result<act116::Activity116Cost<UpgradeElementReply>, AppError> {
        upgrade_element(db, self.player_id, activity_id, element_id, tables).await
    }

    pub async fn build_act116_trap(
        &self,
        db: &SqlitePool,
        activity_id: i32,
        trap_id: i32,
        tables: &config::GameDB,
    ) -> Result<act116::Activity116Cost<BuildTrapReply>, AppError> {
        build_trap(db, self.player_id, activity_id, trap_id, tables).await
    }

    pub async fn put_act116_trap(
        &self,
        db: &SqlitePool,
        activity_id: i32,
        trap_id: i32,
        tables: &config::GameDB,
    ) -> Result<PutTrapReply, AppError> {
        put_trap(db, self.player_id, activity_id, trap_id, tables).await
    }

    pub async fn get_act128_milestone_bonus(
        &self,
        db: &SqlitePool,
        activity_id: Option<i32>,
    ) -> Result<act128::Act128MilestoneClaim, AppError> {
        get_act128_milestone_bonus(db, self.player_id, activity_id).await
    }

    pub async fn get_act128_total_rewards(
        &self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        boss_id: Option<i32>,
    ) -> Result<act128::Act128TotalRewardsClaim, AppError> {
        get_act128_total_rewards(db, self.player_id, activity_id, boss_id).await
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
}
