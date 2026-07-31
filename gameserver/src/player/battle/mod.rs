use crate::error::AppError;
use crate::logic::reward::{self, ConsumedRewards, RewardSet};
use common::time::ServerTime;
use database::db::game::battle;
use flate2::{Compression, read::GzEncoder};
use prost::Message;
use serde::{Deserialize, Serialize};
use sonettobuf::{
    Act229HeroNo, AutoRoundReply, AutoRoundRequest, BeginRoundReply, BeginRoundRequest, CardInfo,
    CardInfoPush, FightEntityInfo, FightReason, FightRoundOperRecord, ReconnectFightReply,
    RedealCardInfoPush, ResetRoundReply, StartDungeonReply, StartDungeonRequest,
    UseClothSkillOperRecord, UseClothSkillReply, UseClothSkillRequest, fight_reason,
};
use sqlx::SqlitePool;
use std::io::Read;

#[derive(Clone, Debug, Default)]
pub struct BattleState {
    active: Option<ActiveBattle>,
    pending_record: Option<PendingDungeonRecord>,
}

impl BattleState {
    pub fn has_active(&self) -> bool {
        self.active.is_some()
    }

    pub fn saved_start_matches(&self, request: &StartDungeonRequest) -> Option<bool> {
        self.active
            .as_ref()
            .map(|active| crate::dungeon::matches_saved_dungeon_start(active, request))
    }

    pub fn active_snapshot(&self) -> Option<ActiveBattle> {
        self.active.clone()
    }

    pub fn restore_active(&mut self, active: ActiveBattle) {
        self.active = Some(active);
    }

    pub fn start_active(&mut self, active: ActiveBattle) {
        self.pending_record = None;
        self.active = Some(active);
    }

    pub fn clear_active(&mut self) {
        self.active = None;
    }

    pub fn complete_active(&mut self, pending_record: Option<PendingDungeonRecord>) {
        self.pending_record = pending_record;
        self.active = None;
    }

    pub fn clear_pending_record(&mut self) {
        self.pending_record = None;
    }

    pub fn take_pending_record(&mut self) -> Option<PendingDungeonRecord> {
        self.pending_record.take()
    }

    pub async fn ensure_can_start(
        &self,
        pool: &SqlitePool,
        player_id: i64,
    ) -> Result<(), AppError> {
        if self.active.is_some() || battle::load_active_fight(pool, player_id).await?.is_some() {
            return Err(AppError::InvalidRequest);
        }
        Ok(())
    }

    pub fn entity_info(&self, uid: i64) -> Result<FightEntityInfo, AppError> {
        self.active
            .as_ref()
            .and_then(|active| active.runtime.entity_info(uid))
            .cloned()
            .ok_or(AppError::InvalidRequest)
    }

    pub fn card_deck(&self, team_type: i32) -> Result<Vec<CardInfo>, AppError> {
        self.active
            .as_ref()
            .and_then(|active| active.runtime.card_deck(team_type))
            .map(<[_]>::to_vec)
            .ok_or(AppError::InvalidRequest)
    }

    pub fn reset_round(&self) -> Result<ResetRoundReply, AppError> {
        self.active
            .as_ref()
            .map(|_| ResetRoundReply::default())
            .ok_or(AppError::InvalidRequest)
    }

    pub fn start_payload(&self) -> Result<(StartDungeonReply, CardInfoPush), AppError> {
        self.active
            .as_ref()
            .map(|active| (active.start_reply(), active.card_info_push()))
            .ok_or(AppError::InvalidRequest)
    }

    pub fn reconnect_reply(&self) -> ReconnectFightReply {
        self.active
            .as_ref()
            .map(ActiveBattle::reconnect_reply)
            .unwrap_or_default()
    }

    pub fn use_cloth_skill(
        &mut self,
        request: UseClothSkillRequest,
    ) -> Result<(UseClothSkillReply, Option<RedealCardInfoPush>), AppError> {
        self.active
            .as_mut()
            .ok_or(AppError::InvalidRequest)?
            .use_cloth_skill(request)
    }

