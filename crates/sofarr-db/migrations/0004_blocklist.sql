-- Failure handling: blocklist per movie or series, and when an active grab's download
-- was first missing from the download client.

CREATE TABLE blocklist (
    id               INTEGER PRIMARY KEY,
    movie_id         INTEGER REFERENCES movies (id) ON DELETE CASCADE,
    series_id        INTEGER REFERENCES series (id) ON DELETE CASCADE,
    title            TEXT NOT NULL,
    -- Lower-case, runs of other characters than letters and digits as one space.
    normalized_title TEXT NOT NULL,
    guid             TEXT NOT NULL,
    indexer          TEXT NOT NULL,
    reason           TEXT NOT NULL,
    blocklisted_at   TEXT NOT NULL,
    CHECK ((movie_id IS NULL) <> (series_id IS NULL))
);
CREATE INDEX blocklist_movie ON blocklist (movie_id);
CREATE INDEX blocklist_series ON blocklist (series_id);

-- First successful download check that did not find the download (reset when seen).
ALTER TABLE grabs ADD COLUMN absent_since TEXT;
ALTER TABLE series_grabs ADD COLUMN absent_since TEXT;
