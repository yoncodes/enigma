use super::*;
use database::db::game::activity_state::{self, ActivityStateKind, ActivityStateSet};

async fn fixture() -> SqlitePool {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let db = SqlitePool::connect("sqlite::memory:").await.unwrap();
    super::super::tests::seed_rank_player(&db, 80, 700, 0).await;
    activity_state::set(
        &db,
        80,
        13809,
        ActivityStateSet {
            kind: ActivityStateKind::Act128BossScore,
            entry_id: 1,
            state: 8_427_039,
            progress: 8_427_039,
            ext: "",
        },
    )
    .await
    .unwrap();
    db
}

#[tokio::test]
async fn gallery_claim_persists_the_paid_threshold_and_does_not_pay_twice() {
    let db = fixture().await;
    let before = get_gallery_infos(&db, 80, Some(13809)).await.unwrap();
    let boss = before
        .gallery_detail
        .iter()
        .find(|boss| boss.r#type == Some(109))
        .unwrap();
    assert_eq!(
        (boss.highest_point, boss.accept_exp_point),
        (Some(8_427_039), Some(0))
    );
    let first = get_act128_exp(&db, 80, Some(13809), Some(109))
        .await
        .unwrap();
    assert_eq!(first.reply.accept_exp_point, Some(7_500_000));
    assert_eq!(
        (first.reply.player_exp, first.reply.player_level),
        (Some(1150), Some(11))
    );
    assert_eq!(first.currency_changes, vec![(3206, 450)]);
    let reloaded = get_gallery_infos(&db, 80, Some(13809)).await.unwrap();
    let boss = reloaded
        .gallery_detail
        .iter()
        .find(|boss| boss.r#type == Some(109))
        .unwrap();
    assert_eq!(
        (boss.highest_point, boss.accept_exp_point),
        (Some(8_427_039), Some(7_500_000))
    );
    let repeated = get_act128_exp(&db, 80, Some(13809), Some(109))
        .await
        .unwrap();
    assert_eq!(repeated.reply, first.reply);
    assert!(repeated.currency_changes.is_empty());
    assert!(
        get_act128_exp(&db, 80, Some(13809), Some(101))
            .await
            .is_err()
    );
    let untouched = get_gallery_infos(&db, 80, Some(13809)).await.unwrap();
    assert_eq!(
        untouched
            .gallery_detail
            .iter()
            .find(|boss| boss.r#type == Some(101))
            .unwrap()
            .have_fight,
        Some(false)
    );
}

#[tokio::test]
async fn failed_gallery_credit_rolls_back_the_claim_cursor() {
    let db = fixture().await;
    sqlx::query(
        "CREATE TRIGGER reject_gallery_credit BEFORE UPDATE OF quantity ON currencies
         WHEN OLD.currency_id = 3206 BEGIN SELECT RAISE(ABORT, 'reject credit'); END",
    )
    .execute(&db)
    .await
    .unwrap();
    assert!(
        get_act128_exp(&db, 80, Some(13809), Some(109))
            .await
            .is_err()
    );
    let reloaded = get_gallery_infos(&db, 80, Some(13809)).await.unwrap();
    assert_eq!(
        reloaded
            .gallery_detail
            .iter()
            .find(|boss| boss.r#type == Some(109))
            .unwrap()
            .accept_exp_point,
        Some(0)
    );
    assert_eq!(
        currencies::get_currency(&db, 80, 3206)
            .await
            .unwrap()
            .unwrap()
            .quantity,
        700
    );
}

#[tokio::test]
async fn concurrent_gallery_claims_pay_each_threshold_once() {
    let db = fixture().await;
    let (first, second) = tokio::join!(
        get_act128_exp(&db, 80, Some(13809), Some(109)),
        get_act128_exp(&db, 80, Some(13809), Some(109)),
    );
    let first = first.unwrap();
    let second = second.unwrap();
    let paid = first
        .currency_changes
        .into_iter()
        .chain(second.currency_changes)
        .map(|(_, amount)| amount)
        .sum::<i32>();
    assert_eq!(paid, 450);
    assert_eq!(
        currencies::get_currency(&db, 80, 3206)
            .await
            .unwrap()
            .unwrap()
            .quantity,
        1150
    );
}
