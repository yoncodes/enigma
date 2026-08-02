ALTER TABLE dungeon_records ADD COLUMN seed TEXT;

UPDATE user_dungeons
SET has_record = 0
WHERE EXISTS (
    SELECT 1
    FROM dungeon_records
    WHERE dungeon_records.user_id = user_dungeons.user_id
      AND dungeon_records.episode_id = user_dungeons.episode_id
      AND dungeon_records.seed IS NULL
);
