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
