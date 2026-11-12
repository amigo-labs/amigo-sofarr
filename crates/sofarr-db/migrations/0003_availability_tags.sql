-- Availability (minimumAvailability and release dates) and tags of movies and series.

ALTER TABLE movies ADD COLUMN minimum_availability TEXT NOT NULL DEFAULT 'released';
ALTER TABLE movies ADD COLUMN status TEXT;
ALTER TABLE movies ADD COLUMN in_cinemas TEXT;
ALTER TABLE movies ADD COLUMN digital_release TEXT;
ALTER TABLE movies ADD COLUMN physical_release TEXT;
-- Comma-separated tag ids (ids of the `tag` setting).
ALTER TABLE movies ADD COLUMN tags TEXT NOT NULL DEFAULT '';

-- Comma-separated tag ids (ids of the `sonarr_tag` setting).
ALTER TABLE series ADD COLUMN tags TEXT NOT NULL DEFAULT '';
