-- Link an exploration encounter to its active fight without modifying prior migrations.
CREATE TABLE user_explore_pending_battles (
    user_id INTEGER PRIMARY KEY REFERENCES user_explore_runtime_profiles(user_id) ON DELETE CASCADE,
    map_id INTEGER NOT NULL,
    unit_id INTEGER NOT NULL,
    battle_id INTEGER NOT NULL CHECK (battle_id > 0),
    fight_id INTEGER UNIQUE REFERENCES fight_instances(id),
    FOREIGN KEY (user_id, map_id) REFERENCES user_explore_runtime_maps(user_id, map_id) ON DELETE CASCADE
);
