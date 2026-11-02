-- Additive storage for detailed exploration state. Existing summary tables are untouched.
CREATE TABLE user_explore_runtime_profiles (
    user_id INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    current_map_id INTEGER,
    use_item_uid INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE user_explore_runtime_items (
    uid INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES user_explore_runtime_profiles(user_id) ON DELETE CASCADE,
    item_id INTEGER NOT NULL,
    quantity INTEGER NOT NULL CHECK (quantity > 0),
    status INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_user_explore_runtime_items ON user_explore_runtime_items(user_id);

CREATE TABLE user_explore_runtime_maps (
    user_id INTEGER NOT NULL REFERENCES user_explore_runtime_profiles(user_id) ON DELETE CASCADE,
    map_id INTEGER NOT NULL,
    revision INTEGER NOT NULL DEFAULT 0,
    state_json TEXT NOT NULL,
    PRIMARY KEY (user_id, map_id)
);
