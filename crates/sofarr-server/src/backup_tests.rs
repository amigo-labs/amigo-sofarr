//! Backups (spec backups): scheduled, manual and pre-migration backups, pruning, the
//! pending restore and its application at start.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use jiff::{SignedDuration, Timestamp};

use crate::{
    app::App,
    backup_files::{self, Kind},
    backups,
    config::Config,
};

/// A database and a backup folder in a fresh temporary directory.
pub struct Env {
    pub root: PathBuf,
    pub db: PathBuf,
    pub backups: PathBuf,
}

impl Env {
    pub fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("sofarr-bk-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("root");
        Self {
            db: root.join("sofarr.db"),
            backups: root.join("backups"),
            root,
        }
    }

    /// The config with `extra`; keys of a leading `[backup]` table join the backup table.
    pub fn config(&self, extra: &str) -> Config {
        let (backup, rest) = match extra.strip_prefix("[backup]\n") {
            Some(rest) => rest
                .split_once("\n[")
                .map_or((rest, String::new()), |(keys, more)| {
                    (keys, format!("[{more}"))
                }),
            None => ("", extra.to_owned()),
        };
        Config::parse(&format!(
            "[database]\npath = '{}'\n[backup]\ndir = '{}'\n{backup}\n{rest}",
            self.db.display(),
            self.backups.display()
        ))
        .expect("config")
    }

    pub async fn app(&self, extra: &str) -> App {
        App::new(self.config(extra)).await.expect("app")
    }

