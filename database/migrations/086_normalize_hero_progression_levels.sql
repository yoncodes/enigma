UPDATE heroes
SET level = CASE rank
    WHEN 2 THEN 31
    WHEN 3 THEN 71
    WHEN 4 THEN 121
END
WHERE (rank = 2 AND level < 31)
   OR (rank = 3 AND level < 71)
   OR (rank = 4 AND level < 121);
