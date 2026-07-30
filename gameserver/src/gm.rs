use anyhow::Result;
use database::{
    db::game::{block_packages, currencies, dungeons, equipment, items},
    models::game::{currencies::UserCurrencyModel, heros::UserHeroModel, items::UserItemModel},
};
use muipserver::{GmRequest, GmResponse, MaterialQuery};
use serde::Serialize;
use sonettobuf::{
    BlockPackageGainPush, ChapterMapElementUpdatePush, ChapterMapUpdatePush, CmdId,
    CurrencyChangePush, DungeonUpdatePush, EquipUpdatePush, HeroSkinGainPush, HeroUpdatePush,
    ItemChangePush, MapElementReply, MaterialChangePush, MaterialData, PlayerCardInfoPush,
    PlayerCloth, PlayerClothInfo, RewardPointUpdatePush, StoryFinishPush, UpdateBgmPush,
    UpdateGuidePush, UpdateOpenPush, prost::Message,
};
use std::collections::{BTreeMap, HashSet};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};
use tracing::{info, warn};

use crate::{
    logic::reward,
    net::{app::AppState, outbound::CommandPacket},
};

pub async fn run_gm_listener(addr: String, state: &'static AppState) -> std::io::Result<()> {
    let listener = TcpListener::bind(&addr).await?;
    info!("MUIP GM bridge listening on {}", listener.local_addr()?);

    loop {
        let (stream, peer) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(err) = handle_connection(stream, state).await {
                warn!("MUIP GM connection {peer} failed: {err}");
            }
        });
    }
}

async fn handle_connection(stream: TcpStream, state: &'static AppState) -> std::io::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    reader.read_line(&mut line).await?;

    let trimmed = line.trim();
    if trimmed.is_empty() {
        return write_response(&mut writer, GmResponse::err(400, "empty GM request")).await;
    }
    if is_http_request(trimmed) {
        return write_http_redirect(&mut writer).await;
    }

    let request = match serde_json::from_str::<GmRequest>(trimmed) {
        Ok(request) => request,
        Err(err) => {
            return write_response(
                &mut writer,
                GmResponse::err(400, format!("invalid GM request: {err}")),
            )
            .await;
        }
    };

    let response = match request {
        GmRequest::Status => status(state).await,
        GmRequest::ListPlayers => list_players(state),
        GmRequest::Dungeons => dungeon_catalog(),
        GmRequest::Materials { query } => materials(state, query).await,
        GmRequest::Execute {
            player_uid,
            command,
        } => execute(state, player_uid, command).await,
    };

    write_response(&mut writer, response).await
}

async fn write_response(
    writer: &mut (impl AsyncWriteExt + Unpin),
    response: GmResponse,
) -> std::io::Result<()> {
    let mut payload = serde_json::to_vec(&response).map_err(std::io::Error::other)?;
    payload.push(b'\n');
    writer.write_all(&payload).await?;
    writer.flush().await
}

fn is_http_request(line: &str) -> bool {
    line.starts_with("GET ") || line.starts_with("HEAD ")
}

