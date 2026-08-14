CREATE TABLE user_act233_bp_state (
    user_id INTEGER NOT NULL,
    activity_id INTEGER NOT NULL,
    bp_id INTEGER NOT NULL,
    score INTEGER NOT NULL DEFAULT 0,
    pay_status INTEGER NOT NULL DEFAULT 0,
    has_get_free_bonus TEXT NOT NULL DEFAULT '[]',
    has_get_pay_bonus TEXT NOT NULL DEFAULT '[]',
    updated_at INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, activity_id, bp_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX idx_user_act233_bp_state_user
    ON user_act233_bp_state(user_id);
