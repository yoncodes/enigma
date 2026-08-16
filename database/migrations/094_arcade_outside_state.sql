CREATE TABLE user_arcade_outside (
    user_id INTEGER NOT NULL,
    activity_id INTEGER NOT NULL,
    character_id INTEGER NOT NULL,
    x INTEGER NOT NULL,
    y INTEGER NOT NULL,
    dir INTEGER NOT NULL DEFAULT 0,
    score INTEGER NOT NULL DEFAULT 0 CHECK (score >= 0),
    hotfix TEXT NOT NULL DEFAULT '[""]',
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, activity_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE user_arcade_talents (
    user_id INTEGER NOT NULL,
    activity_id INTEGER NOT NULL,
    talent_id INTEGER NOT NULL,
    level INTEGER NOT NULL CHECK (level > 0),
    PRIMARY KEY (user_id, activity_id, talent_id),
    FOREIGN KEY (user_id, activity_id)
        REFERENCES user_arcade_outside(user_id, activity_id) ON DELETE CASCADE
);

CREATE TABLE user_arcade_attrs (
    user_id INTEGER NOT NULL,
    activity_id INTEGER NOT NULL,
    attr_id INTEGER NOT NULL,
    base INTEGER NOT NULL DEFAULT 0,
    rate INTEGER NOT NULL DEFAULT 0,
    extra INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, activity_id, attr_id),
    FOREIGN KEY (user_id, activity_id)
        REFERENCES user_arcade_outside(user_id, activity_id) ON DELETE CASCADE
);

CREATE TABLE user_arcade_books (
    user_id INTEGER NOT NULL,
    activity_id INTEGER NOT NULL,
    book_type INTEGER NOT NULL,
    element_id INTEGER NOT NULL,
    is_new INTEGER NOT NULL DEFAULT 0 CHECK (is_new IN (0, 1)),
    PRIMARY KEY (user_id, activity_id, book_type, element_id),
    FOREIGN KEY (user_id, activity_id)
        REFERENCES user_arcade_outside(user_id, activity_id) ON DELETE CASCADE
);

CREATE TABLE user_arcade_unlock_roles (
    user_id INTEGER NOT NULL,
    activity_id INTEGER NOT NULL,
    character_id INTEGER NOT NULL,
    PRIMARY KEY (user_id, activity_id, character_id),
    FOREIGN KEY (user_id, activity_id)
        REFERENCES user_arcade_outside(user_id, activity_id) ON DELETE CASCADE
);

CREATE TABLE user_arcade_unlock_difficulties (
    user_id INTEGER NOT NULL,
    activity_id INTEGER NOT NULL,
    difficulty_id INTEGER NOT NULL,
    PRIMARY KEY (user_id, activity_id, difficulty_id),
    FOREIGN KEY (user_id, activity_id)
        REFERENCES user_arcade_outside(user_id, activity_id) ON DELETE CASCADE
);

CREATE TABLE user_arcade_reward_claims (
    user_id INTEGER NOT NULL,
    activity_id INTEGER NOT NULL,
    reward_id INTEGER NOT NULL,
    claimed_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, activity_id, reward_id),
    FOREIGN KEY (user_id, activity_id)
        REFERENCES user_arcade_outside(user_id, activity_id) ON DELETE CASCADE
);
