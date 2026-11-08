CREATE TABLE user_activity243_episodes (
    user_id INTEGER NOT NULL,
    activity_id INTEGER NOT NULL,
    episode_id INTEGER NOT NULL,
    best_score INTEGER NOT NULL DEFAULT 0,
    best_star INTEGER NOT NULL DEFAULT 0,
    best_time_ms INTEGER NOT NULL DEFAULT 0,
    claimed_stars TEXT NOT NULL DEFAULT '[]',
    record TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, activity_id, episode_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE user_activity243_gifts (
    user_id INTEGER NOT NULL,
    activity_id INTEGER NOT NULL,
    gift_point INTEGER NOT NULL,
    level INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, activity_id, gift_point),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE user_activity243_racers (
    user_id INTEGER NOT NULL,
    activity_id INTEGER NOT NULL,
    racer_id INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, activity_id, racer_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
