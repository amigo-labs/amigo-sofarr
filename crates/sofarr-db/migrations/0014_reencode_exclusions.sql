-- Re-encoding (spec ui-reencode): library files the owner kept out of the background job.
-- They go with their file record; a movie keeps its record when its file is upgraded.

CREATE TABLE reencode_exclusions (
    movie_id        INTEGER UNIQUE REFERENCES movie_files (movie_id) ON DELETE CASCADE,
    episode_file_id INTEGER UNIQUE REFERENCES episode_files (id) ON DELETE CASCADE,
    excluded_at     TEXT NOT NULL,
    CHECK ((movie_id IS NULL) <> (episode_file_id IS NULL))
);
