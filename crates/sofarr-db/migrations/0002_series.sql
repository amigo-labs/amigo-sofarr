-- Series, episodes and their files (Phase 5).

CREATE TABLE series (
    id                 INTEGER PRIMARY KEY,
    tvdb_id            INTEGER NOT NULL UNIQUE,
    imdb_id            TEXT,
    title              TEXT NOT NULL,
    title_de           TEXT,
    year               INTEGER,
    runtime            INTEGER NOT NULL DEFAULT 0,
    original_language  TEXT,
    series_type        TEXT NOT NULL DEFAULT 'standard',
    monitored          INTEGER NOT NULL,
    season_folder      INTEGER NOT NULL DEFAULT 1,
    quality_profile_id INTEGER NOT NULL,
    path               TEXT NOT NULL,
    sonarr_id          INTEGER,
    added_at           TEXT NOT NULL
);

CREATE TABLE series_titles (
    series_id INTEGER NOT NULL REFERENCES series (id) ON DELETE CASCADE,
    title     TEXT NOT NULL,
    PRIMARY KEY (series_id, title)
);

CREATE TABLE episode_files (
    id                  INTEGER PRIMARY KEY,
    series_id           INTEGER NOT NULL REFERENCES series (id) ON DELETE CASCADE,
    relative_path       TEXT NOT NULL,
    size                INTEGER NOT NULL,
    quality             TEXT NOT NULL,
    revision_version    INTEGER NOT NULL DEFAULT 1,
    revision_real       INTEGER NOT NULL DEFAULT 0,
    release_group       TEXT,
    scene_name          TEXT,
    custom_format_score INTEGER NOT NULL DEFAULT 0,
    added_at            TEXT NOT NULL
);

CREATE TABLE episodes (
    id          INTEGER PRIMARY KEY,
    series_id   INTEGER NOT NULL REFERENCES series (id) ON DELETE CASCADE,
    tvdb_id     INTEGER,
    season      INTEGER NOT NULL,
    number      INTEGER NOT NULL,
    absolute    INTEGER,
    title       TEXT,
    title_de    TEXT,
    air_date    TEXT,
    monitored   INTEGER NOT NULL,
    file_id     INTEGER REFERENCES episode_files (id) ON DELETE SET NULL,
    UNIQUE (series_id, season, number)
);

CREATE TABLE series_grabs (
    id                  INTEGER PRIMARY KEY,
    series_id           INTEGER NOT NULL REFERENCES series (id) ON DELETE CASCADE,
    -- Comma-separated episode ids covered by the release.
    episode_ids         TEXT NOT NULL,
    title               TEXT NOT NULL,
    guid                TEXT NOT NULL,
    indexer             TEXT NOT NULL,
    download_url        TEXT,
    quality             TEXT NOT NULL,
    custom_format_score INTEGER NOT NULL,
    size                INTEGER NOT NULL,
    status              TEXT NOT NULL,
    download_id         TEXT,
    message             TEXT,
    grabbed_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
);
CREATE INDEX series_grabs_series ON series_grabs (series_id);
