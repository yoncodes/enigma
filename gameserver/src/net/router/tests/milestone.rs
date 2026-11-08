use super::*;
use sonettobuf::{GetMilestoneBonusReply, GetMilestoneBonusRequest};

#[tokio::test]
async fn racing_claim_emits_the_captured_reward_sequence() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let milestone = configs::get()
        .milestone
        .iter()
        .find(|row| row.r#type == 2)
        .unwrap();
    let episode_id = configs::get()
        .activity243_episode
        .iter()
        .find(|episode| episode.activity_id == milestone.activity_id)
        .unwrap()
        .episode_id;
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 516;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'milestone-route', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_activity243_episodes
         (user_id, activity_id, episode_id, best_score, best_star, best_time_ms,
          claimed_stars, record, created_at, updated_at)
         VALUES (?, ?, ?, 0, 3, 0, '[]', '', 0, 0)",
    )
    .bind(player_id)
    .bind(milestone.activity_id)
    .bind(episode_id)
    .execute(&pool)
    .await
    .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(8);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));
    let mut data = Vec::new();
    GetMilestoneBonusRequest {
        milestone_id: Some(milestone.milestone_id),
    }
    .encode(&mut data)
    .unwrap();
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 1,
            cmd_id: CmdId::GetMilestoneBonusCmd as i16,
            up_tag: 12,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("milestone claim did not emit item snapshots first");
    };
    assert_eq!(cmd_id, CmdId::ItemChangePushCmd);
    let mut items = ItemChangePush::decode(&*body)
        .unwrap()
        .items
        .into_iter()
        .map(|item| (item.item_id.unwrap(), item.quantity.unwrap()))
        .collect::<Vec<_>>();
    items.sort_unstable();
    assert_eq!(items, [(110304, 1), (110903, 3)]);

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("milestone claim did not emit the trade red dot");
    };
    assert_eq!(cmd_id, CmdId::UpdateRedDotPushCmd);
    let trade = UpdateRedDotPush::decode(&*body).unwrap();
    assert_eq!(trade.red_dot_infos[0].define_id, 2240);

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("milestone claim did not emit material deltas");
    };
    assert_eq!(cmd_id, CmdId::MaterialChangePushCmd);
    let material = MaterialChangePush::decode(&*body).unwrap();
    assert_eq!(material.get_approach, Some(172));

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("milestone claim did not clear milestone red dots");
    };
    assert_eq!(cmd_id, CmdId::UpdateRedDotPushCmd);
    let red_dots = UpdateRedDotPush::decode(&*body).unwrap();
    assert_eq!(
        red_dots
            .red_dot_infos
            .iter()
            .map(|group| group.define_id)
            .collect::<Vec<_>>(),
        [3922, 3924, 38511]
    );

    let CommandPacket::Reply {
        cmd_id,
        body,
        result_code,
        up_tag,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("milestone claim did not reply last");
    };
    assert_eq!(cmd_id, CmdId::GetMilestoneBonusCmd);
    assert_eq!(result_code, 0);
    assert_eq!(up_tag, 12);
    let reply = GetMilestoneBonusReply::decode(&*body).unwrap();
    assert_eq!(reply.milestone_id, Some(milestone.milestone_id));
    assert_eq!(reply.get_bonus_id, Some(2));
    assert!(packets.try_recv().is_err());
}

#[tokio::test]
async fn final_racing_claim_emits_the_captured_currency_sequence() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let milestone = configs::get()
        .milestone
        .iter()
        .find(|row| row.r#type == 2)
        .unwrap();
    let episode_ids = configs::get()
        .activity243_episode
        .iter()
        .filter(|episode| episode.activity_id == milestone.activity_id)
        .map(|episode| episode.episode_id)
        .take(7)
        .collect::<Vec<_>>();
    assert_eq!(episode_ids.len(), 7);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 517;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'milestone-final-route', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    for (index, episode_id) in episode_ids.into_iter().enumerate() {
        let stars = if index == 6 { 2 } else { 3 };
        sqlx::query(
            "INSERT INTO user_activity243_episodes
             (user_id, activity_id, episode_id, best_score, best_star, best_time_ms,
              claimed_stars, record, created_at, updated_at)
             VALUES (?, ?, ?, 0, ?, 0, '[]', '', 0, 0)",
        )
        .bind(player_id)
        .bind(milestone.activity_id)
        .bind(episode_id)
        .bind(stars)
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO user_milestones (user_id, milestone_id, get_bonus_id)
         VALUES (?, ?, 9)",
    )
    .bind(player_id)
    .bind(milestone.milestone_id)
    .execute(&pool)
    .await
    .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(8);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));
    let mut data = Vec::new();
    GetMilestoneBonusRequest {
        milestone_id: Some(milestone.milestone_id),
    }
    .encode(&mut data)
    .unwrap();
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 1,
            cmd_id: CmdId::GetMilestoneBonusCmd as i16,
            up_tag: 13,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("final milestone claim did not emit currencies first");
    };
    assert_eq!(cmd_id, CmdId::CurrencyChangePushCmd);
    let mut currencies = CurrencyChangePush::decode(&*body)
        .unwrap()
        .change_currency
        .into_iter()
        .map(|currency| (currency.currency_id.unwrap(), currency.quantity.unwrap()))
        .collect::<Vec<_>>();
    currencies.sort_unstable();
    assert_eq!(currencies, [(3, 20_000), (5, 20_000)]);

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("final milestone claim did not emit material deltas");
    };
    assert_eq!(cmd_id, CmdId::MaterialChangePushCmd);
    let material = MaterialChangePush::decode(&*body).unwrap();
    assert_eq!(material.get_approach, Some(172));
    let mut deltas = material
        .data_list
        .into_iter()
        .map(|entry| {
            (
                entry.materil_type.unwrap(),
                entry.materil_id.unwrap(),
                entry.quantity.unwrap(),
            )
        })
        .collect::<Vec<_>>();
    deltas.sort_unstable();
    assert_eq!(deltas, [(2, 3, 20_000), (2, 5, 20_000)]);

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("final milestone claim did not clear milestone red dots");
    };
    assert_eq!(cmd_id, CmdId::UpdateRedDotPushCmd);
    assert_eq!(
        UpdateRedDotPush::decode(&*body)
            .unwrap()
            .red_dot_infos
            .iter()
            .map(|group| group.define_id)
            .collect::<Vec<_>>(),
        [3922, 3924, 38511]
    );

    let CommandPacket::Reply {
        cmd_id,
        body,
        result_code,
        up_tag,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("final milestone claim did not reply last");
    };
    assert_eq!(cmd_id, CmdId::GetMilestoneBonusCmd);
    assert_eq!(result_code, 0);
    assert_eq!(up_tag, 13);
    let reply = GetMilestoneBonusReply::decode(&*body).unwrap();
    assert_eq!(reply.milestone_id, Some(milestone.milestone_id));
    assert_eq!(reply.get_bonus_id, Some(11));
    assert!(packets.try_recv().is_err());
}
