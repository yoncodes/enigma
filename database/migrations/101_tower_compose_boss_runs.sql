CREATE TABLE user_tower_compose_boss_planes (
    user_id INTEGER NOT NULL,
    theme_id INTEGER NOT NULL,
    layer_id INTEGER NOT NULL,
    plane_id INTEGER NOT NULL CHECK (plane_id IN (1, 2)),
    plane_json TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, theme_id, layer_id, plane_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE user_tower_compose_boss_records (
    user_id INTEGER NOT NULL,
    theme_id INTEGER NOT NULL,
    layer_id INTEGER NOT NULL,
    current_record_json TEXT NOT NULL DEFAULT '',
    saved_record_json TEXT NOT NULL DEFAULT '',
    saved_layer_id INTEGER NOT NULL DEFAULT 0,
    resume_layer_id INTEGER NOT NULL DEFAULT 0,
    resume_boss_json TEXT NOT NULL DEFAULT '',
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, theme_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
