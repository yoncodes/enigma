CREATE TABLE user_arcade_inside_saves (
    user_id INTEGER NOT NULL,
    activity_id INTEGER NOT NULL,
    difficulty INTEGER NOT NULL CHECK (difficulty >= 0),
    snapshot BLOB NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, activity_id),
    FOREIGN KEY (user_id, activity_id)
        REFERENCES user_arcade_outside(user_id, activity_id) ON DELETE CASCADE
);

CREATE TABLE user_arcade_completions (
    user_id INTEGER NOT NULL,
    activity_id INTEGER NOT NULL,
    difficulty INTEGER NOT NULL CHECK (difficulty >= 0),
    finish_count INTEGER NOT NULL CHECK (finish_count > 0),
    PRIMARY KEY (user_id, activity_id, difficulty),
    FOREIGN KEY (user_id, activity_id)
        REFERENCES user_arcade_outside(user_id, activity_id) ON DELETE CASCADE
);
