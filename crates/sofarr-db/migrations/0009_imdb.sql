-- IMDb ratings (spec imdb-ratings): replaced as a whole at every load through
-- imdb_ratings_next; series get their TMDB id (D26).

CREATE TABLE imdb_ratings (
    imdb_id TEXT PRIMARY KEY,
    rating  REAL NOT NULL,
    votes   INTEGER NOT NULL
) WITHOUT ROWID;

ALTER TABLE series ADD COLUMN tmdb_id INTEGER;
