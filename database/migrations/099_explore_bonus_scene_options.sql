-- Preserve ordered choices, including repeated -1 placeholders from the client.
CREATE TABLE user_explore_runtime_bonus_options (
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    chapter_id INTEGER NOT NULL,
    bonus_scene_id INTEGER NOT NULL,
    options_json TEXT NOT NULL,
    PRIMARY KEY (user_id, chapter_id, bonus_scene_id)
);
