mod dungeon;
mod error;
mod gm;
mod handlers;
mod net;
mod player;
mod session;
mod tower;
mod tower_compose;
mod util;

pub use logic;
pub use logic::types;

use common::{excel_data_directory, game_port, host, init_config, init_tracing, load_config};
use database::{DatabaseSettings, migrate_or_rescue};
use tokio::net::TcpListener;
use tracing::info;

use crate::net::{app::AppState, session::handle_client};

// Battle resolution can nest many rule frames. Keep the worker stack consistent
// with the isolated battle replay and regression-test workers.
const BATTLE_WORKER_STACK_SIZE: usize = 32 * 1024 * 1024;

fn main() -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(BATTLE_WORKER_STACK_SIZE)
        .build()?
        .block_on(run())
}

async fn run() -> anyhow::Result<()> {
    init_tracing();

    let cfg = load_config()?;
    init_config(cfg.clone());

    let db = migrate_or_rescue(&DatabaseSettings {
        db_name: cfg.database.path.to_string_lossy().to_string(),
    })
    .await?;

    info!(
        "Loading game data from {}",
        excel_data_directory().display()
    );
    config::configs::init(excel_data_directory().to_str().unwrap())?;
    let tables = config::configs::get();
    info!("Game data loaded");

    let state: &'static AppState = Box::leak(Box::new(AppState::new(db, tables)));
    if common::muip_gm_enabled() {
        let admin_addr = common::muip_gm_listen_addr();
        tokio::spawn(async move {
            if let Err(error) = gm::run_gm_listener(admin_addr, state).await {
                tracing::error!("MUIP GM listener failed: {}", error);
            }
        });
    }

    let addr = format!("{}:{}", host(), game_port());
    let listener = TcpListener::bind(&addr).await?;
    info!("Game server is listening on tcp://{addr}");

    loop {
        let (socket, client) = listener.accept().await?;
        tracing::info!("New client connected: {client}");

        tokio::spawn(async move {
            if let Err(e) = handle_client(socket, state).await {
                tracing::error!("Client handler error: {e}");
            }
        });
    }
}