    pub fn begin_round(&mut self, request: BeginRoundRequest) -> Result<BeginRoundReply, AppError> {
        self.active
            .as_mut()
            .ok_or(AppError::InvalidRequest)?
            .begin_round(request)
    }

    pub fn plan_auto_round(&self, request: &AutoRoundRequest) -> Result<AutoRoundReply, AppError> {
        Ok(self
            .active
            .as_ref()
            .ok_or(AppError::InvalidRequest)?
            .plan_auto_round(request))
    }

    pub fn replay_episode_id(&self) -> Result<i32, AppError> {
        self.active
            .as_ref()
            .filter(|active| active.is_replay.unwrap_or(false))
            .map(|active| active.episode_id)
            .ok_or(AppError::InvalidRequest)
    }
}

#[derive(Debug, Clone)]
pub struct PendingDungeonRecord {
    pub episode_id: i32,
    pub round: i32,
    pub record: database::db::game::dungeons::PreparedDungeonRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CommittedRound {
    request: BeginRoundRequest,
    cloth_skill_opers: Vec<UseClothSkillOperRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BattleCheckpoint {
    chapter_id: i32,
    start_request: StartDungeonRequest,
    seed: u64,
    tower_context: Option<::battle::tower::BattleContext>,
    act229_context: Option<Act229BattleContext>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Act229BattleContext {
    pub activity_id: i32,
    pub stage_id: i32,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
/// Session and persistence wrapper around the authoritative `BattleRuntime`.
/// Gameplay semantics stay in `battle`; this type supplies inputs and records server metadata.
pub struct ActiveBattle {
    pub tower_type: Option<i32>,
    pub tower_id: Option<i32>,
    pub layer_id: Option<i32>,
    pub episode_id: i32,
    pub chapter_id: i32,
    pub difficulty: Option<i32>,
    pub talent_plan_id: Option<i32>,
    pub team_level: Option<i32>,
    pub assist_boss_level: Option<i32>,
    pub battle_id: i32,
    pub runtime: ::battle::engine::runtime::BattleRuntime,
    pub fight_group: Option<sonettobuf::FightGroup>,
    pub fight_id: Option<i64>,
    pub is_replay: Option<bool>,
    pub replay_episode_id: Option<i32>,
    pub multiplication: Option<i32>,
    pub params: Option<String>,
    pub ai_deck: Vec<sonettobuf::CardInfo>,
    pub(crate) seed: u64,
    pub(crate) start_request: Option<StartDungeonRequest>,
    pub(crate) tower_context: Option<::battle::tower::BattleContext>,
    pub(crate) act229_context: Option<Act229BattleContext>,
    pub(crate) rounds: Vec<CommittedRound>,
    pub(crate) pending_cloth_skill_opers: Vec<UseClothSkillOperRecord>,
}

impl ActiveBattle {
    pub fn is_victory(&self) -> bool {
        self.runtime.outcome() == ::battle::engine::runtime::BattleOutcome::Victory
    }

    pub fn current_round(&self) -> i32 {
        self.runtime.current_round()
    }

    pub fn star(&self) -> i32 {
        crate::dungeon::battle_star(&self.runtime, self.battle_id)
    }

    pub fn plan_auto_round(&self, request: &AutoRoundRequest) -> AutoRoundReply {
        self.runtime.plan_auto_round(request)
    }

    pub fn use_cloth_skill(
        &mut self,
        request: UseClothSkillRequest,
    ) -> Result<(UseClothSkillReply, Option<RedealCardInfoPush>), AppError> {
        let reply = self
            .runtime
            .use_cloth_skill(request)
            .ok_or(AppError::InvalidRequest)?;
        self.pending_cloth_skill_opers
            .push(UseClothSkillOperRecord {
                skill_id: request.skill_id,
                from_id: request.from_id,
                to_id: request.to_id,
                r#type: request.r#type,
            });
        Ok((reply, self.runtime.take_redeal_card_push()))
    }

    pub async fn prepare(
        pool: &SqlitePool,
        player_id: i64,
        episode_id: i32,
        battle_id: i32,
        request: StartDungeonRequest,
    ) -> Result<Self, AppError> {
        let use_record = request.use_record.unwrap_or(false);
        let fight_group = request
            .fight_group
            .as_ref()
            .ok_or(AppError::InvalidRequest)?;
        let built = ::battle::dungeon::build_fight(
            pool,
            player_id,
            episode_id,
            battle_id,
            use_record,
            fight_group,
            request.params.as_deref(),
        )
        .await?;
        Self::prepare_from_built(request, built, None, None)
    }

    pub async fn from_built(
        pool: &SqlitePool,
        player_id: i64,
        request: StartDungeonRequest,
        built: ::battle::dungeon::BuiltFight,
        tower_context: Option<::battle::tower::BattleContext>,
    ) -> Result<Self, AppError> {
        Self::persist_built(pool, player_id, request, built, tower_context, None).await
    }

    pub fn prepare_act229(
        request: StartDungeonRequest,
        built: ::battle::dungeon::BuiltFight,
        context: Act229BattleContext,
    ) -> Result<Self, AppError> {
        Self::prepare_from_built(request, built, None, Some(context))
    }

    async fn persist_built(
        pool: &SqlitePool,
        player_id: i64,
        request: StartDungeonRequest,
        built: ::battle::dungeon::BuiltFight,
        tower_context: Option<::battle::tower::BattleContext>,
        act229_context: Option<Act229BattleContext>,
    ) -> Result<Self, AppError> {
        let mut active = Self::prepare_from_built(request, built, tower_context, act229_context)?;
        let checkpoint = active.checkpoint_json()?;
        active.fight_id = Some(
            battle::create_fight_instance(
                pool,
                battle::NewFightInstance {
                    user_id: player_id,
                    episode_id: active.episode_id,
                    battle_id: active.battle_id,
                    multiplication: active.multiplication.unwrap_or(1).max(1),
                    entry_cost: "{}",
                    checkpoint: &checkpoint,
                    created_at: ServerTime::now_ms(),
                },
            )
            .await?,
        );
        Ok(active)
    }

    fn prepare_from_built(
        request: StartDungeonRequest,
        built: ::battle::dungeon::BuiltFight,
        tower_context: Option<::battle::tower::BattleContext>,
        act229_context: Option<Act229BattleContext>,
    ) -> Result<Self, AppError> {
        let episode_id = request.episode_id.ok_or(AppError::InvalidRequest)?;
        let battle_id = built
            .fight
            .battle_id
            .filter(|battle_id| *battle_id > 0)
            .ok_or(AppError::InvalidRequest)?;
        let seed = rand::random();
        Self::from_built_with_seed(
            episode_id,
            battle_id,
            request,
            built,
            tower_context,
            act229_context,
            seed,
        )
    }

    pub async fn activate(
        &mut self,
        pool: &SqlitePool,
        player_id: i64,
        costs: &RewardSet,
    ) -> Result<ConsumedRewards, AppError> {
        if self.fight_id.is_some() {
            return Err(AppError::InvalidRequest);
        }

        let checkpoint = self.checkpoint_json()?;
        let entry_cost = serde_json::to_string(costs)?;
        let mut tx = pool.begin().await?;
        let consumed = reward::RewardManager::new(player_id)
            .consume(&mut tx, costs)
            .await?;
        let fight_id = battle::create_fight_instance_in_transaction(
            &mut tx,
            battle::NewFightInstance {
                user_id: player_id,
                episode_id: self.episode_id,
                battle_id: self.battle_id,
                multiplication: self.multiplication.unwrap_or(1).max(1),
                entry_cost: &entry_cost,
                checkpoint: &checkpoint,
                created_at: ServerTime::now_ms(),
            },
        )
        .await?;
        tx.commit().await?;
        self.fight_id = Some(fight_id);
        Ok(consumed)
    }

    fn from_built_with_seed(
        episode_id: i32,
        battle_id: i32,
        request: StartDungeonRequest,
        built: ::battle::dungeon::BuiltFight,
        tower_context: Option<::battle::tower::BattleContext>,
        act229_context: Option<Act229BattleContext>,
        seed: u64,
    ) -> Result<Self, AppError> {
        let chapter_id = request.chapter_id.unwrap_or_else(|| {
            config::configs::get()
                .episode
                .get(episode_id)
                .map(|episode| episode.chapter_id)
                .unwrap_or_default()
        });
        let use_record = request.use_record.unwrap_or(false);
        let fight_group = request
            .fight_group
            .clone()
            .ok_or(AppError::InvalidRequest)?;
        let attacker = built.fight.attacker.as_ref();
        let team_level = attacker.and_then(average_team_level);
        let assist_boss_level = attacker
            .and_then(|team| team.assist_boss.as_ref())
            .and_then(|boss| boss.level);
        let mut runtime = ::battle::engine::runtime::BattleRuntime::new_with_attributes(
            built.fight,
            built.ex_attributes,
            built.sp_attributes,
        );
        runtime.extend_battle_rule_skills(built.battle_rule_skills);
        runtime
            .start_round_with_determinism(
                ::battle::engine::runtime::determinism::RoundDeterminism::with_seed(seed),
            )
            .map_err(AppError::Custom)?;

        Ok(Self {
            tower_type: tower_context.map(|context| context.tower_type),
            tower_id: tower_context.map(|context| context.tower_id),
            layer_id: tower_context.map(|context| context.layer_id),
            difficulty: tower_context.map(|context| context.difficulty),
            talent_plan_id: tower_context.map(|context| context.talent_plan_id),
            episode_id,
            chapter_id,
            battle_id,
            runtime,
            fight_group: Some(fight_group),
            fight_id: None,
            is_replay: Some(use_record),
            multiplication: request.multiplication,
            params: request.params.clone(),
            team_level,
            assist_boss_level,
            seed,
            start_request: Some(request),
            tower_context,
            act229_context,
            ..Default::default()
        })
    }

    pub async fn restore(
        pool: &SqlitePool,
        player_id: i64,
        record: database::db::game::battle::ActiveFightRecord,
    ) -> Result<Self, AppError> {
        let checkpoint: BattleCheckpoint = serde_json::from_str(&record.checkpoint)
            .map_err(|error| AppError::InvalidBattleCheckpoint(error.to_string()))?;
        let episode_id = checkpoint.start_request.episode_id.ok_or_else(|| {
            AppError::InvalidBattleCheckpoint("start request has no episode".into())
        })?;
        if episode_id != record.episode_id {
            return Err(AppError::InvalidBattleCheckpoint(
                "checkpoint episode does not match fight instance".into(),
            ));
        }
        let fight_group = checkpoint
            .start_request
            .fight_group
            .as_ref()
            .ok_or_else(|| {
                AppError::InvalidBattleCheckpoint("start request has no fight group".into())
            })?;
        let use_record = checkpoint.start_request.use_record.unwrap_or(false);
        let built = if let Some(context) = checkpoint.tower_context {
            ::battle::tower::build_fight(
                pool,
                player_id,
                episode_id,
                record.battle_id,
                use_record,
                fight_group,
                context,
            )
            .await?
        } else {
            ::battle::dungeon::build_fight(
                pool,
                player_id,
                episode_id,
                record.battle_id,
                use_record,
                fight_group,
                checkpoint.start_request.params.as_deref(),
            )
            .await?
        };
        let mut active = Self::from_built_with_seed(
            episode_id,
            record.battle_id,
            checkpoint.start_request,
            built,
            checkpoint.tower_context,
            checkpoint.act229_context,
            checkpoint.seed,
        )?;
        active.fight_id = Some(record.id);
        Ok(active)
    }

    pub fn checkpoint_json(&self) -> Result<String, AppError> {
        Ok(serde_json::to_string(&BattleCheckpoint {
            chapter_id: self.chapter_id,
            start_request: self.start_request.clone().ok_or(AppError::InvalidRequest)?,
            seed: self.seed,
            tower_context: self.tower_context,
            act229_context: self.act229_context,
        })?)
    }

    pub fn reconnect_reply(&self) -> ReconnectFightReply {
        let (fight, last_round) = self.runtime.reconnect_state();
        let data = self
            .act229_context
            .map(|context| {
                serde_json::json!({
                    "episodeId": self.episode_id,
                    "stageId": context.stage_id,
                })
                .to_string()
            })
            .or_else(|| self.params.clone());
        ReconnectFightReply {
            fight: Some(fight),
            last_round,
            fight_reason: Some(FightReason {
                r#type: Some(if self.is_replay.unwrap_or(false) {
                    fight_reason::FightType::DungeonRecord as i32
                } else {
                    fight_reason::FightType::Dungeon as i32
                }),
                content: Some(self.episode_id.to_string()),
                battle_id: Some(self.battle_id),
                multiplication: self.multiplication,
                data,
            }),
            fight_group: self.fight_group.clone(),
        }
    }

    pub fn act229_heroes(&self) -> Vec<Act229HeroNo> {
        let Some(group) = self.fight_group.as_ref() else {
            return Vec::new();
        };
        group
            .hero_list
            .iter()
            .filter_map(|uid| {
                let hero_id = self.runtime.entity_info(*uid)?.model_id;
                let equip_uids = group
                    .equips
                    .iter()
                    .find(|equip| equip.hero_uid == Some(*uid))
                    .map(|equip| equip.equip_uid.clone())
                    .unwrap_or_default();
                Some(Act229HeroNo {
                    hero_id,
                    equip_uids,
                })
            })
            .collect()
    }

    pub fn oper_records(&self) -> Vec<FightRoundOperRecord> {
        self.rounds
            .iter()
            .map(|round| FightRoundOperRecord {
                cloth_skill_opers: round.cloth_skill_opers.clone(),
                opers: round.request.opers.clone(),
            })
            .collect()
    }

    pub fn start_reply(&self) -> StartDungeonReply {
        ::battle::dungeon::start_reply(&self.runtime)
    }

    pub fn card_info_push(&self) -> CardInfoPush {
        self.runtime.card_info_push()
    }

    pub fn begin_round(&mut self, request: BeginRoundRequest) -> Result<BeginRoundReply, AppError> {
        let reply = ::battle::dungeon::begin_round(&mut self.runtime, request.clone())
            .map_err(AppError::Custom)?;
        self.record_round(request);
        compress_round_steps(reply)
    }

    fn record_round(&mut self, request: BeginRoundRequest) {
        self.rounds.push(CommittedRound {
            request,
            cloth_skill_opers: std::mem::take(&mut self.pending_cloth_skill_opers),
        });
    }
}

fn average_team_level(team: &sonettobuf::FightTeam) -> Option<i32> {
    let (sum, count) = team
        .entitys
        .iter()
        .filter_map(|entity| entity.level)
        .fold((0, 0), |(sum, count), level| (sum + level, count + 1));
    (count > 0).then(|| sum / count)
}

/// Applies client transport framing after the battle reply is complete.
/// Framing may encode and compress steps but never filter, reorder, or synthesize them.
fn compress_round_steps(mut reply: BeginRoundReply) -> Result<BeginRoundReply, AppError> {
    let Some(round) = reply.round.as_mut() else {
        return Ok(reply);
    };
    let step_count = i32::try_from(round.fight_step.len())
        .map_err(|_| AppError::Custom("fight step count exceeds i32".to_owned()))?;
    let mut framed = Vec::new();
    framed.extend_from_slice(&step_count.to_be_bytes());
    for step in &round.fight_step {
        let bytes = step.encode_to_vec();
        let len = i32::try_from(bytes.len())
            .map_err(|_| AppError::Custom("encoded fight step exceeds i32".to_owned()))?;
        framed.extend_from_slice(&len.to_be_bytes());
        framed.extend_from_slice(&bytes);
    }

    let mut encoder = GzEncoder::new(framed.as_slice(), Compression::default());
    let mut compressed = Vec::new();
    encoder.read_to_end(&mut compressed)?;
    round.total_step = Some(step_count);
    round.fight_step_bytes = Some(compressed);
    round.fight_step.clear();
    Ok(reply)
}

#[cfg(test)]
mod test;
