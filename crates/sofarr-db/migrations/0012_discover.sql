-- Discover (spec discover AC-08): whether seasons that appear later are monitored.
-- Existing series keep today's behavior (new seasons monitored).

ALTER TABLE series ADD COLUMN monitor_new_seasons INTEGER NOT NULL DEFAULT 1;
