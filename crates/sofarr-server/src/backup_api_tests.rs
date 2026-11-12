//! Backups through the API and the restore at start (spec backups: AC-06, AC-09 to
//! AC-15, AC-20, AC-22).

use std::{path::Path, sync::Arc};

use jiff::Timestamp;
use reqwest::StatusCode;
use serde_json::Value;

use crate::{
    api,
    app::App,
    backup_files::{self, Kind},
    backup_tests::{Env, Undeletable, add_movie, close, hours, movie_count, t},
    backups,
};

// --- API (AC-06, AC-09 to AC-12, AC-14, AC-20, AC-22) --------------------------------

struct Api {
    base: String,
    client: reqwest::Client,
}

impl Api {
    async fn start(app: App) -> Self {
        let app = Arc::new(app);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        let routes = api::routes(app);
        tokio::spawn(async move { axum::serve(listener, routes).await });
        Self {
            base: format!("http://{addr}"),
            client: reqwest::Client::builder()
                .no_proxy()
                .build()
                .expect("client"),
        }
    }

    async fn call(&self, method: reqwest::Method, path: &str) -> (StatusCode, Vec<u8>) {
        let response = self
            .client
            .request(method, format!("{}{path}", self.base))
            .send()
            .await
            .expect("request");
        let status = response.status();
        (status, response.bytes().await.expect("body").to_vec())
    }

    async fn json(&self, method: reqwest::Method, path: &str) -> (StatusCode, Value) {
        let (status, body) = self.call(method, path).await;
        (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
    }
}

fn german(body: &Value) -> String {
    body["error"].as_str().unwrap_or_default().to_owned()
}

#[tokio::test]
async fn a_manual_backup_through_the_api() {
    let env = Env::new("api-manual");
    let api = Api::start(env.app("").await).await;
    let (status, body) = api.json(reqwest::Method::POST, "/api/backups").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let name = body["name"].as_str().expect("name");
    assert!(name.starts_with("sofarr-manual-"), "{name}");
    assert!(body["size"].as_u64().is_some_and(|s| s > 0));
    assert!(body["time"].as_str().is_some());
    assert!(env.backups.join(name).exists());
}

#[tokio::test]
async fn concurrent_backups_get_their_own_names() {
    let env = Env::new("concurrent");
    let app = env.app("").await;
    let now = t("2026-11-12T08:00:00Z");
    let (a, b) = tokio::join!(
        app.write_backup(Kind::Manual, now),
        app.write_backup(Kind::Manual, now)
    );
    let mut names = vec![a.expect("a").name, b.expect("b").name];
    names.sort();
    assert_eq!(
        names,
        [
            "sofarr-manual-2026-11-12T080000Z-2.db",
            "sofarr-manual-2026-11-12T080000Z.db"
        ]
    );
    // A name taken by another process: the next suffix.
    let path = backup_files::reserve(&env.backups, Kind::Manual, now, None).expect("reserve");
    assert!(path.ends_with("sofarr-manual-2026-11-12T080000Z-3.db"));
}

#[tokio::test]
async fn the_backup_list() {
    let env = Env::new("api-list");
    let app = env.app("").await;
    let t0 = t("2026-11-12T03:00:00Z");
    let scheduled = app.backup_tick(t0).await.expect("tick").expect("due");
    let manual = app
        .write_backup(Kind::Manual, t0 + hours(1))
        .await
        .expect("manual");
    std::fs::write(env.backups.join("notes.db"), b"no database").expect("notes");
    std::fs::write(env.backups.join("half.db.tmp"), b"").expect("tmp");
    // A database of a newer sofarr version: listed, not valid.
    let newer = env.backups.join("newer.db");
    std::fs::copy(env.backups.join(&manual.name), &newer).expect("copy");
    {
        let mut conn = <sqlx::SqliteConnection as sqlx::Connection>::connect(&format!(
            "sqlite://{}",
            newer.display()
        ))
        .await
        .expect("open");
        sqlx::query(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time)
             VALUES (9999, 'future', 1, x'00', 0)",
        )
        .execute(&mut conn)
        .await
        .expect("future migration");
        sqlx::Connection::close(conn).await.expect("close");
    }
    app.choose_restore(&scheduled.name).await.expect("restore");
    let api = Api::start(app).await;
    let (status, list) = api.json(reqwest::Method::GET, "/api/backups").await;
    assert_eq!(status, StatusCode::OK);
    let names: Vec<&str> = list["backups"]
        .as_array()
        .expect("backups")
        .iter()
        .filter_map(|b| b["name"].as_str())
        .collect();
    assert_eq!(names[1..], [manual.name.as_str(), scheduled.name.as_str()]);
    assert_eq!(names[0], "newer.db", "other: file time, the newest");
    let first = &list["backups"][0];
    assert_eq!(first["kind"], "other");
    assert_eq!(first["valid"], false);
    assert_eq!(list["backups"][1]["kind"], "manual");
    assert_eq!(list["backups"][1]["valid"], true);
    assert_eq!(list["backups"][2]["kind"], "scheduled");
    assert_eq!(list["pending_restore"], scheduled.name.as_str());

