-- Re-encoding (spec reencode-file): every attempt to re-encode a library file. The links to
-- the movie or episode file are cleared when they go; title and path stay as copied.

CREATE TABLE reencode_attempts (
    id              INTEGER PRIMARY KEY,
    movie_id        INTEGER REFERENCES movies (id) ON DELETE SET NULL,
    episode_file_id INTEGER REFERENCES episode_files (id) ON DELETE SET NULL,
    title           TEXT NOT NULL,
    path            TEXT NOT NULL,
    -- server or cli
    runner          TEXT NOT NULL,
    started_at      TEXT NOT NULL,
    finished_at     TEXT,
    -- replaced, no_gain, failed, refused; NULL while it runs
    outcome         TEXT,
    reason          TEXT,
    size_before     INTEGER NOT NULL,
    size_after      INTEGER
);
CREATE INDEX reencode_attempts_movie ON reencode_attempts (movie_id);
CREATE INDEX reencode_attempts_episode_file ON reencode_attempts (episode_file_id);
