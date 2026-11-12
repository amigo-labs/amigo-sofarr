-- Library pages (spec web-ui-library): the poster path the metadata provider offers
-- (TMDB file path for movies, TVDB artwork path for series) and the overview text.

ALTER TABLE movies ADD COLUMN poster_path TEXT;
ALTER TABLE movies ADD COLUMN overview TEXT;
ALTER TABLE series ADD COLUMN poster_path TEXT;
ALTER TABLE series ADD COLUMN overview TEXT;

-- Activity and import problems are read newest first.
CREATE INDEX grabs_updated ON grabs (updated_at);
CREATE INDEX series_grabs_updated ON series_grabs (updated_at);
