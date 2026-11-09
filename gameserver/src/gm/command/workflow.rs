use std::collections::BTreeMap;

use anyhow::Result;
use database::db::game::dungeons;
use muipserver::GmResponse;
use sonettobuf::{
    ChapterMapElementUpdatePush, ChapterMapUpdatePush, CmdId, DungeonUpdatePush, MapElementReply,
    RewardPointUpdatePush, StoryFinishPush, UpdateBgmPush, UpdateGuidePush, UpdateOpenPush,
};

use crate::{logic::reward, net::app::AppState};

use super::{
    grant::{GrantData, RewardData, parse_positive},
    push::{send_material_push, send_push, send_snapshot_pushes},
};

pub(super) async fn grant_hero_upgrade_materials(
    state: &'static AppState,
    player_id: i64,
    args: &[&str],
) -> Result<GmResponse> {
    let [upgrade, materials, values @ ..] = args else {
        anyhow::bail!(
            "usage: hero upgrade materials <hero id> <1-180> <resonance 1-15> [destiny rank] [destiny stone id]"
        );
    };
    if !upgrade.eq_ignore_ascii_case("upgrade") || !materials.eq_ignore_ascii_case("materials") {
        anyhow::bail!(
            "usage: hero upgrade materials <hero id> <1-180> <resonance 1-15> [destiny rank] [destiny stone id]"
        );
    }
    let (hero_id, target_level, target_talent, destiny_target) = match values {
        [hero_id, target_level, target_talent] => (hero_id, target_level, target_talent, None),
        [hero_id, target_level, target_talent, destiny_rank, stone_id] => (
            hero_id,
            target_level,
            target_talent,
            Some(crate::logic::hero::DestinyMaterialTarget {
                rank: destiny_rank.parse::<i32>()?,
                stone_id: stone_id.parse::<i32>()?,
            }),
        ),
        _ => anyhow::bail!(
            "usage: hero upgrade materials <hero id> <1-180> <resonance 1-15> [destiny rank] [destiny stone id]"
        ),
    };
    let hero_id = hero_id.parse::<i32>()?;
    let target_level = target_level
        .parse::<i32>()
        .ok()
        .filter(|level| (1..=config::configs::get().max_character_level()).contains(level))
        .ok_or_else(|| anyhow::anyhow!("target level must be between 1 and 180"))?;
    let target_talent = target_talent
        .parse::<i32>()
        .ok()
        .filter(|talent| (1..=15).contains(talent))
        .ok_or_else(|| anyhow::anyhow!("resonance must be between 1 and 15"))?;

    let rewards = crate::logic::hero::HeroManager::new(player_id)
        .upgrade_materials(
            state.db,
            hero_id,
            target_level,
            target_talent,
            destiny_target,
        )
        .await?;
    let material_changes = rewards.material_changes();
    if material_changes.is_empty() {
        return Ok(GmResponse::ok("no hero upgrade materials are required"));
    }
    let applied = reward::RewardManager::new(player_id)
        .apply(state.db, rewards)
        .await?;
    let mut data = GrantData {
        user_id: player_id,
        rewards: material_changes
            .into_iter()
            .map(|(kind, id, amount)| RewardData {
                r#type: kind as i32,
                id: id as i32,
                amount,
            })
            .collect(),
        changed_item_ids: Vec::new(),
        changed_power_item_ids: Vec::new(),
        changed_insight_item_ids: Vec::new(),
        changed_currency_ids: Vec::new(),
        changed_hero_ids: Vec::new(),
        changed_skin_ids: Vec::new(),
        changed_equip_ids: Vec::new(),
        cloth_updates: Vec::new(),
        player_info_changed: false,
    };
    data.merge_rewards(applied);
    data.deduplicate();
    send_material_push(state, player_id, data.rewards.iter().cloned()).await?;
    send_snapshot_pushes(state, &data).await?;

    let message = destiny_target.map_or_else(
        || {
            format!(
                "granted hero upgrade materials through level {target_level} and resonance {target_talent}"
            )
        },
        |destiny| {
            format!(
                "granted materials for destiny stone {} through rank {}",
                destiny.stone_id, destiny.rank
            )
        },
    );
    Ok(GmResponse::ok_data(message, data))
}