    /// File names in the backup folder, sorted.
    pub fn files(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.backups)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter_map(|e| e.file_name().to_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    pub fn of_kind(&self, prefix: &str) -> Vec<String> {
        self.files()
            .into_iter()
            .filter(|n| n.starts_with(prefix))
            .collect()
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Closes the app's database, so its files can be moved (Windows keeps open files).
pub async fn close(app: App) {
    app.db.pool().close().await;
}

pub fn t(s: &str) -> Timestamp {
    s.parse().expect("ts")
}

pub fn hours(n: i64) -> SignedDuration {
    SignedDuration::from_hours(n)
}

pub async fn movie_count(app: &App) -> usize {
    app.db.movies().await.expect("movies").len()
}

pub async fn add_movie(app: &App, tmdb: i64) {
    app.db
        .upsert_movie(
            &sofarr_db::NewMovie {
                tmdb_id: tmdb,
                imdb_id: None,
                title: format!("Film {tmdb}"),
                original_title: None,
                title_de: None,
                year: Some(2020),
                runtime_minutes: 90,
                original_language: None,
                monitored: true,
                quality_profile_id: 1,
                path: format!("/m/{tmdb}"),
                radarr_id: None,
                alternative_titles: vec!["A".to_owned(), "B".to_owned(), "C".to_owned()],
                availability: sofarr_db::Availability::default(),
                tags: vec![],
            },
            Timestamp::now(),
        )
        .await
        .expect("movie");
}

// --- scheduled backups (AC-01, AC-03, AC-18) ---------------------------------------

#[tokio::test]
async fn a_scheduled_backup_is_due_once_per_interval() {
    let env = Env::new("schedule");
    let app = env.app("").await;
    let t0 = t("2026-11-12T03:00:00Z");
    let first = app.backup_tick(t0).await.expect("tick").expect("due");
    assert_eq!(first.kind, Kind::Scheduled);
    assert_eq!(first.name, "sofarr-2026-11-12T030000Z.db");
    assert!(
        app.backup_tick(t0 + hours(1))
            .await
            .expect("tick")
            .is_none()
    );
    assert!(
        app.backup_tick(t0 + hours(24))
            .await
            .expect("tick")
            .is_some()
    );
    // A backup carrying a time in the future does not count as the newest.
    std::fs::copy(
        env.backups.join(&first.name),
        env.backups.join("sofarr-2099-01-01T000000Z.db"),
    )
    .expect("future");
    assert!(
        app.backup_tick(t0 + hours(30))
            .await
            .expect("tick")
            .is_none()
    );
    // Down for days: one backup, not one per missed interval.
    assert!(
        app.backup_tick(t0 + hours(100))
            .await
            .expect("tick")
            .is_some()
    );
    assert!(
        app.backup_tick(t0 + hours(100))
            .await
            .expect("tick")
            .is_none()
    );
    assert!(backups::CHECK_INTERVAL <= std::time::Duration::from_secs(5 * 60));
}

#[tokio::test]
async fn a_backup_runs_while_the_database_answers() {
    let env = Env::new("answers");
    let app = Arc::new(env.app("").await);
    for tmdb in 1..=200 {
        add_movie(&app, tmdb).await;
    }
    let backup = {
        let app = Arc::clone(&app);
        tokio::spawn(async move { app.write_backup(Kind::Manual, Timestamp::now()).await })
    };
    for _ in 0..20 {
        assert_eq!(movie_count(&app).await, 200);
    }
    backup.await.expect("join").expect("backup");
}

#[tokio::test]
async fn scheduled_backups_beyond_keep_are_pruned() {
    let env = Env::new("prune");
    let app = env.app("[backup]\nkeep = 2\n").await;
    let t0 = t("2026-11-01T03:00:00Z");
    for day in 0..3 {
        app.backup_tick(t0 + hours(24 * day)).await.expect("tick");
    }
    assert_eq!(
        env.of_kind("sofarr-2026"),
        [
            "sofarr-2026-11-02T030000Z.db",
            "sofarr-2026-11-03T030000Z.db"
        ]
    );
    // A future one, a manual one, one copied by hand, and the pending restore.
    let any = env.backups.join("sofarr-2026-11-03T030000Z.db");
    std::fs::copy(&any, env.backups.join("sofarr-2099-01-01T000000Z.db")).expect("future");
    std::fs::copy(&any, env.backups.join("mine.db")).expect("other");
    app.write_backup(Kind::Manual, t0).await.expect("manual");
    app.choose_restore("sofarr-2026-11-02T030000Z.db")
        .await
        .expect("restore");
    app.backup_tick(t0 + hours(72)).await.expect("tick");
    assert_eq!(
        env.files(),
        [
            "mine.db",
            "sofarr-2026-11-02T030000Z.db",
            "sofarr-2026-11-04T030000Z.db",
            "sofarr-manual-2026-11-01T030000Z.db",
        ]
    );
}

#[tokio::test]
async fn interval_zero_writes_no_scheduled_backups() {
    let env = Env::new("off");
    let app = env.app("[backup]\ninterval_hours = 0\n").await;
    assert!(
        app.backup_tick(Timestamp::now())
            .await
            .expect("tick")
            .is_none()
    );
    let manual = app
        .write_backup(Kind::Manual, Timestamp::now())
        .await
        .expect("manual");
    assert_eq!(env.files(), [manual.name]);
    // Pre-migration backups still work.
    let old = Env::new("off-premigrate");
    sofarr_db::create_at_version(&old.db, 3)
        .await
        .expect("old db");
    close(old.app("[backup]\ninterval_hours = 0\n").await).await;
    assert_eq!(old.of_kind("sofarr-pre-migrate-0004-").len(), 1);
}

// --- consistency (AC-02, AC-21) ------------------------------------------------------

#[tokio::test]
async fn a_backup_is_valid_and_consistent() {
    let env = Env::new("consistent");
    let app = Arc::new(env.app("").await);
    add_movie(&app, 1).await;
    // Movies with three titles each, one transaction per movie, while the backup runs.
    let writer = {
        let app = Arc::clone(&app);
        tokio::spawn(async move {
            for tmdb in 2..=300 {
                add_movie(&app, tmdb).await;
            }
        })
    };
    let file = app
        .write_backup(Kind::Manual, Timestamp::now())
        .await
        .expect("backup");
    writer.await.expect("writer");
    let path = env.backups.join(&file.name);
    assert!(sofarr_db::inspect(&path).await.valid);
    let copy = env.root.join("copy.db");
    std::fs::copy(&path, &copy).expect("copy");
    let restored = sofarr_db::Db::open(&copy).await.expect("open backup");
    let movies = restored.movies().await.expect("movies");
    assert!(
        movies.iter().any(|m| m.tmdb_id == 1),
        "committed before the request"
    );
    for movie in &movies {
        assert_eq!(movie.alternative_titles.len(), 3, "{}", movie.title);
    }
}

#[tokio::test]
async fn no_backup_contains_the_config() {
    let env = Env::new("secret");
    let extra = "[nzbget]\nurl = \"http://nzbget\"\nusername = \"u\"\npassword = \"geheim-4711\"\n";
    let app = env.app(extra).await;
    let file = app
        .write_backup(Kind::Manual, Timestamp::now())
        .await
        .expect("backup");
    let bytes = std::fs::read(env.backups.join(&file.name)).expect("read");
    let needle = b"geheim-4711";
    assert!(!bytes.windows(needle.len()).any(|w| w == needle));
}

// --- pre-migration backups (AC-04, AC-05, AC-08) -------------------------------------

#[tokio::test]
async fn a_start_with_pending_migrations_writes_a_pre_migration_backup() {
    let env = Env::new("premigrate");
    sofarr_db::create_at_version(&env.db, 3)
        .await
        .expect("old db");
    std::fs::create_dir_all(&env.backups).expect("dir");
    for day in 1..=3 {
        std::fs::copy(
            &env.db,
            env.backups
                .join(format!("sofarr-pre-migrate-0003-2026-10-0{day}T000000Z.db")),
        )
        .expect("older");
    }
    let app = env.app("").await;
    let pre = env.of_kind("sofarr-pre-migrate-");
    assert_eq!(pre.len(), 3, "{pre:?}");
    assert!(
        pre.iter().all(|n| !n.contains("2026-10-01")),
        "oldest pruned"
    );
    let new: Vec<&String> = pre.iter().filter(|n| n.contains("-0004-")).collect();
    assert_eq!(new.len(), 1, "{pre:?}");
    // The backup holds the old schema, the database the new one.
    let pending = sofarr_db::pending_migrations(&env.backups.join(new[0]))
        .await
        .expect("pending")
        .expect("file");
    assert_eq!(pending.first().map(|p| p.version), Some(4));
    close(app).await;
    assert_eq!(
        sofarr_db::pending_migrations(&env.db).await.expect("db"),
        Some(Vec::new())
    );
}

#[tokio::test]
async fn a_start_without_pending_migrations_writes_none() {
    let env = Env::new("nopre");
    close(env.app("").await).await;
    close(env.app("").await).await;
    assert!(env.files().is_empty(), "{:?}", env.files());
}

#[tokio::test]
async fn a_failed_pre_migration_backup_stops_before_the_migrations() {
    let env = Env::new("prefail");
    sofarr_db::create_at_version(&env.db, 3)
        .await
        .expect("old db");
    std::fs::write(env.root.join("blocker"), b"file").expect("blocker");
    let config = Config::parse(&format!(
        "[database]\npath = '{}'\n[backup]\ndir = '{}'\n",
        env.db.display(),
        env.root.join("blocker").join("backups").display()
    ))
    .expect("config");
    let error = App::new(config).await.err().expect("must not start");
    let message = format!("{error:#}");
    assert!(
        message.contains("Migrationen nicht ausgeführt"),
        "{message}"
    );
    let pending = sofarr_db::pending_migrations(&env.db)
        .await
        .expect("db")
        .expect("file");
    assert_eq!(pending.first().map(|p| p.version), Some(4));
}

// --- failures (AC-07, AC-22) ---------------------------------------------------------

#[tokio::test]
async fn a_failed_backup_leaves_nothing_and_is_shown() {
    let env = Env::new("fail");
    let app = env.app("").await;
    let now = t("2026-11-12T08:00:00Z");
    let older = app
        .write_backup(Kind::Manual, now - hours(1))
        .await
        .expect("older");
    // The `.tmp` target is taken by a folder: VACUUM INTO fails.
    let name = backup_files::file_name(Kind::Manual, now, None, 1);
    std::fs::create_dir_all(env.backups.join(format!("{name}.tmp"))).expect("blocker");
    assert!(app.write_backup(Kind::Manual, now).await.is_err());
    assert!(!env.backups.join(&name).exists(), "no partial backup");
    assert!(env.backups.join(&older.name).exists());
    let list = app.backup_list().await.expect("list");
    let error = list.last_error.expect("shown");
    assert_eq!(error.time, now);
    assert!(!error.message.is_empty());
    // A later backup clears it.
    app.write_backup(Kind::Manual, now + hours(1))
        .await
        .expect("later");
    assert!(app.backup_list().await.expect("list").last_error.is_none());

    // A folder that cannot be created.
    std::fs::write(env.root.join("blocker"), b"file").expect("blocker");
    let mut config = env.config("");
    config.backup.dir = env.root.join("blocker").join("backups");
    let blocked = App::new(config).await.expect("app");
    assert!(blocked.write_backup(Kind::Manual, now).await.is_err());
}

/// Makes deleting `file` fail until the guard is dropped.
pub struct Undeletable {
    #[cfg(windows)]
    _file: std::fs::File,
    #[cfg(unix)]
    dir: PathBuf,
}

impl Undeletable {
    pub fn new(file: &Path) -> Self {
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            let handle = std::fs::OpenOptions::new()
                .read(true)
                // Others may read (and inspect) it, nobody may delete it.
                .share_mode(1)
                .open(file)
                .expect("lock file");
            Self { _file: handle }
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let dir = file.parent().expect("dir").to_path_buf();
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).expect("chmod");
            Self { dir }
        }
    }
}

#[cfg(unix)]
impl Drop for Undeletable {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&self.dir, std::fs::Permissions::from_mode(0o755));
    }
}

