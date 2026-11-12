-- Trailers (spec trailers): the state of the trailer download per movie or series.

CREATE TABLE trailers (
    movie_id      INTEGER UNIQUE REFERENCES movies (id) ON DELETE CASCADE,
    series_id     INTEGER UNIQUE REFERENCES series (id) ON DELETE CASCADE,
    -- waiting, downloaded, no_candidate, failed, low_space
    state         TEXT NOT NULL,
    -- ISO 639-1 code of the downloaded trailer; NULL without language.
    language      TEXT,
    -- YouTube video id of the downloaded trailer.
    video_key     TEXT,
    -- The trailer sofarr wrote, relative to the item folder ('/'-separated).
    file          TEXT,
    message       TEXT,
    attempts      INTEGER NOT NULL DEFAULT 0,
    next_attempt  TEXT,
    -- Set when an import made the item eligible: first in line for the next run.
    first_in_line TEXT,
    updated_at    TEXT NOT NULL,
    CHECK ((movie_id IS NULL) <> (series_id IS NULL))
);
