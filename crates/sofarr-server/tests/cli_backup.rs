//! `amigo-sofarr backup` and `amigo-sofarr restore` (spec backups AC-06, AC-08, AC-16,
//! AC-17).

use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

struct Env {
    root: PathBuf,
    config: PathBuf,
    db: PathBuf,
    backups: PathBuf,
}

impl Env {
    fn new(name: &str, backups: Option<&Path>) -> Self {
        let root = std::env::temp_dir().join(format!("sofarr-cli-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("root");
        let db = root.join("sofarr.db");
        let backups = backups.map_or_else(|| root.join("backups"), Path::to_path_buf);
        let config = root.join("sofarr.toml");
        std::fs::write(
            &config,
            format!(
                "[database]\npath = '{}'\n[backup]\ndir = '{}'\n[log]\nlevel = \"error\"\n",
                db.display(),
                backups.display()
            ),
        )
        .expect("config");
        Self {
            root,
            config,
            db,
            backups,
        }
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_amigo-sofarr"))
            .arg("--config")
            .arg(&self.config)
            .args(args)
            .output()
            .expect("binary runs")
    }

    fn backups(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.backups)
            .map(|e| {
                e.filter_map(Result::ok)
                    .filter_map(|e| e.file_name().to_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    fn replaced(&self) -> usize {
        std::fs::read_dir(&self.root)
            .expect("root")
            .filter_map(Result::ok)
            .filter(|e| {
                e.file_name().to_str().is_some_and(|n| {
                    n.starts_with("sofarr.db.replaced-")
                        && n.ends_with(|c: char| c.is_ascii_digit() || c == 'Z')
                })
            })
            .count()
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn backup_and_restore_through_the_cli() {
    let env = Env::new("backup", None);
    let backup = env.run(&["backup"]);
    assert!(backup.status.success(), "{}", stderr(&backup));
    let names = env.backups();
    assert_eq!(names.len(), 1, "{names:?}");
    assert!(names[0].starts_with("sofarr-manual-"));
    let stdout = String::from_utf8_lossy(&backup.stdout);
    assert!(stdout.contains(&names[0]), "{stdout}");

    // A running server holds the lock: nothing changes.
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(env.root.join("sofarr.db.lock"))
        .expect("lock file");
    lock.try_lock().expect("lock");
    let busy = env.run(&["restore", &names[0]]);
    assert!(!busy.status.success());
    assert!(stderr(&busy).contains("Server"), "{}", stderr(&busy));
    lock.unlock().expect("unlock");
    drop(lock);
    assert_eq!(env.replaced(), 0);

    // The current database itself, or a file that is no valid backup.
    let db = env.db.display().to_string();
    let itself = env.run(&["restore", &db]);
    assert!(!itself.status.success());
    assert!(
        stderr(&itself).contains("aktuelle Datenbank"),
        "{}",
        stderr(&itself)
    );
    std::fs::write(env.backups.join("notes.db"), b"no database").expect("notes");
    let invalid = env.run(&["restore", "notes.db"]);
    assert!(!invalid.status.success());
    assert!(
        stderr(&invalid).contains("keine gültige Sicherung"),
        "{}",
        stderr(&invalid)
    );
    let missing = env.run(&["restore", "missing.db"]);
    assert!(!missing.status.success());
    assert_eq!(env.replaced(), 0);

    // Putting it in place fails: the database stays.
    let before = std::fs::read(&env.db).expect("db");
    let blocker = env.root.join("sofarr.db.restore-tmp");
    std::fs::create_dir_all(blocker.join("x")).expect("blocker");
    let failed = env.run(&["restore", &names[0]]);
    assert!(!failed.status.success());
    assert!(
        stderr(&failed).contains("unverändert"),
        "{}",
        stderr(&failed)
    );
    assert_eq!(std::fs::read(&env.db).expect("db"), before);
    std::fs::remove_dir_all(&blocker).expect("unblock");

    let restored = env.run(&["restore", &names[0]]);
    assert!(restored.status.success(), "{}", stderr(&restored));
    assert_eq!(env.replaced(), 1);
    assert_eq!(
        std::fs::read(&env.db).expect("db"),
        std::fs::read(env.backups.join(&names[0])).expect("backup")
    );
}

#[test]
fn a_failed_pre_migration_backup_stops_the_start() {
    let blocked = std::env::temp_dir().join(format!("sofarr-cli-preblock-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&blocked);
    std::fs::write(&blocked, b"a file").expect("file");
    let env = Env::new("premigrate-fail", Some(&blocked.join("backups")));
    tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(sofarr_db::create_at_version(&env.db, 3))
        .expect("old db");
    let output = env.run(&["wanted"]);
    let _ = std::fs::remove_file(&blocked);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("Migrationen nicht ausgeführt"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_failing_cli_backup_exits_with_a_german_message() {
    let blocked = std::env::temp_dir().join(format!("sofarr-cli-blocked-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&blocked);
    std::fs::write(&blocked, b"a file").expect("file");
    let env = Env::new("backup-fail", Some(&blocked.join("backups")));
    let output = env.run(&["backup"]);
    let _ = std::fs::remove_file(&blocked);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("Sicherung"), "{}", stderr(&output));
}
