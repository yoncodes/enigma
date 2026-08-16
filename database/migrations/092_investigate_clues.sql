CREATE TABLE user_investigate_clues (
    user_id INTEGER NOT NULL,
    info_id INTEGER NOT NULL,
    clue_id INTEGER NOT NULL,
    PRIMARY KEY (user_id, info_id, clue_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
