CREATE TABLE user_milestones (
    user_id INTEGER NOT NULL,
    milestone_id INTEGER NOT NULL,
    get_bonus_id INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, milestone_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
