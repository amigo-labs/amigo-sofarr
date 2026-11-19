//! `amigo-sofarr import-radarr` / `import-sonarr` from export folders and their errors
//! (spec library-import).

use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn run(config: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_amigo-sofarr"))
        .arg("--config")
        .arg(config)
        .args(args)
        .output()
        .expect("binary runs")
}

/// A fresh folder with a config whose database lives in it.
fn setup(name: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("sofarr-cli-import-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let config = dir.join("sofarr.toml");
    std::fs::write(
        &config,
        format!(
            "[database]\npath = '{}'\n[log]\nlevel = \"error\"\n",
            dir.join("sofarr.db").display()
        ),
    )
    .expect("config");
    (dir, config)
}

fn write(dir: &Path, name: &str, body: &str) {
    std::fs::create_dir_all(dir).expect("export dir");
    std::fs::write(dir.join(name), body).expect("export file");
}

const PROFILES: &str = r#"[{"id": 1, "name": "HD", "upgradeAllowed": true, "cutoff": 7,
    "items": [{"quality": {"id": 3, "name": "WEBDL-1080p"}, "allowed": true},
              {"quality": {"id": 7, "name": "Bluray-1080p"}, "allowed": true}]}]"#;

const MOVIES: &str = r#"[
    {"id": 1, "tmdbId": 101, "title": "Erster Film", "year": 2001, "monitored": true,
     "qualityProfileId": 1, "path": "/movies/Erster Film (2001)",
     "movieFile": {"relativePath": "Erster Film (2001).mkv", "size": 10,
        "quality": {"quality": {"name": "WEBDL-1080p"}, "revision": {"version": 1, "real": 0}}}},
    {"id": 2, "tmdbId": 102, "title": "Zweiter Film", "monitored": true, "qualityProfileId": 1},
    {"id": 3, "title": "Ohne TMDB"}
]"#;

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn import_radarr_reads_an_export_folder() {
    let (dir, config) = setup("radarr");
    let export = dir.join("export");
    write(&export, "qualityprofile.json", PROFILES);
    let definitions = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/public/config/radarr/qualitydefinition.json");
    std::fs::copy(definitions, export.join("qualitydefinition.json")).expect("definitions");
    write(&export, "customformat.json", "[]");
    write(&export, "movie.json", MOVIES);
    let export_arg = export.to_string_lossy().into_owned();

    let output = run(&config, &["import-radarr", "--exports", &export_arg]);
    assert!(output.status.success(), "{}", stderr(&output));
    let out = stdout(&output);
    assert!(
        out.contains("Importiert: 3 Einstellungen, 2 Filme (1 mit Datei), 1 übersprungen"),
        "{out}"
    );
    assert!(out.contains("Regeln: 1 Profile, 0 Custom Formats"), "{out}");

    // Only movies, no settings yet in a fresh database: the rules are incomplete.
    let (dir2, config2) = setup("radarr-movies-only");
    let movies_only = dir2.join("export");
    write(&movies_only, "movie.json", MOVIES);
    let output = run(
        &config2,
        &["import-radarr", "--exports", &movies_only.to_string_lossy()],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let out = stdout(&output);
    assert!(
        out.contains("Importiert: 0 Einstellungen, 2 Filme"),
        "{out}"
    );
    assert!(out.contains("Hinweis: Regeln noch unvollständig"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir2);
}

#[test]
fn import_sonarr_reads_series_without_episodes_from_an_export_folder() {
    let (dir, config) = setup("sonarr");
    let export = dir.join("export");
    write(&export, "qualityprofile.json", PROFILES);
    write(
        &export,
        "series.json",
        r#"[{"id": 3, "tvdbId": 77, "title": "Kleine Insel", "monitored": true,
             "qualityProfileId": 1, "path": "/tv/Kleine Insel"}]"#,
    );
    let output = run(
        &config,
        &["import-sonarr", "--exports", &export.to_string_lossy()],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let out = stdout(&output);
    assert!(
        out.contains("Importiert: 1 Einstellungen, 1 Serien, 0 Episoden, 0 Dateien"),
        "{out}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn imports_fail_on_bad_exports_and_missing_connections() {
    let (dir, config) = setup("errors");
    let empty = dir.join("empty");
    std::fs::create_dir_all(&empty).expect("empty");
    for (command, app) in [("import-radarr", "Radarr"), ("import-sonarr", "Sonarr")] {
        let output = run(&config, &[command, "--exports", &empty.to_string_lossy()]);
        assert!(!output.status.success(), "{command}");
        assert!(
            stderr(&output).contains(&format!("enthält keinen {app}-Export")),
            "{command}: {}",
            stderr(&output)
        );
    }

    let broken = dir.join("broken");
    write(&broken, "movie.json", "[{");
    let output = run(
        &config,
        &["import-radarr", "--exports", &broken.to_string_lossy()],
    );
    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("movie.json: ungültiges JSON"), "{err}");

    for (command, section) in [("import-radarr", "[radarr]"), ("import-sonarr", "[sonarr]")] {
        let output = run(&config, &[command]);
        assert!(!output.status.success(), "{command}");
        let err = stderr(&output);
        assert!(
            err.contains(&format!("{section} fehlt in der Konfiguration")),
            "{command}: {err}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
