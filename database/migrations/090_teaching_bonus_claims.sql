CREATE TABLE user_teaching_bonus_claims (
    user_id INTEGER NOT NULL,
    teaching_id INTEGER NOT NULL,
    PRIMARY KEY (user_id, teaching_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
