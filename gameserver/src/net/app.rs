use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

use crate::net::outbound::CommandPacket;

/// App-level shared state
pub struct AppState {
    next_down_tag: Mutex<u8>,
    pub db: &'static SqlitePool,
    pub tables: &'static config::GameDB,
    sessions: dashmap::DashMap<i64, mpsc::Sender<CommandPacket>>,
    session_locks: dashmap::DashMap<i64, Arc<Mutex<()>>>,
}

#[allow(dead_code)]
impl AppState {
    pub fn new(db: SqlitePool, tables: &'static config::GameDB) -> Self {
        Self {
            next_down_tag: Mutex::new(0),
            db: Box::leak(Box::new(db)),
            tables,
            sessions: dashmap::DashMap::new(),
            session_locks: dashmap::DashMap::new(),
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
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn stale_session_cannot_unregister_replacement() {
        let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("data/excel2json");
        let _ = config::init(data_dir.to_str().unwrap());
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let state = Arc::new(AppState::new(pool, config::configs::get()));
        let (first, _) = mpsc::channel(1);
        let (replacement, _) = mpsc::channel(1);

        state.register_session(7, first.clone());
        let teardown = state.lock_session(7).await;
        let replacement_state = Arc::clone(&state);
        let replacement_sender = replacement.clone();
        let replacement_task = tokio::spawn(async move {
            let _registration = replacement_state.lock_session(7).await;
            replacement_state.register_session(7, replacement_sender);
        });

        tokio::task::yield_now().await;
        assert!(state.is_current_session(7, &first));
        assert!(!replacement_task.is_finished());
        assert!(state.unregister_session_if_current(7, &first));
        drop(teardown);
        replacement_task.await.unwrap();

        assert!(!state.unregister_session_if_current(7, &first));
        assert!(
            state
                .get_session_sender(7)
                .unwrap()
                .same_channel(&replacement)
        );
        assert!(state.unregister_session_if_current(7, &replacement));
        assert!(state.get_session_sender(7).is_none());
    }
}
