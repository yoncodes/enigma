CREATE TABLE user_explore_map_state (
    user_id INTEGER NOT NULL,
    map_id INTEGER NOT NULL,
    snapshot TEXT NOT NULL,
    PRIMARY KEY (user_id, map_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
