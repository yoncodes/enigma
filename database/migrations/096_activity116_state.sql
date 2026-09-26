CREATE TABLE user_activity116_state (
    user_id INTEGER NOT NULL,
    activity_id INTEGER NOT NULL,
    put_trap INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, activity_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE user_activity116_elements (
    user_id INTEGER NOT NULL,
    activity_id INTEGER NOT NULL,
    element_id INTEGER NOT NULL,
    level INTEGER NOT NULL DEFAULT 0 CHECK (level >= 0),
    PRIMARY KEY (user_id, activity_id, element_id),
    FOREIGN KEY (user_id, activity_id)
        REFERENCES user_activity116_state(user_id, activity_id) ON DELETE CASCADE
);

CREATE TABLE user_activity116_traps (
    user_id INTEGER NOT NULL,
    activity_id INTEGER NOT NULL,
    trap_id INTEGER NOT NULL,
    PRIMARY KEY (user_id, activity_id, trap_id),
    FOREIGN KEY (user_id, activity_id)
        REFERENCES user_activity116_state(user_id, activity_id) ON DELETE CASCADE
);
