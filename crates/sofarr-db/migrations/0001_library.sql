-- Library, configuration and grab log (Phase 3).

-- Configuration in the shape of the Radarr API (quality profiles, custom formats, …),
-- one JSON document per key.
CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE movies (
    id                 INTEGER PRIMARY KEY,
    tmdb_id            INTEGER NOT NULL UNIQUE,
    imdb_id            TEXT,
    title              TEXT NOT NULL,
    original_title     TEXT,
    title_de           TEXT,
    year               INTEGER,
    runtime            INTEGER NOT NULL DEFAULT 0,
    original_language  TEXT,
    monitored          INTEGER NOT NULL,
    quality_profile_id INTEGER NOT NULL,
    path               TEXT NOT NULL,
    radarr_id          INTEGER,
    added_at           TEXT NOT NULL
);

CREATE TABLE movie_titles (
    movie_id INTEGER NOT NULL REFERENCES movies (id) ON DELETE CASCADE,
    title    TEXT NOT NULL,
    PRIMARY KEY (movie_id, title)
);

CREATE TABLE movie_files (
    movie_id            INTEGER PRIMARY KEY REFERENCES movies (id) ON DELETE CASCADE,
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

-- Every release sofarr decided to take. In shadow mode nothing is sent anywhere.
CREATE TABLE grabs (
    id                  INTEGER PRIMARY KEY,
    movie_id            INTEGER NOT NULL REFERENCES movies (id) ON DELETE CASCADE,
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
CREATE INDEX grabs_movie ON grabs (movie_id);
CREATE INDEX grabs_status ON grabs (status);
