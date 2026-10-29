use super::*;
use sonettobuf::{UnlocHeroStoryReply, UnlocHeroStoryRequest};

#[tokio::test]
async fn hero_story_unlock_routes_cost_push_before_reply() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 919;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'hero-story-unlock-route', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO currencies
         (user_id, currency_id, quantity, last_recover_time, expired_time)
         VALUES (?, 24, 1, 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(4);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));

    let mut data = Vec::new();
    UnlocHeroStoryRequest { story_id: Some(1) }
        .encode(&mut data)
        .unwrap();
    let request = ClientPacket {
        sequence: 1,
        cmd_id: CmdId::UnlocHeroStoryCmd as i16,
        up_tag: 12,
        data,
    }
    .encode();

    dispatch_command(&mut ctx, request).await.unwrap();

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Hero story unlock did not emit a currency snapshot push");
    };
    assert_eq!(cmd_id, CmdId::CurrencyChangePushCmd);
    let currency = CurrencyChangePush::decode(&*body).unwrap();
    assert_eq!(currency.change_currency[0].currency_id, Some(24));
    assert_eq!(currency.change_currency[0].quantity, Some(0));

    let CommandPacket::Reply {
        cmd_id,
        body,
        result_code,
        up_tag,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("Hero story unlock did not emit its reply");
    };
    assert_eq!(cmd_id, CmdId::UnlocHeroStoryCmd);
    assert_eq!(result_code, 0);
    assert_eq!(up_tag, 12);
    let info = UnlocHeroStoryReply::decode(&*body).unwrap().info.unwrap();
    assert_eq!(info.story_id, Some(1));
    assert_eq!(info.unlock, Some(true));
    assert!(packets.try_recv().is_err());
    assert!(
        sqlx::query_scalar::<_, bool>(
            "SELECT unlock FROM user_hero_story_state WHERE user_id = ? AND story_id = 1",
        )
        .bind(player_id)
        .fetch_one(state.db)
        .await
        .unwrap()
    );
}
