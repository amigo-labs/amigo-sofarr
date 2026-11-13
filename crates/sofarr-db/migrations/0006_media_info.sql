-- Media info read from movie and episode files (spec mediainfo): the JSON of
-- sofarr-mediainfo, with the size and modification time of the file when it was read.

CREATE TABLE media_info (
    movie_id        INTEGER UNIQUE REFERENCES movie_files (movie_id) ON DELETE CASCADE,
    episode_file_id INTEGER UNIQUE REFERENCES episode_files (id) ON DELETE CASCADE,
    info            TEXT NOT NULL,
    size            INTEGER NOT NULL,
    modified        TEXT NOT NULL,
    read_at         TEXT NOT NULL,
    CHECK ((movie_id IS NULL) <> (episode_file_id IS NULL))
);
