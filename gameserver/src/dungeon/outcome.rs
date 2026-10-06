use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FightResult {
    Abort = -1,
    Fail = 0,
    Succ = 1,
    OutOfRoundFail = 2,
}

pub fn end_fight(active: &ActiveBattle, result: FightResult) -> EndFightPush {
    EndFightPush {
        record: Some(FightRecord {
            fight_id: active.fight_id,
            fight_name: Some(String::new()),
            fight_time: Some(ServerTime::now_ms()),
            fight_result: Some(result as i32),
            attack_statistics: active.runtime.attack_statistics(),
            defense_statistics: Vec::new(),
        }),
        fight_group_a: Some(active.fight_group.clone().unwrap_or_default()),
        is_record: Some(active.is_replay.unwrap_or(false)),
    }
}

pub fn abort_end_fight(active: &ActiveBattle) -> EndFightPush {
    end_fight(active, FightResult::Abort)
}

pub async fn abort_dungeon_updates(
    db: &SqlitePool,
    player_id: i64,
    active: &ActiveBattle,
) -> Result<(DungeonUpdatePush, EndDungeonPush), AppError> {
    let dungeon =
        dungeons::get_user_dungeon_state(db, player_id, active.chapter_id, active.episode_id)
            .await?;
    let chapter_type_nums = dungeons::get_chapter_type_nums(db, player_id).await?;

    Ok((
        DungeonUpdatePush {
            dungeon_info: Some(dungeon.into()),
            chapter_type_nums: chapter_type_nums.into_iter().map(Into::into).collect(),
        },
        EndDungeonPush {
            chapter_id: Some(active.chapter_id),
            episode_id: Some(active.episode_id),
            star: Some(0),
            total_round: Some(active.runtime.current_round()),
            extra_str: Some(String::new()),
            assist_user_id: Some(0),
            update_dungeon_record: Some(false),
            can_update_dungeon_record: Some(false),
            first_pass: Some(false),
            ..Default::default()
        },
    ))
}

pub fn completed_end_fight(active: &ActiveBattle) -> EndFightPush {
    let result = match active.runtime.outcome() {
        battle::BattleOutcome::Victory => FightResult::Succ,
        battle::BattleOutcome::OutOfRounds => FightResult::OutOfRoundFail,
        battle::BattleOutcome::Defeat | battle::BattleOutcome::Unfinished => FightResult::Fail,
    };
    end_fight(active, result)
}

pub fn dungeon_pass_types(chapter_id: i32) -> Vec<i32> {
    let Some(chapter) = configs::get().chapter.get(chapter_id) else {
        return Vec::new();
    };

    let mut types = vec![chapter.r#type];
    if chapter.act_id > 0 && chapter.r#type != 9999 {
        types.push(9999);
    }
    types
}
