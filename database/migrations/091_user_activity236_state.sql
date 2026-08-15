CREATE TABLE IF NOT EXISTS user_activity236_state (
    user_id INTEGER NOT NULL,
    activity_id INTEGER NOT NULL,
    score INTEGER NOT NULL DEFAULT 0,
    gain_reward_ids TEXT NOT NULL DEFAULT '[]',
    updated_at INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, activity_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
