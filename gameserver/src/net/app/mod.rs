use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

use crate::net::outbound::CommandPacket;
use crate::player::BattleState;

#[cfg(test)]
pub(crate) fn init_test_server_config() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| common::init_config(common::config::ServerConfig::template()));
}

/// App-level shared state
pub struct AppState {
    next_down_tag: Mutex<u8>,
    pub db: &'static SqlitePool,
    pub tables: &'static config::GameDB,
    sessions: dashmap::DashMap<i64, mpsc::Sender<CommandPacket>>,
    session_locks: dashmap::DashMap<i64, Arc<Mutex<()>>>,
    parked_battles: dashmap::DashMap<i64, BattleState>,
}

#[allow(dead_code)]
impl AppState {
    pub fn new(db: SqlitePool, tables: &'static config::GameDB) -> Self {
        #[cfg(test)]
        init_test_server_config();
        Self {
            next_down_tag: Mutex::new(0),
            db: Box::leak(Box::new(db)),
            tables,
            sessions: dashmap::DashMap::new(),
            session_locks: dashmap::DashMap::new(),
            parked_battles: dashmap::DashMap::new(),
        }
    }

    pub async fn reserve_down_tag(&self) -> u8 {
        let mut tag = self.next_down_tag.lock().await;
        let current = *tag & 0x7F;
        *tag = (*tag + 1) & 0x7F;
        current
    }

    pub fn get_session_sender(&self, player_id: i64) -> Option<mpsc::Sender<CommandPacket>> {
        self.sessions.get(&player_id).map(|v| v.value().clone())
    }

    pub fn register_session(&self, player_id: i64, outbound: mpsc::Sender<CommandPacket>) {
        self.sessions.insert(player_id, outbound);
        self.parked_battles.remove(&player_id);
    }

    pub fn parked_battle(&self, player_id: i64) -> Option<BattleState> {
        self.parked_battles
            .get(&player_id)
            .map(|battle| battle.clone())
    }

    pub fn park_battle(&self, player_id: i64, battle: BattleState) {
        if battle.has_active() {
            self.parked_battles.insert(player_id, battle);
        } else {
            self.parked_battles.remove(&player_id);
        }
    }

    pub async fn lock_session(&self, player_id: i64) -> tokio::sync::OwnedMutexGuard<()> {
        self.session_locks
            .entry(player_id)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
            .lock_owned()
            .await
    }

    pub fn is_current_session(
        &self,
        player_id: i64,
        outbound: &mpsc::Sender<CommandPacket>,
    ) -> bool {
        self.sessions
            .get(&player_id)
            .is_some_and(|current| current.same_channel(outbound))
    }

    pub fn unregister_session(&self, player_id: i64) {
        self.sessions.remove(&player_id);
    }

    pub fn unregister_session_if_current(
        &self,
        player_id: i64,
        outbound: &mpsc::Sender<CommandPacket>,
    ) -> bool {
        match self.sessions.entry(player_id) {
            dashmap::mapref::entry::Entry::Occupied(entry)
                if entry.get().same_channel(outbound) =>
            {
                entry.remove();
                true
            }
            _ => false,
        }
    }

    pub fn online_player_ids(&self) -> Vec<i64> {
        let mut players = self
            .sessions
            .iter()
            .map(|entry| *entry.key())
            .collect::<Vec<_>>();
        players.sort();
        players
    }
}

#[cfg(test)]
mod test;
