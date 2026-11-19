//! `amigo-sofarr sync-import-lists` runs the import list sync once and prints how many
//! movies it added (spec import-lists).

use std::process::Command;

#[test]
fn sync_import_lists_prints_the_added_count() {
    let dir = std::env::temp_dir().join(format!("sofarr-cli-lists-{}", std::process::id()));
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

    let output = Command::new(env!("CARGO_BIN_EXE_amigo-sofarr"))
        .arg("--config")
        .arg(&config)
        .arg("sync-import-lists")
        .output()
        .expect("binary runs");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(stdout.trim(), "Hinzugefügt: 0");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A fake TMDB that knows person 7 with one movie.
async fn fake_tmdb() -> std::net::SocketAddr {
    use axum::{Json, Router, extract::Path, routing::get};
    use serde_json::json;

    let router = Router::new()
        .route(
            "/3/person/{id}/movie_credits",
            get(|Path(id): Path<i64>| async move {
                Json(if id == 7 {
                    json!({"cast": [{"id": 501, "title": "Garten Eins"}], "crew": []})
                } else {
                    json!({"cast": [], "crew": []})
                })
            }),
        )
        .route(
            "/3/movie/{id}",
            get(|Path(id): Path<i64>| async move {
                Json(
                    json!({"id": id, "imdb_id": "tt0000501", "title": "Garten Eins",
                    "original_title": "Garten Eins", "release_date": "2001-03-04",
                    "runtime": 100, "status": "Released", "original_language": "en",
                    "overview": "Text."}),
                )
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move { axum::serve(listener, router).await });
    addr
}

/// A TMDB key made at runtime (code scanning flags hard-coded keys, also in tests).
fn runtime_key() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// With one usable list, the command adds its movie in shadow mode and prints the count
/// and the title (spec import-lists AC-10).
#[tokio::test]
async fn sync_import_lists_adds_and_prints_a_lists_movie_in_shadow_mode() {
    let dir = std::env::temp_dir().join(format!("sofarr-cli-lists-usable-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let root = dir.join("filme");
    std::fs::create_dir_all(&root).expect("dir");
    let addr = fake_tmdb().await;
    let db_path = dir.join("sofarr.db");
    let db = sofarr_db::Db::open(&db_path).await.expect("db");
    db.put_setting(
        "importlist",
        &serde_json::json!([{"id": 1, "name": "Regie", "implementation": "TMDbPersonImport",
            "enabled": true, "enableAuto": true, "monitor": "movieOnly",
            "qualityProfileId": 1, "rootFolderPath": root.to_string_lossy(),
            "searchOnAdd": false, "minimumAvailability": "released", "tags": [],
            "fields": [{"name": "personId", "value": 7}]}]),
    )
    .await
    .expect("lists");
    let config = dir.join("sofarr.toml");
    std::fs::write(
        &config,
        format!(
            "[database]\npath = '{}'\n[log]\nlevel = \"error\"\n[automation]\nmode = \"shadow\"\n\
             [tmdb]\napi_key = \"{}\"\nurl = \"http://{addr}/3\"\n",
            db_path.display(),
            runtime_key()
        ),
    )
    .expect("config");

    let output = tokio::task::spawn_blocking(move || {
        Command::new(env!("CARGO_BIN_EXE_amigo-sofarr"))
            .arg("--config")
            .arg(&config)
            .arg("sync-import-lists")
            .output()
            .expect("binary runs")
    })
    .await
    .expect("join");
    assert!(output.status.success(), "the command succeeds");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().map(str::trim).collect();
    assert_eq!(lines, ["Hinzugefügt: 1", "Garten Eins"]);
    assert!(db.movie_by_tmdb(501).await.expect("db").is_some());
    db.pool().close().await;
    let _ = std::fs::remove_dir_all(&dir);
}