    // A backup folder that cannot be read.
    let file_env = Env::new("api-list-unreadable");
    std::fs::write(&file_env.backups, b"not a folder").expect("file");
    let api = Api::start(file_env.app("").await).await;
    let (status, body) = api.json(reqwest::Method::GET, "/api/backups").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(german(&body).contains("Sicherungsordner"), "{body}");
}

#[tokio::test]
async fn download_and_delete_a_backup() {
    let env = Env::new("api-file");
    let app = env.app("").await;
    let file = app
        .write_backup(Kind::Manual, Timestamp::now())
        .await
        .expect("backup");
    let other = app
        .write_backup(Kind::Manual, Timestamp::now() + hours(1))
        .await
        .expect("other");
    app.choose_restore(&other.name).await.expect("restore");
    let api = Api::start(app).await;
    let (status, bytes) = api
        .call(reqwest::Method::GET, &format!("/api/backups/{}", file.name))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        bytes,
        std::fs::read(env.backups.join(&file.name)).expect("file")
    );
    let (status, _) = api
        .call(
            reqwest::Method::DELETE,
            &format!("/api/backups/{}", file.name),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!env.backups.join(&file.name).exists());
    // Deleting the pending restore clears it.
    let (status, _) = api
        .call(
            reqwest::Method::DELETE,
            &format!("/api/backups/{}", other.name),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(backup_files::read_marker(&env.db).expect("marker"), None);
}

#[tokio::test]
async fn names_outside_the_folder_or_unknown_are_not_found() {
    let env = Env::new("api-names");
    let app = env.app("").await;
    app.write_backup(Kind::Manual, Timestamp::now())
        .await
        .expect("backup");
    std::fs::write(env.backups.join("notes.db"), b"no database").expect("notes");
    std::fs::write(env.root.join("outside.db"), b"x").expect("outside");
    let before = std::fs::read(&env.db).expect("db");
    let api = Api::start(app).await;
    for name in [
        "..%2Fsofarr.db",
        "..%2Foutside.db",
        "..%5Coutside.db",
        "missing.db",
        "notes.db",
        "..",
    ] {
        for (method, path) in [
            (reqwest::Method::GET, format!("/api/backups/{name}")),
            (reqwest::Method::DELETE, format!("/api/backups/{name}")),
            (
                reqwest::Method::POST,
                format!("/api/backups/{name}/restore"),
            ),
        ] {
            let (status, _) = api.call(method.clone(), &path).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{method} {path}");
        }
    }
    assert!(env.backups.join("notes.db").exists());
    assert!(env.root.join("outside.db").exists());
    assert_eq!(backup_files::read_marker(&env.db).expect("marker"), None);
    drop(api);
    assert_eq!(std::fs::read(&env.db).expect("db").len(), before.len());
}

