-- Add migration script here
ALTER TABLE todos ADD COLUMN completed_at TEXT;
UPDATE todos SET completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE completed = 1;

ALTER TABLE todos DROP COLUMN completed;