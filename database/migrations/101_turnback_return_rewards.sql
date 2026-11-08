ALTER TABLE user_turnback_state
ADD COLUMN has_get_return_reward_ids TEXT NOT NULL DEFAULT '[]';

ALTER TABLE user_turnback_state
ADD COLUMN return_random_task_id INTEGER NOT NULL DEFAULT 0;

ALTER TABLE user_turnback_state
ADD COLUMN return_left_task_ids TEXT NOT NULL DEFAULT '[]';

ALTER TABLE user_turnback_state
ADD COLUMN return_last_random_task_id INTEGER NOT NULL DEFAULT 0;

ALTER TABLE user_turnback_state
ADD COLUMN return_random_refresh_day INTEGER NOT NULL DEFAULT 0;
