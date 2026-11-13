-- Subtitles (spec subtitles): the own subtitle files sofarr wrote next to a video, and
-- the needed subtitles of each file with their search schedule.

CREATE TABLE subtitles (
    id                INTEGER PRIMARY KEY,
    movie_id          INTEGER REFERENCES movie_files (movie_id) ON DELETE CASCADE,
    episode_file_id   INTEGER REFERENCES episode_files (id) ON DELETE CASCADE,
    -- ISO 639-1.
    language          TEXT NOT NULL,
    -- 'normal' or 'forced'.
    kind              TEXT NOT NULL,
    hearing_impaired  INTEGER NOT NULL,
    -- File name in the video's folder.
    file_name         TEXT NOT NULL,
    provider_id       TEXT NOT NULL,
    release           TEXT NOT NULL,
    -- 'hash', 'release:<n>' or 'title'.
    match_quality     TEXT NOT NULL,
    created_at        TEXT NOT NULL,
    CHECK ((movie_id IS NULL) <> (episode_file_id IS NULL))
);
-- At most one own subtitle per file, language and kind (AC-03).
CREATE UNIQUE INDEX subtitles_movie ON subtitles (movie_id, language, kind)
    WHERE movie_id IS NOT NULL;
CREATE UNIQUE INDEX subtitles_episode ON subtitles (episode_file_id, language, kind)
    WHERE episode_file_id IS NOT NULL;

CREATE TABLE subtitle_needs (
    movie_id          INTEGER REFERENCES movie_files (movie_id) ON DELETE CASCADE,
    episode_file_id   INTEGER REFERENCES episode_files (id) ON DELETE CASCADE,
    language          TEXT NOT NULL,
    kind              TEXT NOT NULL,
    last_search       TEXT,
    -- NULL: due now.
    next_search       TEXT,
    -- Whether the last search found a candidate (NULL: not searched yet).
    found             INTEGER,
    -- Shadow mode: the release that would have been downloaded (AC-13).
    would_download    TEXT,
    CHECK ((movie_id IS NULL) <> (episode_file_id IS NULL))
);
CREATE UNIQUE INDEX subtitle_needs_movie ON subtitle_needs (movie_id, language, kind)
    WHERE movie_id IS NOT NULL;
CREATE UNIQUE INDEX subtitle_needs_episode ON subtitle_needs (episode_file_id, language, kind)
    WHERE episode_file_id IS NOT NULL;
