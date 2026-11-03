-- Existing stone identifiers encode their source map. Other legacy items have
-- no reliable origin, so retain them on the currently selected map.
ALTER TABLE user_explore_runtime_items ADD COLUMN map_id INTEGER;
UPDATE user_explore_runtime_items AS i SET map_id = COALESCE(
    CASE WHEN i.item_id = 150001 AND i.uid >= 1000000000
        AND EXISTS (SELECT 1 FROM user_explore_runtime_maps m
            WHERE m.user_id = i.user_id AND m.map_id = (i.uid % 1000000) / 1000
              AND json_extract(m.state_json, '$.objects."' || (i.uid % 1000) || '".kind') = 217)
        THEN (i.uid % 1000000) / 1000 END,
    (SELECT current_map_id FROM user_explore_runtime_profiles p WHERE p.user_id = i.user_id)
);
CREATE INDEX idx_explore_items_map ON user_explore_runtime_items(user_id, map_id);
CREATE TRIGGER explore_item_require_map BEFORE INSERT ON user_explore_runtime_items
WHEN NEW.map_id IS NULL AND (SELECT current_map_id FROM user_explore_runtime_profiles WHERE user_id = NEW.user_id) IS NULL
BEGIN
    SELECT RAISE(ABORT, 'exploration item requires a current map');
END;
CREATE TRIGGER explore_item_assign_map AFTER INSERT ON user_explore_runtime_items
WHEN NEW.map_id IS NULL
BEGIN
    UPDATE user_explore_runtime_items SET map_id =
        (SELECT current_map_id FROM user_explore_runtime_profiles WHERE user_id = NEW.user_id)
        WHERE uid = NEW.uid;
END;
UPDATE user_explore_runtime_profiles SET use_item_uid = 0
WHERE use_item_uid != 0 AND EXISTS (
    SELECT 1 FROM user_explore_runtime_items i WHERE i.user_id = user_explore_runtime_profiles.user_id
    AND i.uid = user_explore_runtime_profiles.use_item_uid AND i.map_id IS NOT user_explore_runtime_profiles.current_map_id
);

-- Keep the acquisition hint once per stone table, including old saves where
-- the table currently records a previously acquired stone.
CREATE TABLE user_explore_stone_hints (
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    map_id INTEGER NOT NULL,
    unit_id INTEGER NOT NULL,
    PRIMARY KEY(user_id, map_id, unit_id)
);
INSERT OR IGNORE INTO user_explore_stone_hints(user_id, map_id, unit_id)
SELECT m.user_id, m.map_id, CAST(o.key AS INTEGER)
FROM user_explore_runtime_maps m, json_each(m.state_json, '$.objects') o
WHERE json_extract(o.value, '$.kind') = 217
  AND (json_extract(o.value, '$.status') & 4) != 0;

-- Older servers left claimed bag-item reward objects visible. Preserve the
-- claim bit and all other map state while hiding those objects on reload.
UPDATE user_explore_runtime_maps AS m
SET state_json = json_patch(m.state_json, json_object('objects', json((
    SELECT json_group_object(o.key, json_object('status', json_extract(o.value, '$.status') & ~1))
    FROM json_each(m.state_json, '$.objects') o
    WHERE json_extract(o.value, '$.kind') = 232
      AND (json_extract(o.value, '$.status') & 5) = 5
))))
WHERE EXISTS (
    SELECT 1 FROM json_each(m.state_json, '$.objects') o
    WHERE json_extract(o.value, '$.kind') = 232
      AND (json_extract(o.value, '$.status') & 5) = 5
);
