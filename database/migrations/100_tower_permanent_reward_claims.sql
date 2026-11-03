-- Track first-clear rewards separately from the highest cleared layer.
-- Existing users are initialized transactionally by the settlement code.
CREATE TABLE user_tower_permanent_reward_init (
    user_id INTEGER PRIMARY KEY,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE user_tower_permanent_reward_claims (
    user_id INTEGER NOT NULL,
    layer_id INTEGER NOT NULL,
    PRIMARY KEY (user_id, layer_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
