use crate::{error::AppError, reward};
use database::db::game::{
    activity_state::{self, ActivityStateKind, ActivityStateSet},
    activity101,
};
use serde::{Deserialize, Serialize};
use sonettobuf::{
    AcceptAct186SpBonusReply, Act101Info, Act101SpInfo, Act104EpisodeNo, Act104PreSummaryNo,
    Act104RetailNo, Act104SpecialNo, Act104TrialNo, Act123EpisodeNo, Act123RetailNo, Act123StageNo,
    Act125Episode, Act146Episode, Act146EpisodeBonusReply, Act160FinishMissionReply,
    Act160GetInfoReply, Act160MissionInfo, Act165GainMilestoneRewardReply,
    Act165GenerateEndingReply, Act165GetInfoReply, Act165ModifyKeywordReply, Act165RestartReply,
    Act165StoryInfo, Act172Info, Act186GameInfo, Act186Info, Act186LikeInfo, Act186TaskInfo,
    Act205GetGameInfoReply, Act205GetInfoReply, Act206ChooseDirectionReply, Act206GetInfoReply,
    Act208BonusNo, Act208ReceiveBonusReply, Act212BonusNo, Act212InfoNo, Act212ReceiveBonusReply,
    Act218FinishGameReply, Act221SummonReply, Act228FlipGridGridReply, Act228GetFinalBonusReply,
    Act228Info, Act229BattleFinishPush, Act229HeroNo, Act229ResetStageReply, ActivityInfo,
    ActivityNewStageReadReply, EndingInfo, FinishAct125EpisodeReply, FinishAct146EpisodeReply,
    Get101BonusListReply, Get101BonusReply, Get101InfosReply, Get101SpBonusReply, Get104InfosReply,
    Get123InfosReply, Get128InfosReply, Get136InfoReply, Get152InfoReply, Get153InfosReply,
    Get154InfosReply, Get158InfosReply, Get166InfosReply, Get196InfoReply, Get197InfoReply,
    Get199InfoReply, Get217InfosReply, Get218InfoReply, Get221InfoReply, GetAct125InfosReply,
    GetAct146InfosReply, GetAct172InfoReply, GetAct186InfoReply, GetAct186SpBonusInfoReply,
    GetAct189InfoReply, GetAct189OnceBonusReply, GetAct208InfoReply, GetAct209InfoReply,
    GetAct212InfoReply, GetAct216InfoReply, GetAct225InfoReply, GetAct228InfoReply,
    GetAct229InfoReply, GetActivityInfosReply, GetActivityInfosWithParamReply,
    MarkActivity104StoryReply, MarkEpisodeAfterStoryReply, MarkPopSummaryReply, StepInfo,
    UnlockPermanentReply,
};
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};

mod catalog;
mod manager;
mod schedule;
pub use manager::ActivityManager;

mod act101;
mod act104;
mod act123;
mod act125;
mod act128;
mod act136;
mod act146;
mod act152;
mod act154;
mod act158;
mod act160;
mod act165;
mod act166;
mod act172;
mod act186;
mod act189;
mod act196;
mod act197;
mod act198;
mod act199;
mod act205;
mod act206;
mod act208;
mod act209;
mod act212;
mod act216;
mod act217;
mod act218;
mod act221;
mod act225;
mod act228;
mod act229;
mod dice_hero;

use act101::{get101_bonus, get101_bonus_list, get101_infos, get101_sp_bonus};
use act104::{act104_infos, mark_activity104_story, mark_episode_after_story, mark_pop_summary};
use act123::{act123_infos, act153_infos};
use act125::{act125_infos, finish_act125_episode};
use act128::act128_info;
pub use act128::settle_act128_score_in_transaction;
use act136::{act136_info, act136_select};
use act146::{act146_episode_bonus, act146_infos, finish_act146_episode};
use act152::{accept_act152_present, act152_info};
use act154::{act154_infos, answer154_puzzle};
use act158::act158_infos;
use act160::{act160_get_info, finish_act160_mission};
use act165::{
    act165_gain_milestone_reward, act165_generate_ending, act165_get_info, act165_modify_keyword,
    act165_restart,
};
use act166::act166_infos;
use act172::act172_info;
use act186::{accept_act186_sp_bonus, act186_info, get_act186_sp_bonus_info};
use act189::{act189_info, get_act189_once_bonus};
use act196::{act196_gain, act196_info};
use act197::{act197_explore, act197_info, act197_rummage};
use act198::act198_gain;
use act199::{act199_gain, act199_info};
use act205::{act205_finish_game, act205_get_game_info, act205_get_info};
use act206::{act206_choose_direction, act206_get_bonus, act206_get_info};
use act208::{act208_info, receive_act208_bonus};
use act209::act209_info;
use act212::{act212_info, receive_act212_bonus};
use act216::{act216_info, finish_act216_task, get_act216_once_bonus};
use act217::act217_infos;
use act218::{accept_act218_reward, act218_info, finish_act218_game};
use act221::{act221_info, act221_select, act221_summon};
use act225::act225_info;
use act228::{act228_flip_grid, act228_get_final_bonus, act228_info};
use act229::{
    act229_battle_episode, act229_heroes_available, act229_info, finish_act229_battle,
    reset_act229_stage,
};
use catalog::*;