pub(super) async fn unlock_bgms(
    state: &'static AppState,
    player_id: i64,
    args: &[&str],
) -> Result<GmResponse> {
    if !matches!(args, [unlock, all] if unlock.eq_ignore_ascii_case("unlock") && all.eq_ignore_ascii_case("all"))
    {
        anyhow::bail!("usage: bgm unlock all");
    }

    let bgm_infos = crate::logic::preferences::PreferenceManager::new(player_id)
        .unlock_all_bgms(state.db)
        .await?;
    if bgm_infos.is_empty() {
        return Ok(GmResponse::ok("all BGM tracks are already unlocked"));
    }

    send_push(
        state,
        player_id,
        CmdId::UpdateBgmPushCmd,
        UpdateBgmPush {
            bgm_infos: bgm_infos.clone(),
        },
    )
    .await?;

    Ok(GmResponse::ok_data(
        format!("unlocked {} BGM tracks", bgm_infos.len()),
        bgm_infos,
    ))
}

pub(super) async fn complete_guides(
    state: &'static AppState,
    player_id: i64,
    args: &[&str],
) -> Result<GmResponse> {
    if !matches!(args, [complete, all] if complete.eq_ignore_ascii_case("complete") && all.eq_ignore_ascii_case("all"))
    {
        anyhow::bail!("usage: guide complete all");
    }

    let guide_infos = crate::logic::guide::GuideManager::new(player_id)
        .complete_all(state.db)
        .await?;
    send_push(
        state,
        player_id,
        CmdId::UpdateGuidePushCmd,
        UpdateGuidePush {
            guide_infos: guide_infos.clone(),
        },
    )
    .await?;

    Ok(GmResponse::ok_data(
        format!("completed {} guides", guide_infos.len()),
        guide_infos,
    ))
}

