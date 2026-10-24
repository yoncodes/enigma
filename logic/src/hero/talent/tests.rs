use super::*;

#[tokio::test]
async fn template_commands_mutate_the_owned_template() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (24, 'talent', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let hero = UserHeroModel::new(24, pool.clone());
    hero.create_hero(3003).await.unwrap();
    hero.replace_talent_cubes(3003, 1, vec![(1, 0, 0, 0)])
        .await
        .unwrap();

    let manager = HeroManager::new(24);
    let renamed = manager
        .rename_talent_template(&pool, 3003, 1, "  Alpha  ".into())
        .await
        .unwrap();
    assert_eq!(
        renamed
            .template_info
            .as_ref()
            .and_then(|template| template.name.as_deref()),
        Some("Alpha")
    );
    assert!(
        manager
            .rename_talent_template(&pool, 3003, 1, "12345678901".into())
            .await
            .is_err()
    );

    let (reply, hero_info) = manager
        .takeoff_all_talent_cubes(&pool, 3003, 1)
        .await
        .unwrap();
    assert!(reply.template_info.unwrap().talent_cube_infos.is_empty());
    assert!(hero_info.talent_cube_infos.is_empty());
}

#[tokio::test]
async fn talent_up_requires_rank_and_consumes_configured_costs() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (25, 'talent-up', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let hero = UserHeroModel::new(25, pool.clone());
    hero.create_hero(3003).await.unwrap();
    let talent = config::configs::get().character_talent(3003, 2).unwrap();
    let costs = crate::reward::parse(&talent.consume);
    crate::reward::RewardManager::new(25)
        .apply(&pool, costs.clone())
        .await
        .unwrap();

    assert!(HeroManager::new(25).talent_up(&pool, 3003).await.is_err());
    assert_eq!(hero.get(3003).await.unwrap().record.talent, 1);
    for (item_id, amount) in &costs.items {
        let quantity: i32 =
            sqlx::query_scalar("SELECT quantity FROM items WHERE user_id = 25 AND item_id = ?")
                .bind(item_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(quantity, *amount);
    }

    hero.set_rank_and_level(3003, talent.requirement, 1)
        .await
        .unwrap();
    let (_, updated, consumed) = HeroManager::new(25).talent_up(&pool, 3003).await.unwrap();

    assert_eq!(updated.talent, Some(2));
    assert_eq!(consumed.item_ids, vec![120011]);
    for (item_id, _) in &costs.items {
        let quantity: i32 =
            sqlx::query_scalar("SELECT quantity FROM items WHERE user_id = 25 AND item_id = ?")
                .bind(item_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(quantity, 0);
    }
}

#[tokio::test]
async fn talent_style_unlock_consumes_configured_costs_once() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (26, 'talent-style', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let hero = UserHeroModel::new(26, pool.clone());
    hero.create_hero(3003).await.unwrap();
    let style = config::configs::get().talent_style_cost(3003, 1).unwrap();
    let costs = crate::reward::parse(&style.consume);

    assert!(
        HeroManager::new(26)
            .unlock_talent_style(&pool, 3003, 1)
            .await
            .is_err()
    );
    assert!(!hero.has_talent_style(3003, 1).await.unwrap());

    crate::reward::RewardManager::new(26)
        .apply(&pool, costs.clone())
        .await
        .unwrap();
    let (_, updated, consumed) = HeroManager::new(26)
        .unlock_talent_style(&pool, 3003, 1)
        .await
        .unwrap();

    assert!(hero.has_talent_style(3003, 1).await.unwrap());
    assert_eq!(updated.talent_style_unlock, Some(1 << 1),);
    assert_eq!(
        consumed.currency_ids,
        costs
            .currencies
            .iter()
            .map(|(id, amount)| (*id, -*amount))
            .collect::<Vec<_>>()
    );
    for (item_id, _) in &costs.items {
        let quantity: i32 =
            sqlx::query_scalar("SELECT quantity FROM items WHERE user_id = 26 AND item_id = ?")
                .bind(item_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(quantity, 0);
    }
    for (currency_id, _) in &costs.currencies {
        let quantity: i32 = sqlx::query_scalar(
            "SELECT quantity FROM currencies WHERE user_id = 26 AND currency_id = ?",
        )
        .bind(currency_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(quantity, 0);
    }
    let (_, _, repeated) = HeroManager::new(26)
        .unlock_talent_style(&pool, 3003, 1)
        .await
        .unwrap();
    assert!(repeated.item_ids.is_empty());
    assert!(repeated.currency_ids.is_empty());
}
