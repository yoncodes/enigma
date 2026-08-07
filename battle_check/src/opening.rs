use std::io;

use anyhow::{Context, Result};
use battle::engine::runtime::BattleRuntime;
use sonettobuf::{CardInfo, FightGroup};

pub(crate) fn print(episode_id: i32) -> Result<()> {
    std::thread::Builder::new()
        .name("battle-check-opening".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?
                .block_on(run(episode_id))
        })?
        .join()
        .map_err(|_| io::Error::other("opening simulation thread panicked"))?
}

async fn run(episode_id: i32) -> Result<()> {
    let db = config::configs::get();
    let episode = db
        .episode
        .get(episode_id)
        .with_context(|| format!("episode {episode_id} is missing"))?;
    let configured = db.teaching_card.get(episode_id);
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await?;
    let built = battle::dungeon::build_fight(
        &pool,
        0,
        episode_id,
        episode.battle_id,
        &FightGroup::default(),
        battle::dungeon::FightOptions::default(),
        None,
    )
    .await?;
    let mut runtime = BattleRuntime::new(built.fight);
    let round = runtime.start_round().map_err(anyhow::Error::msg)?;
    let push = runtime.card_info_push();

    match configured {
        Some(row) => {
            println!("opening.mode=configured");
            println!("opening.config={}", row.opening_cards);
        }
        None => {
            println!("opening.mode=random-sample seed={}", episode.battle_id);
            println!("opening.config=<none>");
        }
    }
    print_cards("opening.raw", &round.team_a_cards1);
    print_cards("opening.deal", &push.deal_card_group);
    print_cards("opening.visible", &push.card_group);
    Ok(())
}

fn print_cards(label: &str, cards: &[CardInfo]) {
    let cards = cards
        .iter()
        .enumerate()
        .map(|(index, card)| {
            format!(
                "{}:{}#{} uid={}",
                index + 1,
                card.hero_id.unwrap_or_default(),
                card.skill_id.unwrap_or_default(),
                card.uid.unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join(" | ");
    println!("{label}={cards}");
}