#[tokio::test]
async fn choosing_and_cancelling_a_restore() {
    let env = Env::new("api-restore");
    let app = env.app("").await;
    let a = app
        .write_backup(Kind::Manual, t("2026-11-12T08:00:00Z"))
        .await
        .expect("a");
    let b = app
        .write_backup(Kind::Manual, t("2026-11-12T09:00:00Z"))
        .await
        .expect("b");
    let api = Api::start(app).await;
    let restore = |name: &str| format!("/api/backups/{name}/restore");
    let (status, body) = api.json(reqwest::Method::POST, &restore(&a.name)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body["message"]
            .as_str()
            .is_some_and(|m| m.contains("nächsten Start"))
    );
    api.json(reqwest::Method::POST, &restore(&b.name)).await;
    assert_eq!(
        backup_files::read_marker(&env.db).expect("marker"),
        Some(b.name.clone())
    );
    let (status, _) = api
        .call(reqwest::Method::DELETE, "/api/backups/restore")
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(backup_files::read_marker(&env.db).expect("marker"), None);

    // Recording fails: the previous state stays.
    api.json(reqwest::Method::POST, &restore(&a.name)).await;
    std::fs::create_dir_all(backup_files::with_suffix(
        &backup_files::marker_path(&env.db),
        ".tmp",
    ))
    .expect("blocker");
    let (status, body) = api.json(reqwest::Method::POST, &restore(&b.name)).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(!german(&body).is_empty());
    assert_eq!(
        backup_files::read_marker(&env.db).expect("marker"),
        Some(a.name.clone())
    );
}

#[tokio::test]
async fn a_failed_delete_keeps_the_backup_and_is_shown() {
    let env = Env::new("api-delete-fail");
    let app = env.app("").await;
    let file = app
        .write_backup(Kind::Manual, Timestamp::now())
        .await
        .expect("backup");
    let api = Api::start(app).await;
    let guard = Undeletable::new(&env.backups.join(&file.name));
    let (status, body) = api
        .json(
            reqwest::Method::DELETE,
            &format!("/api/backups/{}", file.name),
        )
        .await;
    drop(guard);
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(german(&body).contains("nicht gelöscht"), "{body}");
    assert!(env.backups.join(&file.name).exists());
    let (_, list) = api.json(reqwest::Method::GET, "/api/backups").await;
    assert!(list["last_error"]["message"].as_str().is_some());
}

// --- restore at start (AC-13 to AC-15) -----------------------------------------------

/// Adds a migration row of a future version: a sofarr database that is not valid.
async fn make_newer(path: &Path) {
    let mut conn = <sqlx::SqliteConnection as sqlx::Connection>::connect(&format!(
        "sqlite://{}",
        path.display()
    ))
    .await
    .expect("open");
    sqlx::query(
        "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time)
         VALUES (9999, 'future', 1, x'00', 0)",
    )
    .execute(&mut conn)
    .await
    .expect("future migration");
    sqlx::Connection::close(conn).await.expect("close");
}

fn replaced(env: &Env) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(&env.root)
        .expect("root")
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter(|n| {
            n.starts_with("sofarr.db.replaced-") && !n.ends_with("-wal") && !n.ends_with("-shm")
        })
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn a_pending_restore_is_applied_at_start() {
    let env = Env::new("restore");
    let app = env.app("").await;
    add_movie(&app, 1).await;
    let file = app
        .write_backup(Kind::Manual, Timestamp::now())
        .await
        .expect("backup");
    add_movie(&app, 2).await;
    app.choose_restore(&file.name).await.expect("choose");
    close(app).await;

    let restored = backups::open_for_serve(env.config(""), Timestamp::now())
        .await
        .expect("start");
    assert_eq!(movie_count(&restored).await, 1);
    assert_eq!(backup_files::read_marker(&env.db).expect("marker"), None);
    let kept = replaced(&env);
    assert_eq!(kept.len(), 1, "{kept:?}");
    close(restored).await;
    // The replaced database keeps the newer state.
    let previous = sofarr_db::Db::open(&env.root.join(&kept[0]))
        .await
        .expect("open");
    assert_eq!(previous.movies().await.expect("movies").len(), 2);
    assert!(env.of_kind("sofarr-pre-migrate-").is_empty());
}

