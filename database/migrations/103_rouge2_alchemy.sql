ALTER TABLE user_rouge2_state ADD COLUMN alchemy_formula INTEGER NOT NULL DEFAULT 0;
ALTER TABLE user_rouge2_state ADD COLUMN alchemy_main_effect INTEGER NOT NULL DEFAULT 0;
ALTER TABLE user_rouge2_state ADD COLUMN alchemy_sub_effects TEXT NOT NULL DEFAULT '[]';

ALTER TABLE user_rouge2_materials ADD COLUMN is_seen INTEGER NOT NULL DEFAULT 0;
UPDATE user_rouge2_materials SET is_seen = 1 WHERE num <> 0;
