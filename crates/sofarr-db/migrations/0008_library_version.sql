-- A counter that every change of the library or its settings raises (spec web-ui-library
-- AC-04): the list pages keep their computed lists until it moves.

CREATE TABLE library_version (
    id      INTEGER PRIMARY KEY CHECK (id = 1),
    version INTEGER NOT NULL
);
INSERT INTO library_version (id, version) VALUES (1, 0);

CREATE TRIGGER settings_insert_version AFTER INSERT ON settings
BEGIN UPDATE library_version SET version = version + 1; END;
CREATE TRIGGER settings_update_version AFTER UPDATE ON settings
BEGIN UPDATE library_version SET version = version + 1; END;
CREATE TRIGGER settings_delete_version AFTER DELETE ON settings
BEGIN UPDATE library_version SET version = version + 1; END;

CREATE TRIGGER movies_insert_version AFTER INSERT ON movies
BEGIN UPDATE library_version SET version = version + 1; END;
CREATE TRIGGER movies_update_version AFTER UPDATE ON movies
BEGIN UPDATE library_version SET version = version + 1; END;
CREATE TRIGGER movies_delete_version AFTER DELETE ON movies
BEGIN UPDATE library_version SET version = version + 1; END;

CREATE TRIGGER movie_titles_insert_version AFTER INSERT ON movie_titles
BEGIN UPDATE library_version SET version = version + 1; END;
CREATE TRIGGER movie_titles_update_version AFTER UPDATE ON movie_titles
BEGIN UPDATE library_version SET version = version + 1; END;
CREATE TRIGGER movie_titles_delete_version AFTER DELETE ON movie_titles
BEGIN UPDATE library_version SET version = version + 1; END;

CREATE TRIGGER movie_files_insert_version AFTER INSERT ON movie_files
BEGIN UPDATE library_version SET version = version + 1; END;
CREATE TRIGGER movie_files_update_version AFTER UPDATE ON movie_files
BEGIN UPDATE library_version SET version = version + 1; END;
CREATE TRIGGER movie_files_delete_version AFTER DELETE ON movie_files
BEGIN UPDATE library_version SET version = version + 1; END;

CREATE TRIGGER series_insert_version AFTER INSERT ON series
BEGIN UPDATE library_version SET version = version + 1; END;
CREATE TRIGGER series_update_version AFTER UPDATE ON series
BEGIN UPDATE library_version SET version = version + 1; END;
CREATE TRIGGER series_delete_version AFTER DELETE ON series
BEGIN UPDATE library_version SET version = version + 1; END;

CREATE TRIGGER series_titles_insert_version AFTER INSERT ON series_titles
BEGIN UPDATE library_version SET version = version + 1; END;
CREATE TRIGGER series_titles_update_version AFTER UPDATE ON series_titles
BEGIN UPDATE library_version SET version = version + 1; END;
CREATE TRIGGER series_titles_delete_version AFTER DELETE ON series_titles
BEGIN UPDATE library_version SET version = version + 1; END;

CREATE TRIGGER episodes_insert_version AFTER INSERT ON episodes
BEGIN UPDATE library_version SET version = version + 1; END;
CREATE TRIGGER episodes_update_version AFTER UPDATE ON episodes
BEGIN UPDATE library_version SET version = version + 1; END;
CREATE TRIGGER episodes_delete_version AFTER DELETE ON episodes
BEGIN UPDATE library_version SET version = version + 1; END;

CREATE TRIGGER episode_files_insert_version AFTER INSERT ON episode_files
BEGIN UPDATE library_version SET version = version + 1; END;
CREATE TRIGGER episode_files_update_version AFTER UPDATE ON episode_files
BEGIN UPDATE library_version SET version = version + 1; END;
CREATE TRIGGER episode_files_delete_version AFTER DELETE ON episode_files
BEGIN UPDATE library_version SET version = version + 1; END;
