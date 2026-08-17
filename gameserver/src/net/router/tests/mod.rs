use super::*;
use crate::{
    net::{app::AppState, outbound::CommandPacket, packet::ClientPacket},
    player::{Player, PlayerState},
};
use config::configs;
use prost::Message;
use sonettobuf::{
    Act128GetMilestoneBonusReply, Act128GetMilestoneBonusRequest, Act220EpisodeRecord,
    Act233BpScoreUpdatePush, Act236GetAutoGainRewardReply, Act236GetAutoGainRewardRequest,
    Act236Info, Act236UpdateInfoPush, Act239BonusReply, Act239BonusRequest, ArcadeAttrChangePush,
    ArcadeAttrContainer, ArcadeAttrValue, ArcadeBook, ArcadeBookInfo, ArcadeExtendInfo,
    ArcadeGainRewardReply, ArcadeGainRewardRequest, ArcadeGetInSideInfoReply,
    ArcadeGetInSideInfoRequest, ArcadeGetOutSideInfoReply, ArcadeGetOutSideInfoRequest,
    ArcadeInSideInfo, ArcadeInSideProp, ArcadePlayer, ArcadeSaveGameReply, ArcadeSaveGameRequest,
    ArcadeSettleGameReply, ArcadeSettleGameRequest, ArcadeTalentUpgradeReply,
    ArcadeTalentUpgradeRequest, CurrencyChangePush, FinishTaskReply, FinishTaskRequest,
    GainInviteRewardReply, GainInviteRewardRequest, GetAct220InfoReply, GetAct220InfoRequest,
    GetAct233BpBonusReply, GetAct233BpBonusRequest, GetAct233BpInfoReply, GetAct233BpInfoRequest,
    GetAct236InfoReply, GetAct236InfoRequest, GetAct239InfoReply, GetAct239InfoRequest,
    GetHeroInvitationInfoReply, GetHeroInvitationInfoRequest, GetInvestigateReply,
    GetInvestigateRequest, GetRouge2OutsideInfoReply, GetRouge2OutsideInfoRequest, ItemChangePush,
    MarkPopShallowSettleReply, MarkPopShallowSettleRequest, MaterialChangePush, NewOrderRequest,
    PutClueReply, PutClueRequest, Rouge2AlchemyInfo, Rouge2AlchemyMaterialInfo,
    Rouge2BossBattleInfo, Rouge2CareerLevelInfo, Rouge2OutsideInfo, Rouge2RewardInfo,
    Rouge2TotalRecordInfo, TeachingGetBonusReply, TeachingGetBonusRequest, TeachingGetInfoReply,
    TeachingGetInfoRequest, UpdateRedDotPush, UpdateTaskPush,
};
use sqlx::SqlitePool;
use tokio::sync::mpsc;

async fn complete_teaching(pool: &SqlitePool, player_id: i64, teaching_id: i32) {
    for episode in configs::get()
        .teaching_episode
        .iter()
        .filter(|episode| episode.teaching == teaching_id)
    {
        let chapter_id = configs::get().episode.get(episode.id).unwrap().chapter_id;
        sqlx::query(
            "INSERT INTO user_dungeons
             (user_id, chapter_id, episode_id, star, challenge_count, has_record,
              left_return_all_num, today_pass_num, today_total_num, created_at, updated_at)
             VALUES (?, ?, ?, 1, 0, 0, 1, 0, 0, 0, 0)",
        )
        .bind(player_id)
        .bind(chapter_id)
        .bind(episode.id)
        .execute(pool)
        .await
        .unwrap();
    }
}

mod activity_events;
mod activity_pass;
mod activity_rewards;
mod arcade;
mod progression;
mod social;
mod teaching;