async fn write_http_redirect(writer: &mut (impl AsyncWriteExt + Unpin)) -> std::io::Result<()> {
    let location = format!("http://127.0.0.1:{}/", common::muip_port());
    let body = format!(
        "<!doctype html><meta http-equiv=\"refresh\" content=\"0;url={0}\"><a href=\"{0}\">Open MUIP panel</a>",
        location
    );
    let response = format!(
        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    writer.write_all(response.as_bytes()).await?;
    writer.flush().await
}

async fn status(state: &AppState) -> GmResponse {
    let online_players = state.online_player_ids();
    let player_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
        .fetch_one(state.db)
        .await
        .unwrap_or_default() as usize;

    let mut response = GmResponse::ok("online");
    response.online = online_players.len();
    response.players = online_players
        .into_iter()
        .map(|id| id.to_string())
        .collect();
    response.data = Some(serde_json::json!({
        "online": response.online,
        "playerCount": player_count,
        "maxPlayers": 99999999,
        "players": response.players,
        "status": "online"
    }));
    response
}

fn list_players(state: &AppState) -> GmResponse {
    let players = state
        .online_player_ids()
        .into_iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>();

    let mut response = GmResponse::ok(format!("{} player(s) online", players.len()));
    response.online = players.len();
    response.players = players;
    response
}

fn dungeon_catalog() -> GmResponse {
    let tables = config::configs::get();
    let active_episodes = tables
        .story_episodes()
        .chain(tables.resource_episodes())
        .collect::<Vec<_>>();
    let active_chapter_ids = active_episodes
        .iter()
        .map(|episode| episode.chapter_id)
        .collect::<HashSet<_>>();
    let mut chapters = tables
        .chapter
        .iter()
        .filter(|chapter| {
            active_chapter_ids.contains(&chapter.id)
                && crate::logic::dungeon::chapter_missing_reward_heroes(chapter.id).is_empty()
        })
        .map(|chapter| DungeonCatalogChapter {
            id: chapter.id,
            name: resolve_name(
                tables,
                if chapter.name_en.is_empty() {
                    &chapter.name
                } else {
                    &chapter.name_en
                },
            ),
        })
        .collect::<Vec<_>>();
    let visible_chapter_ids = chapters
        .iter()
        .map(|chapter| chapter.id)
        .collect::<HashSet<_>>();
    let mut episodes = active_episodes
        .into_iter()
        .filter(|episode| visible_chapter_ids.contains(&episode.chapter_id))
        .map(|episode| DungeonCatalogEpisode {
            id: episode.id,
            chapter_id: episode.chapter_id,
            name: resolve_name(
                tables,
                if episode.name_en.is_empty() {
                    &episode.name
                } else {
                    &episode.name_en
                },
            ),
        })
        .collect::<Vec<_>>();
    chapters.sort_unstable_by_key(|chapter| chapter.id);
    episodes.sort_unstable_by_key(|episode| (episode.chapter_id, episode.id));

    GmResponse::ok_data("dungeons", DungeonCatalog { chapters, episodes })
}

async fn materials(state: &AppState, query: MaterialQuery) -> GmResponse {
    match material_catalog(state, query).await {
        Ok(catalog) => GmResponse::ok_data("materials", catalog),
        Err(err) => GmResponse::err(400, err.to_string()),
    }
}

async fn execute(state: &'static AppState, player_uid: String, command: String) -> GmResponse {
    let Ok(player_id) = player_uid.parse::<i64>() else {
        return GmResponse::err(400, format!("invalid player_uid `{player_uid}`"));
    };

    let exists = match sqlx::query_scalar::<_, i64>("SELECT 1 FROM users WHERE id = ?")
        .bind(player_id)
        .fetch_optional(state.db)
        .await
    {
        Ok(exists) => exists,
        Err(err) => return GmResponse::err(500, format!("database error: {err}")),
    };
    if exists.is_none() {
        return GmResponse::err(404, format!("player `{player_uid}` was not found"));
    }

    match run_command(state, player_id, &command).await {
        Ok(response) => response,
        Err(err) => GmResponse::err(400, err.to_string()),
    }
}

async fn run_command(
    state: &'static AppState,
    player_id: i64,
    command: &str,
) -> Result<GmResponse> {
    let parts = command.split_whitespace().collect::<Vec<_>>();
    let Some(first) = parts.first() else {
        anyhow::bail!("command is required");
    };

    let args = match first.to_ascii_lowercase().as_str() {
        "bgm" => return unlock_bgms(state, player_id, &parts[1..]).await,
        "dungeon" => return unlock_dungeon(state, player_id, &parts[1..]).await,
        "guide" | "guides" => return complete_guides(state, player_id, &parts[1..]).await,
        "material" | "reward" | "give" | "add" => &parts[1..],
        kind if MaterialKind::parse(kind).is_some() => &parts[..],
        "status" => return Ok(status(state).await),
        "players" | "list" | "listplayers" | "list_players" => return Ok(list_players(state)),
        "help" | "?" => {
            return Ok(GmResponse::ok(
                "commands: help, status, players, bgm unlock all, guide complete all, dungeon unlock <stage|chapter> <id>, material <type> <id> <amount>, give <item|currency|hero|skin|equip|power|insight> <id> <amount>",
            ));
        }
        _ => anyhow::bail!("unknown command '{}'", first),
    };

    grant(state, player_id, args).await
}

async fn unlock_bgms(
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

async fn complete_guides(
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

async fn unlock_dungeon(
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
    let episode_ids = episodes
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
            "passedPrerequisiteEpisodes": episode_ids,
            "finishedTrails": trails.finished_element_ids,
            "finishedStories": finished_story_ids,
            "unlockedOpenIds": unlocked_open_ids,
        }),
    ))
}