/// Pruning that fails keeps the old backup; the new one still counts. Windows only: there
/// an open file cannot be deleted, while under unix the read-only folder that blocks the
/// deletion would block the new backup too (the delete case runs on both, see
/// `backup_api_tests::a_failed_delete_keeps_the_backup_and_is_shown`).
#[cfg(windows)]
#[tokio::test]
async fn a_failed_pruning_keeps_the_backup_and_is_shown() {
    let env = Env::new("prunefail");
    let app = env.app("[backup]\nkeep = 1\n").await;
    let t0 = t("2026-11-01T03:00:00Z");
    let old = app.backup_tick(t0).await.expect("tick").expect("first");
    let guard = Undeletable::new(&env.backups.join(&old.name));
    let new = app
        .backup_tick(t0 + hours(24))
        .await
        .expect("tick")
        .expect("written");
    drop(guard);
    assert!(env.backups.join(&new.name).exists());
    assert!(env.backups.join(&old.name).exists(), "kept");
    let error = app
        .backup_list()
        .await
        .expect("list")
        .last_error
        .expect("shown");
    assert!(
        error.message.contains("nicht gelöscht"),
        "{}",
        error.message
    );
}

// --- after the first verification (AC-01, AC-13, AC-20, AC-22) ------------------------

#[tokio::test]
async fn the_scheduler_writes_a_backup_soon_after_start() {
    let env = Env::new("spawn");
    let app = Arc::new(env.app("").await);
    let job = backups::spawn_schedule(&app).expect("scheduled backups are on");
    let mut found = Vec::new();
    for _ in 0..100 {
        found = env.of_kind("sofarr-20");
        if !found.is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    job.abort();
    assert_eq!(found.len(), 1, "{:?}", env.files());

    let off = Env::new("spawn-off");
    let app = Arc::new(off.app("[backup]\ninterval_hours = 0\n").await);
    assert!(backups::spawn_schedule(&app).is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn api_requests_are_answered_while_a_backup_runs() {
    let env = Env::new("busy");
    let app = Arc::new(env.app("").await);
    // A database large enough that copying it takes a while.
    let chunk = serde_json::Value::String("x".repeat(1 << 20));
    for i in 0..64 {
        app.db
            .put_setting(&format!("filler{i}"), &chunk)
            .await
            .expect("filler");
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let routes = crate::api::routes(Arc::clone(&app));
    tokio::spawn(async move { axum::serve(listener, routes).await });
    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("client");
    let backup = {
        let app = Arc::clone(&app);
        tokio::spawn(async move {
            let result = app.write_backup(Kind::Manual, Timestamp::now()).await;
            (result, std::time::Instant::now())
        })
    };
    let mut answered = Vec::new();
    while !backup.is_finished() {
        let response = client
            .get(format!("http://{addr}/api/movies"))
            .send()
            .await
            .expect("request");
        assert!(response.status().is_success());
        answered.push(std::time::Instant::now());
    }
    let (result, done) = backup.await.expect("join");
    result.expect("backup");
    assert!(
        answered.iter().any(|t| *t < done),
        "no request answered while the backup ran"
    );
}

#[tokio::test]
async fn journal_files_go_aside_with_the_replaced_database() {
    let env = Env::new("journals");
    let app = env.app("").await;
    add_movie(&app, 1).await;
    let file = app
        .write_backup(Kind::Manual, Timestamp::now())
        .await
        .expect("backup");
    app.choose_restore(&file.name).await.expect("choose");
    close(app).await;
    let wal = backup_files::with_suffix(&env.db, "-wal");
    let shm = backup_files::with_suffix(&env.db, "-shm");
    std::fs::write(&wal, b"old wal").expect("wal");
    std::fs::write(&shm, b"old shm").expect("shm");

    let restored = backups::open_for_serve(env.config(""), Timestamp::now())
        .await
        .expect("start");
    assert_eq!(movie_count(&restored).await, 1);
    close(restored).await;
    let replaced: Vec<String> = std::fs::read_dir(&env.root)
        .expect("root")
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter(|n| n.starts_with("sofarr.db.replaced-") && n.ends_with('Z'))
        .collect();
    assert_eq!(replaced.len(), 1, "{replaced:?}");
    let aside = env.root.join(&replaced[0]);
    assert_eq!(
        std::fs::read(backup_files::with_suffix(&aside, "-wal")).expect("wal aside"),
        b"old wal"
    );
    assert_eq!(
        std::fs::read(backup_files::with_suffix(&aside, "-shm")).expect("shm aside"),
        b"old shm"
    );
    // The restored database never got the old journal files.
    for journal in [&wal, &shm] {
        let content = std::fs::read(journal).unwrap_or_default();
        assert!(!content.starts_with(b"old"), "{}", journal.display());
    }
}

#[tokio::test]
async fn backup_operations_wait_for_each_other() {
    let env = Env::new("serial");
    let app = Arc::new(env.app("").await);
    let a = app
        .write_backup(Kind::Manual, t("2026-11-12T08:00:00Z"))
        .await
        .expect("a");
    let b = app
        .write_backup(Kind::Manual, t("2026-11-12T09:00:00Z"))
        .await
        .expect("b");
    let before = env.files();
    // An operation in progress holds the lock: every other one waits for it.
    let guard = app.backup_lock.lock().await;
    let write = tokio::spawn({
        let app = Arc::clone(&app);
        async move {
            app.write_backup(Kind::Manual, Timestamp::now())
                .await
                .map(|_| ())
        }
    });
    let delete = tokio::spawn({
        let app = Arc::clone(&app);
        let name = a.name.clone();
        async move { app.delete_backup(&name).await }
    });
    let choose = tokio::spawn({
        let app = Arc::clone(&app);
        let name = b.name.clone();
        async move { app.choose_restore(&name).await }
    });
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert!(!write.is_finished() && !delete.is_finished() && !choose.is_finished());
    assert_eq!(env.files(), before, "nothing done while the lock is held");
    assert_eq!(backup_files::read_marker(&env.db).expect("marker"), None);
    drop(guard);
    write.await.expect("join").expect("write");
    assert!(delete.await.expect("join").is_ok());
    assert!(choose.await.expect("join").is_ok());
    assert!(!env.backups.join(&a.name).exists());
    assert_eq!(
        backup_files::read_marker(&env.db).expect("marker"),
        Some(b.name)
    );
}

/// Pruning pre-migration backups at start fails: the start goes on, the list shows the
/// error. Windows only, like `a_failed_pruning_keeps_the_backup_and_is_shown`.
#[cfg(windows)]
#[tokio::test]
async fn a_failed_pre_migration_pruning_is_shown() {
    let env = Env::new("prepruning");
    sofarr_db::create_at_version(&env.db, 3)
        .await
        .expect("old db");
    std::fs::create_dir_all(&env.backups).expect("dir");
    for day in 1..=3 {
        std::fs::copy(
            &env.db,
            env.backups
                .join(format!("sofarr-pre-migrate-0003-2026-10-0{day}T000000Z.db")),
        )
        .expect("older");
    }
    let oldest = env
        .backups
        .join("sofarr-pre-migrate-0003-2026-10-01T000000Z.db");
    let guard = Undeletable::new(&oldest);
    let app = env.app("").await;
    drop(guard);
    assert!(oldest.exists());
    let error = app
        .backup_list()
        .await
        .expect("list")
        .last_error
        .expect("shown");
    assert!(
        error.message.contains("nicht gelöscht"),
        "{}",
        error.message
    );
}

#[test]
fn pre_migration_backups_keep_the_three_newest_by_time() {
    let file = |name: &str, time: &str| backup_files::BackupFile {
        name: name.to_owned(),
        kind: Kind::PreMigration,
        size: 1,
        time: t(time),
        valid: true,
    };
    let files = [
        file("future", "2099-01-01T00:00:00Z"),
        file("a", "2026-10-01T00:00:00Z"),
        file("b", "2026-10-02T00:00:00Z"),
        file("c", "2026-10-03T00:00:00Z"),
    ];
    let now = t("2026-11-12T00:00:00Z");
    assert_eq!(
        backup_files::to_prune(&files, Kind::PreMigration, 3, now, None),
        ["a"]
    );
    let scheduled: Vec<_> = files
        .iter()
        .map(|f| backup_files::BackupFile {
            kind: Kind::Scheduled,
            ..f.clone()
        })
        .collect();
    assert_eq!(
        backup_files::to_prune(&scheduled, Kind::Scheduled, 3, now, None),
        ["future"]
    );
}
