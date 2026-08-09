ALTER TABLE user_power_maker_state
    ADD COLUMN updated_at INTEGER NOT NULL DEFAULT 0;

ALTER TABLE user_power_maker_state
    ADD COLUMN last_logout_at INTEGER NOT NULL DEFAULT 0;
