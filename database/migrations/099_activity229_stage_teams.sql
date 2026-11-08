ALTER TABLE user_activity229_stages
ADD COLUMN hero_group_sub_id INTEGER NOT NULL DEFAULT 0;

ALTER TABLE user_activity229_stages
ADD COLUMN last_update_team_time INTEGER NOT NULL DEFAULT 0;
