CREATE TABLE IF NOT EXISTS user_rouge2_attrs (
    user_id INTEGER NOT NULL,
    attr_id INTEGER NOT NULL,
    final_val INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, attr_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
