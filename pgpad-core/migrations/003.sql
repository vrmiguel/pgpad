-- Adds the permissions column to `connections`
-- Defaults to 'read_write' for all existing DBs
ALTER TABLE connections ADD COLUMN permissions TEXT NOT NULL DEFAULT 'read_write';

