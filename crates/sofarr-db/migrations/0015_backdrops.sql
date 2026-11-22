-- Cinema look (spec ui-cinema-backdrops): the TMDB file path of the wide image of a movie
-- or series, next to the poster path.

ALTER TABLE movies ADD COLUMN backdrop_path TEXT;
ALTER TABLE series ADD COLUMN backdrop_path TEXT;