pub(in crate::gm) async fn unlock_dungeon(
    state: &'static AppState,
    player_id: i64,
    args: &[&str],
) -> Result<GmResponse> {
    if !args
        .first()
        .is_some_and(|arg| arg.eq_ignore_ascii_case("unlock"))
    {
        anyhow::bail!("usage: dungeon unlock <stage|chapter> <id>");
    }

    let kind = args.get(1).map(|arg| arg.to_ascii_lowercase());
    let id = args
        .get(2)
        .map(|id| parse_positive(id, "dungeon id"))
        .transpose()?;
    let target = id.map_or_else(
        || kind.clone().unwrap_or_default(),
        |id| format!("{} {id}", kind.as_deref().unwrap_or_default()),
    );
    let unlock = match (kind.as_deref(), id, args.len()) {
        (Some("stage" | "episode"), Some(id), 3) => {
            crate::logic::dungeon::DungeonManager::new(player_id)
                .unlock_stage(state.db, id)
                .await?
        }
        (Some("chapter"), Some(id), 3) => {
            crate::logic::dungeon::DungeonManager::new(player_id)
                .unlock_chapter(state.db, id)
                .await?
        }
        _ => anyhow::bail!("usage: dungeon unlock <stage|chapter> <id>"),
    };
    let crate::logic::dungeon::DungeonUnlock {
        changed,
        episodes,
        mut trails,
    } = unlock;
    if !changed {
        return Ok(GmResponse::ok(format!(
            "dungeon {target} is already unlocked"
        )));
    }

    let mut episodes = episodes;
    let mut material_totals = BTreeMap::<i32, BTreeMap<(u32, u32), i32>>::new();
    let mut reward_data = GrantData {
        user_id: player_id,
        rewards: Vec::new(),
        changed_item_ids: Vec::new(),
        changed_power_item_ids: Vec::new(),
        changed_insight_item_ids: Vec::new(),
        changed_currency_ids: Vec::new(),
        changed_hero_ids: Vec::new(),
        changed_skin_ids: Vec::new(),
        changed_equip_ids: Vec::new(),
        cloth_updates: Vec::new(),
        player_info_changed: false,
    };
    for episode in &mut episodes {
        for (kind, id, amount) in std::mem::take(&mut episode.material_changes) {
            let total = material_totals
                .entry(episode.chapter_id)
                .or_default()
                .entry((kind, id))
                .or_default();
            *total = total.saturating_add(amount);
        }
        reward_data.merge_rewards(std::mem::take(&mut episode.rewards));
    }
    for (chapter_id, changes) in std::mem::take(&mut trails.material_changes) {
        for (kind, id, amount) in changes {
            let total = material_totals
                .entry(chapter_id)
                .or_default()
                .entry((kind, id))
                .or_default();
            *total = total.saturating_add(amount);
        }
    }
    reward_data.merge_rewards(std::mem::take(&mut trails.rewards));
    for rewards in material_totals.into_values() {
        let rewards = rewards
            .into_iter()
            .map(|((kind, id), amount)| RewardData {
                r#type: kind as i32,
                id: id as i32,
                amount,
            })
            .collect::<Vec<_>>();
        send_material_push(state, player_id, rewards.iter().cloned()).await?;
        reward_data.rewards.extend(rewards);
    }
    reward_data.deduplicate();
    send_snapshot_pushes(state, &reward_data).await?;
    for element_id in &trails.finished_element_ids {
        send_push(
            state,
            player_id,
            CmdId::MapElementCmd,
            MapElementReply {
                element_id: Some(*element_id),
                dialog_ids: Vec::new(),
                record: None,
            },
        )
        .await?;
    }
    for (chapter_id, value) in &trails.reward_points {
        send_push(
            state,
            player_id,
            CmdId::RewardPointUpdatePushCmd,
            RewardPointUpdatePush {
                chapter_id: Some(*chapter_id),
                value: Some(*value),
            },
        )
        .await?;
    }

    let chapter_type_nums = dungeons::get_chapter_type_nums(state.db, player_id)
        .await?
        .into_iter()
        .map(Into::into)
        .collect::<Vec<sonettobuf::UserChapterTypeNum>>();
    let completed_episode_ids = episodes
        .iter()
        .filter_map(|episode| episode.dungeon.as_ref().map(|dungeon| dungeon.episode_id))
        .collect::<Vec<_>>();
    let mut finished_story_ids = Vec::new();
    let mut unlocked_open_ids = Vec::new();
    for episode in episodes {
        for story_id in episode.finished_story_ids {
            send_push(
                state,
                player_id,
                CmdId::StoryFinishPushCmd,
                StoryFinishPush {
                    story_id: Some(story_id),
                },
            )
            .await?;
            finished_story_ids.push(story_id);
        }
        if !episode.open_infos.is_empty() {
            unlocked_open_ids.extend(
                episode
                    .open_infos
                    .iter()
                    .filter(|info| info.is_open)
                    .map(|info| info.id),
            );
            send_push(
                state,
                player_id,
                CmdId::UpdateOpenPushCmd,
                UpdateOpenPush {
                    open_infos: episode.open_infos,
                },
            )
            .await?;
        }
        if let Some(dungeon_info) = episode.dungeon {
            send_push(
                state,
                player_id,
                CmdId::DungeonUpdatePushCmd,
                DungeonUpdatePush {
                    dungeon_info: Some(dungeon_info.into()),
                    chapter_type_nums: chapter_type_nums.clone(),
                },
            )
            .await?;
        }
    }
    let (map_ids, elements) = dungeons::reconcile_map_progression(state.db, player_id).await?;
    if !map_ids.is_empty() {
        send_push(
            state,
            player_id,
            CmdId::ChapterMapUpdatePushCmd,
            ChapterMapUpdatePush { map_ids },
        )
        .await?;
    }
    if !elements.is_empty() {
        send_push(
            state,
            player_id,
            CmdId::ChapterMapElementUpdatePushCmd,
            ChapterMapElementUpdatePush { elements },
        )
        .await?;
    }

    Ok(GmResponse::ok_data(
        format!("unlocked dungeon {target}"),
        serde_json::json!({
            "completedEpisodes": completed_episode_ids,
            "passedPrerequisiteEpisodes": completed_episode_ids,
            "finishedTrails": trails.finished_element_ids,
            "finishedStories": finished_story_ids,
            "unlockedOpenIds": unlocked_open_ids,
        }),
    ))
}