#[tokio::test]
async fn an_older_backup_is_migrated_after_the_restore() {
    let env = Env::new("restore-old");
    close(env.app("").await).await;
    std::fs::create_dir_all(&env.backups).expect("dir");
    let old = env.backups.join("sofarr-manual-2026-01-01T000000Z.db");
    sofarr_db::create_at_version(&old, 3).await.expect("old");
    backup_files::write_marker(&env.db, "sofarr-manual-2026-01-01T000000Z.db").expect("marker");
    let app = backups::open_for_serve(env.config(""), Timestamp::now())
        .await
        .expect("start");
    close(app).await;
    assert_eq!(replaced(&env).len(), 1, "restored");
    assert_eq!(
        sofarr_db::pending_migrations(&env.db).await.expect("db"),
        Some(Vec::new())
    );
    assert!(
        env.of_kind("sofarr-pre-migrate-").is_empty(),
        "the backup is the snapshot"
    );
}

#[tokio::test]
async fn an_unusable_pending_restore_is_dropped() {
    let env = Env::new("restore-unusable");
    let app = env.app("").await;
    add_movie(&app, 1).await;
    let file = app
        .write_backup(Kind::Manual, Timestamp::now())
        .await
        .expect("backup");
    close(app).await;
    let newer = env.backups.join("newer.db");
    std::fs::copy(env.backups.join(&file.name), &newer).expect("copy");
    make_newer(&newer).await;
    for marker in ["missing.db", "../sofarr.db", "newer.db"] {
        backup_files::write_marker(&env.db, marker).expect("marker");
        let app = backups::open_for_serve(env.config(""), Timestamp::now())
            .await
            .expect("start");
        assert_eq!(movie_count(&app).await, 1, "{marker}");
        close(app).await;
        assert_eq!(backup_files::read_marker(&env.db).expect("marker"), None);
        assert!(replaced(&env).is_empty(), "{marker}");
    }
    // Through the API a backup of a newer version is refused.
    let api = Api::start(env.app("").await).await;
    let (status, body) = api
        .json(reqwest::Method::POST, "/api/backups/newer.db/restore")
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(german(&body).contains("keine gültige Sicherung"), "{body}");
    assert_eq!(backup_files::read_marker(&env.db).expect("marker"), None);
}

#[tokio::test]
async fn a_failed_restore_leaves_the_previous_database() {
    let env = Env::new("restore-fail");
    let app = env.app("").await;
    add_movie(&app, 1).await;
    let file = app
        .write_backup(Kind::Manual, Timestamp::now())
        .await
        .expect("backup");
    add_movie(&app, 2).await;
    app.choose_restore(&file.name).await.expect("choose");
    close(app).await;
    // Copying fails: the temporary copy's place is taken by a folder.
    let blocker = backup_files::with_suffix(&env.db, ".restore-tmp");
    std::fs::create_dir_all(blocker.join("x")).expect("blocker");
    let app = backups::open_for_serve(env.config(""), Timestamp::now())
        .await
        .expect("start");
    assert_eq!(movie_count(&app).await, 2);
    assert_eq!(backup_files::read_marker(&env.db).expect("marker"), None);
    assert!(replaced(&env).is_empty());
    close(app).await;
    std::fs::remove_dir_all(&blocker).expect("unblock");

    // Migrations fail on the restored database: the previous one comes back.
    let old = env.backups.join("sofarr-manual-2026-01-01T000000Z.db");
    sofarr_db::create_at_version(&old, 3).await.expect("old");
    {
        let mut conn = <sqlx::SqliteConnection as sqlx::Connection>::connect(&format!(
            "sqlite://{}",
            old.display()
        ))
        .await
        .expect("open");
        // Migration 4 creates this table: it fails on this backup.
        sqlx::query("CREATE TABLE blocklist (id INTEGER PRIMARY KEY)")
            .execute(&mut conn)
            .await
            .expect("conflict");
        sqlx::Connection::close(conn).await.expect("close");
    }
    backup_files::write_marker(&env.db, "sofarr-manual-2026-01-01T000000Z.db").expect("marker");
    let app = backups::open_for_serve(env.config(""), Timestamp::now())
        .await
        .expect("start with the previous database");
    assert_eq!(movie_count(&app).await, 2);
    assert_eq!(backup_files::read_marker(&env.db).expect("marker"), None);
    assert!(replaced(&env).is_empty());
}
