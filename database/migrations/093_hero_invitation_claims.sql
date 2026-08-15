CREATE TABLE user_hero_invitation_claims (
    user_id INTEGER NOT NULL,
    invite_id INTEGER NOT NULL CHECK (invite_id >= 0),
    claimed_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, invite_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