async fn grant(state: &'static AppState, player_id: i64, args: &[&str]) -> Result<GmResponse> {
    if args.len() != 3 {
        anyhow::bail!(
            "usage: material <type> <id> <amount> or give <item|currency|hero|skin|equip|power|package|insight> <id> <amount>"
        );
    }

    let kind = MaterialKind::parse(args[0])
        .ok_or_else(|| anyhow::anyhow!("unknown material type '{}'", args[0]))?;
    let id = parse_positive(args[1], "material id")?;
    let amount = parse_positive(args[2], "amount")?;

    let mut data = GrantData {
        user_id: player_id,
        rewards: vec![RewardData {
            r#type: kind.id(),
            id,
            amount,
        }],
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

    match kind {
        MaterialKind::Item => {
            let model = UserItemModel::new(player_id, (*state.db).clone());
            data.changed_item_ids = model.create_items(vec![(id as u32, amount)]).await?;
        }
        MaterialKind::Currency => {
            let model = UserCurrencyModel::new(player_id, (*state.db).clone());
            data.changed_currency_ids = model
                .create_currencies(&[(id, amount)])
                .await?
                .into_iter()
                .map(|(id, _)| id)
                .collect();
        }
        MaterialKind::PlayerExp => {
            let applied = reward::RewardManager::new(player_id)
                .apply(
                    state.db,
                    reward::RewardSet {
                        player_exp: amount,
                        ..Default::default()
                    },
                )
                .await?;
            data.merge_rewards(applied);
        }
        MaterialKind::Hero => {
            let model = UserHeroModel::new(player_id, (*state.db).clone());
            for _ in 0..amount {
                if model.has_hero(id).await? {
                    let duplicate_count = model.add_hero_duplicate(id).await?;
                    let rewards = reward::hero_duplicate_rewards(id, duplicate_count)?;
                    let applied = reward::RewardManager::new(player_id)
                        .apply(state.db, rewards)
                        .await?;
                    data.merge_rewards(applied);
                } else {
                    model.create_hero(id).await?;
                }
            }
            data.changed_hero_ids.push(id);
        }
        MaterialKind::Skin => {
            let model = UserHeroModel::new(player_id, (*state.db).clone());
            if model.unlock_skin(id).await? {
                data.changed_skin_ids.push(id);
            }
        }
        MaterialKind::Equipment => {
            data.changed_equip_ids =
                equipment::add_equipments(state.db, player_id, &[(id, amount)]).await?;
        }
        MaterialKind::PowerItem => {
            let model = UserItemModel::new(player_id, (*state.db).clone());
            data.changed_power_item_ids = model.create_power_items(vec![(id, amount)]).await?;
        }
        MaterialKind::BlockPackage => {
            block_packages::add_block_package(state.db, player_id, id).await?;
        }
        MaterialKind::InsightItem => {
            let model = UserItemModel::new(player_id, (*state.db).clone());
            data.changed_insight_item_ids = model.create_insight_items(vec![(id, amount)]).await?;
        }
    }

    send_material_push(state, data.user_id, data.rewards.iter().cloned()).await?;
    send_snapshot_pushes(state, &data).await?;

    Ok(GmResponse::ok_data(
        format!("granted {amount} of {}#{id} to {player_id}", kind.id()),
        data,
    ))
}

async fn send_material_push(
    state: &'static AppState,
    player_id: i64,
    rewards: impl IntoIterator<Item = RewardData>,
) -> Result<()> {
    let data_list = rewards
        .into_iter()
        .filter_map(|reward| {
            let kind = reward::RewardMaterialType::from_i32(reward.r#type)?;
            Some(MaterialData {
                materil_type: Some(kind.id()),
                materil_id: Some(reward.id as u32),
                quantity: Some(reward.amount),
            })
        })
        .collect::<Vec<_>>();
    if !data_list.is_empty() {
        send_push(
            state,
            player_id,
            CmdId::MaterialChangePushCmd,
            MaterialChangePush {
                data_list,
                get_approach: None,
            },
        )
        .await?;
    }
    Ok(())
}

async fn send_snapshot_pushes(state: &'static AppState, data: &GrantData) -> Result<()> {
    let mut changed_items = Vec::new();
    for item_id in &data.changed_item_ids {
        if let Some(item) = items::get_item(state.db, data.user_id, *item_id as u32).await? {
            changed_items.push(item.into());
        }
    }

    let mut changed_power_items = Vec::new();
    for item_id in &data.changed_power_item_ids {
        if let Some(item) = items::get_power_item(state.db, data.user_id, *item_id as u32).await? {
            changed_power_items.push(item.into());
        }
    }

    let mut changed_insight_items = Vec::new();
    for item_id in &data.changed_insight_item_ids {
        if let Some(item) = items::get_insight_item(state.db, data.user_id, *item_id as u32).await?
        {
            changed_insight_items.push(item.into());
        }
    }

    send_item_push(
        state,
        data.user_id,
        changed_items,
        changed_power_items,
        changed_insight_items,
    )
    .await?;

    let mut block_package_ids = HashSet::new();
    for reward in data
        .rewards
        .iter()
        .filter(|reward| MaterialKind::from_raw(reward.r#type) == Some(MaterialKind::BlockPackage))
        .filter(|reward| block_package_ids.insert(reward.id))
    {
        let packages = block_packages::get_block_packages(state.db, data.user_id)
            .await?
            .into_iter()
            .filter(|package| package.block_package_id == reward.id)
            .map(Into::into)
            .collect();
        send_push(
            state,
            data.user_id,
            CmdId::BlockPackageGainPushCmd,
            BlockPackageGainPush {
                block_packages: packages,
            },
        )
        .await?;
    }

    let mut change_currency = Vec::new();
    for currency_id in &data.changed_currency_ids {
        if let Some(currency) =
            currencies::get_currency(state.db, data.user_id, *currency_id).await?
        {
            change_currency.push(currency.into());
        }
    }

    if !change_currency.is_empty() {
        send_push(
            state,
            data.user_id,
            CmdId::CurrencyChangePushCmd,
            CurrencyChangePush { change_currency },
        )
        .await?;
    }

    let mut equips = Vec::new();
    for equip_uid in &data.changed_equip_ids {
        equips.push(
            equipment::get_equipment_by_uid(state.db, data.user_id, *equip_uid)
                .await?
                .into(),
        );
    }

    if !equips.is_empty() {
        send_push(
            state,
            data.user_id,
            CmdId::EquipUpdatePushCmd,
            EquipUpdatePush { equips },
        )
        .await?;
    }

    let hero_updates = crate::logic::hero::HeroManager::new(data.user_id)
        .snapshots(state.db, data.changed_hero_ids.iter().copied())
        .await?;

    if !hero_updates.is_empty() {
        send_push(
            state,
            data.user_id,
            CmdId::HeroHeroUpdatePushCmd,
            HeroUpdatePush { hero_updates },
        )
        .await?;
        let player_card = crate::logic::profile::ProfileManager::new(data.user_id)
            .card_info(state.db)
            .await?;
        send_push(
            state,
            data.user_id,
            CmdId::PlayerCardInfoPushCmd,
            PlayerCardInfoPush {
                player_card_info: player_card.player_card_info,
            },
        )
        .await?;
    }

    for skin_id in &data.changed_skin_ids {
        send_push(
            state,
            data.user_id,
            CmdId::HeroSkinGainPushCmd,
            HeroSkinGainPush {
                skin_id: Some(*skin_id),
                first_gain: Some(true),
                get_approach: None,
            },
        )
        .await?;
    }

    if data.player_info_changed {
        send_push(
            state,
            data.user_id,
            CmdId::PlayerInfoPushCmd,
            crate::logic::profile::ProfileManager::new(data.user_id)
                .snapshot(state.db)
                .await?,
        )
        .await?;
    }
    if !data.cloth_updates.is_empty() {
        send_push(
            state,
            data.user_id,
            CmdId::ClothUpdatePushCmd,
            sonettobuf::ClothUpdatePush {
                update_infos: Some(PlayerClothInfo {
                    clothes: data.cloth_updates.clone(),
                }),
            },
        )
        .await?;
    }

    Ok(())
}

async fn send_item_push(
    state: &'static AppState,
    player_id: i64,
    items: Vec<sonettobuf::Item>,
    power_items: Vec<sonettobuf::PowerItem>,
    insight_items: Vec<sonettobuf::InsightItem>,
) -> Result<()> {
    if items.is_empty() && power_items.is_empty() && insight_items.is_empty() {
        return Ok(());
    }

    send_push(
        state,
        player_id,
        CmdId::ItemChangePushCmd,
        ItemChangePush {
            items,
            power_items,
            insight_items,
            expire_items: Vec::new(),
            talent_items: Vec::new(),
        },
    )
    .await
}

async fn send_push<M: Message>(
    state: &'static AppState,
    player_id: i64,
    cmd_id: CmdId,
    message: M,
) -> Result<()> {
    let Some(sender) = state.get_session_sender(player_id) else {
        return Ok(());
    };

    let down_tag = state.reserve_down_tag().await;
    sender
        .send(CommandPacket::Push {
            cmd_id,
            body: message.encode_to_vec(),
            down_tag,
        })
        .await
        .map_err(|err| anyhow::anyhow!("failed to send MUIP push: {err}"))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GrantData {
    user_id: i64,
    rewards: Vec<RewardData>,
    changed_item_ids: Vec<i32>,
    changed_power_item_ids: Vec<i32>,
    changed_insight_item_ids: Vec<i32>,
    changed_currency_ids: Vec<i32>,
    changed_hero_ids: Vec<i32>,
    changed_skin_ids: Vec<i32>,
    changed_equip_ids: Vec<i64>,
    cloth_updates: Vec<PlayerCloth>,
    player_info_changed: bool,
}

impl GrantData {
    fn merge_rewards(&mut self, rewards: reward::AppliedRewards) {
        self.player_info_changed |= rewards.player_info_changed;
        self.changed_item_ids
            .extend(rewards.item_ids.into_iter().map(|id| id as i32));
        self.changed_power_item_ids.extend(rewards.power_item_ids);
        self.changed_insight_item_ids
            .extend(rewards.insight_item_ids);
        self.changed_currency_ids
            .extend(rewards.currency_ids.into_iter().map(|(id, _)| id));
        self.changed_hero_ids.extend(rewards.hero_ids);
        self.changed_skin_ids
            .extend(rewards.skin_gains.into_iter().map(|skin| skin.skin_id));
        self.changed_equip_ids.extend(rewards.equip_uids);
        self.cloth_updates.extend(rewards.cloth_updates);
    }

    fn deduplicate(&mut self) {
        self.changed_item_ids.sort_unstable();
        self.changed_item_ids.dedup();
        self.changed_power_item_ids.sort_unstable();
        self.changed_power_item_ids.dedup();
        self.changed_insight_item_ids.sort_unstable();
        self.changed_insight_item_ids.dedup();
        self.changed_currency_ids.sort_unstable();
        self.changed_currency_ids.dedup();
        self.changed_hero_ids.sort_unstable();
        self.changed_hero_ids.dedup();
        self.changed_skin_ids.sort_unstable();
        self.changed_skin_ids.dedup();
        self.changed_equip_ids.sort_unstable();
        self.changed_equip_ids.dedup();
        let mut cloth_ids = HashSet::new();
        self.cloth_updates.reverse();
        self.cloth_updates
            .retain(|cloth| cloth_ids.insert(cloth.cloth_id));
        self.cloth_updates.reverse();
    }
}

#[derive(Clone, Debug, Serialize)]
struct RewardData {
    r#type: i32,
    id: i32,
    amount: i32,
}

#[derive(Debug, Serialize)]
pub struct CatalogType {
    r#type: i32,
    name: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    r#type: i32,
    id: i32,
    name: String,
    raw_name: String,
    rare: i32,
}

#[derive(Debug, Serialize)]
struct CatalogResponse {
    types: Vec<CatalogType>,
    items: Vec<CatalogEntry>,
}

#[derive(Debug, Serialize)]
struct DungeonCatalog {
    chapters: Vec<DungeonCatalogChapter>,
    episodes: Vec<DungeonCatalogEpisode>,
}

#[derive(Debug, Serialize)]
struct DungeonCatalogChapter {
    id: i32,
    name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DungeonCatalogEpisode {
    id: i32,
    chapter_id: i32,
    name: String,
}

async fn material_catalog(state: &AppState, query: MaterialQuery) -> Result<CatalogResponse> {
    let Some(kind) = query.r#type.and_then(MaterialKind::from_raw) else {
        return Ok(CatalogResponse {
            types: catalog_types(),
            items: Vec::new(),
        });
    };

    let q = query.q.unwrap_or_default().to_ascii_lowercase();
    let limit = query.limit.unwrap_or(200).min(1000);
    let owned_skins = if kind == MaterialKind::Skin && query.unowned_only.unwrap_or(false) {
        match query.player_uid {
            Some(player_id) => Some(
                UserHeroModel::new(player_id, (*state.db).clone())
                    .get_skins()
                    .await?
                    .into_iter()
                    .collect::<HashSet<_>>(),
            ),
            None => None,
        }
    } else {
        None
    };
    let mut materials = entries_for_kind(state.tables, kind, owned_skins.as_ref());

    if !q.is_empty() {
        materials.retain(|entry| {
            entry.id.to_string().contains(&q)
                || entry.name.to_ascii_lowercase().contains(&q)
                || entry.raw_name.to_ascii_lowercase().contains(&q)
        });
    }

    materials.truncate(limit);

    Ok(CatalogResponse {
        types: catalog_types(),
        items: materials,
    })
}

fn catalog_types() -> Vec<CatalogType> {
    [
        MaterialKind::Item,
        MaterialKind::Currency,
        MaterialKind::PlayerExp,
        MaterialKind::Hero,
        MaterialKind::Skin,
        MaterialKind::Equipment,
        MaterialKind::PowerItem,
        MaterialKind::BlockPackage,
        MaterialKind::InsightItem,
    ]
    .into_iter()
    .map(|kind| CatalogType {
        r#type: kind.id(),
        name: kind.label(),
    })
    .collect()
}

fn entries_for_kind(
    tables: &config::GameDB,
    kind: MaterialKind,
    owned_skins: Option<&HashSet<i32>>,
) -> Vec<CatalogEntry> {
    match kind {
        MaterialKind::Item => tables
            .item
            .iter()
            .map(|row| catalog_entry(tables, kind, row.id, &row.name, row.rare))
            .collect(),
        MaterialKind::Currency => tables
            .currency
            .iter()
            .map(|row| catalog_entry(tables, kind, row.id, &row.name, row.rare))
            .collect(),
        MaterialKind::BlockPackage => tables
            .block_package
            .iter()
            .map(|row| catalog_entry(tables, kind, row.id, &row.name, row.rare))
            .collect(),
        MaterialKind::PlayerExp => vec![catalog_entry(tables, kind, 1, "Player EXP", 0)],
        MaterialKind::Hero => tables
            .character
            .iter()
            .map(|row| {
                let raw_name = if row.name_eng.is_empty() {
                    &row.name
                } else {
                    &row.name_eng
                };
                catalog_entry(tables, kind, row.id, raw_name, row.rare)
            })
            .collect(),
        MaterialKind::Skin => tables
            .skin
            .iter()
            .filter(|row| is_premium_hero_skin(tables, row))
            .filter(|row| !owned_skins.is_some_and(|owned| owned.contains(&row.id)))
            .map(|row| {
                let raw_name = if row.name_eng.is_empty() {
                    &row.name
                } else {
                    &row.name_eng
                };
                catalog_entry(tables, kind, row.id, raw_name, row.rare)
            })
            .collect(),
        MaterialKind::Equipment => tables
            .equip
            .iter()
            .map(|row| {
                let raw_name = if row.name_en.is_empty() {
                    &row.name
                } else {
                    &row.name_en
                };
                catalog_entry(tables, kind, row.id, raw_name, row.rare)
            })
            .collect(),
        MaterialKind::PowerItem => tables
            .power_item
            .iter()
            .map(|row| catalog_entry(tables, kind, row.id, &row.name, row.rare))
            .collect(),
        MaterialKind::InsightItem => tables
            .insight_item
            .iter()
            .map(|row| catalog_entry(tables, kind, row.id, &row.name, row.rare))
            .collect(),
    }
}

fn is_premium_hero_skin(tables: &config::GameDB, skin: &config::skin::Skin) -> bool {
    let Some(character) = tables.character.get(skin.character_id) else {
        return false;
    };

    skin.character_id == character.id && !matches!(skin.id % 100, 1 | 2)
}

fn catalog_entry(
    tables: &config::GameDB,
    kind: MaterialKind,
    id: i32,
    raw_name: &str,
    rare: i32,
) -> CatalogEntry {
    let name = resolve_name(tables, raw_name);
    CatalogEntry {
        r#type: kind.id(),
        id,
        name,
        raw_name: raw_name.to_string(),
        rare,
    }
}

fn resolve_name(tables: &config::GameDB, raw_name: &str) -> String {
    let resolved = tables.language_en.get(raw_name).unwrap_or(raw_name);

    clean_name(resolved)
}

fn clean_name(value: &str) -> String {
    let mut cleaned = String::with_capacity(value.len());
    let mut in_tag = false;
    for ch in value.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            '\r' | '\n' if !in_tag => cleaned.push(' '),
            _ if !in_tag => cleaned.push(ch),
            _ => {}
        }
    }
    cleaned.trim().to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
enum MaterialKind {
    Item = 1,
    Currency = 2,
    PlayerExp = 3,
    Hero = 4,
    Skin = 5,
    Equipment = 9,
    PowerItem = 10,
    BlockPackage = 13,
    InsightItem = 24,
}

impl MaterialKind {
    fn parse(value: &str) -> Option<Self> {
        if let Ok(raw) = value.parse::<i32>() {
            return Self::from_raw(raw);
        }

        match value.to_ascii_lowercase().as_str() {
            "item" | "items" => Some(Self::Item),
            "currency" | "currencies" | "coin" => Some(Self::Currency),
            "exp" | "playerexp" => Some(Self::PlayerExp),
            "hero" | "heroes" => Some(Self::Hero),
            "skin" | "skins" => Some(Self::Skin),
            "equip" | "equipment" | "psychube" | "psychubes" => Some(Self::Equipment),
            "power" | "poweritem" => Some(Self::PowerItem),
            "blockpackage" | "package" => Some(Self::BlockPackage),
            "insight" | "insightitem" => Some(Self::InsightItem),
            _ => None,
        }
    }

    fn from_raw(raw: i32) -> Option<Self> {
        match raw {
            raw if raw == Self::Item as i32 => Some(Self::Item),
            raw if raw == Self::Currency as i32 => Some(Self::Currency),
            raw if raw == Self::PlayerExp as i32 => Some(Self::PlayerExp),
            raw if raw == Self::Hero as i32 => Some(Self::Hero),
            raw if raw == Self::Skin as i32 => Some(Self::Skin),
            raw if raw == Self::Equipment as i32 => Some(Self::Equipment),
            raw if raw == Self::PowerItem as i32 => Some(Self::PowerItem),
            raw if raw == Self::BlockPackage as i32 => Some(Self::BlockPackage),
            raw if raw == Self::InsightItem as i32 => Some(Self::InsightItem),
            _ => None,
        }
    }

    fn id(self) -> i32 {
        self as i32
    }

    fn label(self) -> &'static str {
        match self {
            Self::Item => "item",
            Self::Currency => "currency",
            Self::PlayerExp => "playerExp",
            Self::Hero => "hero",
            Self::Skin => "heroSkin",
            Self::Equipment => "equipment",
            Self::PowerItem => "powerItem",
            Self::BlockPackage => "blockPackage",
            Self::InsightItem => "insightItem",
        }
    }
}

fn parse_positive(value: &str, label: &str) -> Result<i32> {
    let parsed = value.parse::<i32>()?;
    if parsed <= 0 {
        anyhow::bail!("invalid {label} '{value}'");
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::{MaterialKind, dungeon_catalog, entries_for_kind, is_premium_hero_skin};
    use std::collections::HashSet;

    #[test]
    fn dungeon_catalog_lists_story_and_resource_episodes() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let data = dungeon_catalog().data.unwrap();
        let chapters = data["chapters"].as_array().unwrap();
        let episodes = data["episodes"].as_array().unwrap();

        assert!(!chapters.is_empty());
        assert!(!chapters.iter().any(|chapter| chapter["id"] == 201));
        assert!(episodes.iter().any(|episode| episode["id"] == 10102));
        assert!(episodes.iter().any(|episode| episode["id"] == 40101));
    }

    #[test]
    fn hero_skin_catalog_excludes_basic_and_insight_skins() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let tables = config::configs::get();
        let first_skin_id = tables
            .skin
            .iter()
            .find(|skin| is_premium_hero_skin(tables, skin))
            .unwrap()
            .id;
        let owned = HashSet::from([first_skin_id]);

        let all = entries_for_kind(tables, MaterialKind::Skin, None);
        let unowned = entries_for_kind(tables, MaterialKind::Skin, Some(&owned));

        assert!(all.iter().all(|entry| {
            tables
                .skin
                .get(entry.id)
                .is_some_and(|skin| is_premium_hero_skin(tables, skin))
        }));
        assert!(all.iter().any(|entry| entry.id == first_skin_id));
        assert!(!unowned.iter().any(|entry| entry.id == first_skin_id));
    }
}
