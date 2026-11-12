//! Backup files (spec backups): names, the backup folder, writing, pruning, the pending
//! restore marker, the server lock and putting a backup in place. The jobs and the API
//! live in `backups.rs`.

use std::{
    fs,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
};

use jiff::Timestamp;
use serde::Serialize;

/// Pre-migration backups kept (spec backups: three steps back).
pub const PRE_MIGRATION_KEEP: usize = 3;

/// `.tmp` files older than this are leftovers of a crash.
pub const STALE_TMP_HOURS: i64 = 24;

const TIME_FORMAT: &str = "%Y-%m-%dT%H%M%SZ";
/// The same without the `Z`, parsed as a civil time in UTC.
const CIVIL_FORMAT: &str = "%Y-%m-%dT%H%M%S";
/// Length of a formatted time, e.g. `2026-11-12T030000Z`.
const TIME_LEN: usize = 18;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Scheduled,
    Manual,
    PreMigration,
    Other,
}

/// A backup in the folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BackupFile {
    pub name: String,
    pub kind: Kind,
    pub size: u64,
    #[serde(serialize_with = "as_text")]
    pub time: Timestamp,
    pub valid: bool,
}

/// A timestamp as RFC 3339 text in JSON answers.
pub fn as_text<S: serde::Serializer>(time: &Timestamp, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(time)
}

fn format_time(t: Timestamp) -> String {
    t.strftime(TIME_FORMAT).to_string()
}

fn parse_time(s: &str) -> Option<Timestamp> {
    let civil = jiff::civil::DateTime::strptime(CIVIL_FORMAT, s.strip_suffix('Z')?).ok()?;
    civil
        .to_zoned(jiff::tz::TimeZone::UTC)
        .ok()
        .map(|z| z.timestamp())
}

/// The file name of a backup; `n` > 1 adds the suffix for a second backup in the same
/// second.
pub fn file_name(kind: Kind, time: Timestamp, migration: Option<i64>, n: u32) -> String {
    let time = format_time(time);
    let suffix = if n > 1 {
        format!("-{n}")
    } else {
        String::new()
    };
    match kind {
        Kind::Manual => format!("sofarr-manual-{time}{suffix}.db"),
        Kind::PreMigration => format!(
            "sofarr-pre-migrate-{:04}-{time}{suffix}.db",
            migration.unwrap_or(0)
        ),
        Kind::Scheduled | Kind::Other => format!("sofarr-{time}{suffix}.db"),
    }
}

/// Kind and backup time from a name; `None` for names sofarr does not write.
pub fn parse_name(name: &str) -> Option<(Kind, Timestamp)> {
    let rest = name.strip_prefix("sofarr-")?.strip_suffix(".db")?;
    let (kind, rest) = if let Some(r) = rest.strip_prefix("manual-") {
        (Kind::Manual, r)
    } else if let Some(r) = rest.strip_prefix("pre-migrate-") {
        let (version, r) = r.split_once('-')?;
        if version.is_empty() || !version.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        (Kind::PreMigration, r)
    } else {
        (Kind::Scheduled, rest)
    };
    let time = parse_time(rest.get(..TIME_LEN)?)?;
    let suffix = &rest[TIME_LEN..];
    let ok = suffix.is_empty()
        || suffix
            .strip_prefix('-')
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
    ok.then_some((kind, time))
}

/// Whether `name` is a plain file name inside the folder (no separators, no `..`).
pub fn is_plain_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\', ':'])
        && Path::new(name).file_name().and_then(|n| n.to_str()) == Some(name)
}

/// Reserves a fresh backup name by creating the file exclusively; the next suffix is
/// tried when the name exists (another backup in the same second, maybe in another
/// process).
pub fn reserve(
    dir: &Path,
    kind: Kind,
    time: Timestamp,
    migration: Option<i64>,
) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    for n in 1..=1000 {
        let path = dir.join(file_name(kind, time, migration, n));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(_) => return Ok(path),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(
        ErrorKind::AlreadyExists,
        "no free backup name",
    ))
}

/// The `.tmp` file a backup is written to before it takes its name.
pub fn tmp_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".tmp");
    PathBuf::from(name)
}

/// Removes `.tmp` files older than a day (a younger one may belong to a running backup).
pub fn remove_stale_tmp(dir: &Path, now: Timestamp) -> io::Result<usize> {
    let mut removed = 0;
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("tmp") {
            continue;
        }
        let modified = entry.metadata()?.modified()?;
        let old = Timestamp::try_from(modified)
            .is_ok_and(|t| now - jiff::SignedDuration::from_hours(STALE_TMP_HOURS) > t);
        if old {
            fs::remove_file(&path)?;
            removed += 1;
        }
    }
    Ok(removed)
}

/// Every sofarr database in the folder, newest first; a missing folder has none.
pub async fn list(dir: &Path) -> io::Result<Vec<BackupFile>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut candidates = Vec::new();
    for entry in entries {
        let entry = entry?;
        let meta = entry.metadata()?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !meta.is_file() || name.ends_with(".tmp") {
            continue;
        }
        candidates.push((name, entry.path(), meta));
    }
    let mut out = Vec::new();
    for (name, path, meta) in candidates {
        let inspection = sofarr_db::inspect(&path).await;
        if !inspection.sofarr {
            continue;
        }
        let (kind, time) = parse_name(&name).unwrap_or_else(|| {
            let mtime = meta
                .modified()
                .ok()
                .and_then(|m| Timestamp::try_from(m).ok())
                .unwrap_or(Timestamp::UNIX_EPOCH);
            (Kind::Other, mtime)
        });
        out.push(BackupFile {
            name,
            kind,
            size: meta.len(),
            time,
            valid: inspection.valid,
        });
    }
    out.sort_by(|a, b| b.time.cmp(&a.time).then_with(|| b.name.cmp(&a.name)));
    Ok(out)
}

/// The backups of `kind` to delete so that `keep` remain: those with a backup time in the
/// future first, then the oldest; `protected` (the pending restore) is never deleted.
pub fn to_prune(
    files: &[BackupFile],
    kind: Kind,
    keep: usize,
    now: Timestamp,
    protected: Option<&str>,
) -> Vec<String> {
    let mut of_kind: Vec<&BackupFile> = files.iter().filter(|f| f.kind == kind).collect();
    let excess = of_kind.len().saturating_sub(keep);
    // Future ones first, then by time ascending.
    of_kind.sort_by_key(|f| (f.time <= now, f.time));
    of_kind
        .into_iter()
        .filter(|f| Some(f.name.as_str()) != protected)
        .take(excess)
        .map(|f| f.name.clone())
        .collect()
}

/// The marker file of the pending restore, next to the database.
pub fn marker_path(db: &Path) -> PathBuf {
    with_suffix(db, ".restore")
}

/// `<path><suffix>` (appended to the file name).
pub fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// The pending restore: the backup name in the marker file.
pub fn read_marker(db: &Path) -> io::Result<Option<String>> {
    match fs::read_to_string(marker_path(db)) {
        Ok(raw) => Ok(Some(raw.trim().to_owned())),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Records the pending restore (written aside, then renamed: never half a marker).
pub fn write_marker(db: &Path, name: &str) -> io::Result<()> {
    let marker = marker_path(db);
    let tmp = with_suffix(&marker, ".tmp");
    fs::write(&tmp, name)
        .and_then(|()| fs::rename(&tmp, &marker))
        .inspect_err(|_| {
            let _ = fs::remove_file(&tmp);
        })
}

pub fn clear_marker(db: &Path) -> io::Result<()> {
    match fs::remove_file(marker_path(db)) {
        Err(e) if e.kind() != ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// The exclusive lock a running server holds on `<db>.lock`.
#[derive(Debug)]
pub struct ServerLock {
    _file: fs::File,
}

/// Takes the server lock; `None` while another process holds it.
pub fn lock_server(db: &Path) -> io::Result<Option<ServerLock>> {
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(with_suffix(db, ".lock"))?;
    match file.try_lock() {
        Ok(()) => Ok(Some(ServerLock { _file: file })),
        Err(fs::TryLockError::WouldBlock) => Ok(None),
        Err(fs::TryLockError::Error(e)) => Err(e),
    }
}

fn journals(path: &Path) -> [PathBuf; 2] {
    [with_suffix(path, "-wal"), with_suffix(path, "-shm")]
}

/// A backup put in place of the database; [`Swap::undo`] puts the previous one back.
#[derive(Debug)]
pub struct Swap {
    db: PathBuf,
    /// Renames done (from, to), in order.
    moved: Vec<(PathBuf, PathBuf)>,
    /// Where the previous database went, if there was one.
    pub replaced: Option<PathBuf>,
}

impl Swap {
    /// Removes the restored database (with any journal files it got) and moves the
    /// previous one back.
    pub fn undo(&self) -> io::Result<()> {
        for path in std::iter::once(self.db.clone()).chain(journals(&self.db)) {
            match fs::remove_file(&path) {
                Err(e) if e.kind() != ErrorKind::NotFound => return Err(e),
                _ => {}
            }
        }
        move_back(&self.moved)
    }
}

fn move_back(moved: &[(PathBuf, PathBuf)]) -> io::Result<()> {
    for (from, to) in moved.iter().rev() {
        fs::rename(to, from)?;
    }
    Ok(())
}

/// A free `<db>.replaced-<time>` name.
fn replaced_name(db: &Path, now: Timestamp) -> PathBuf {
    let base = format!(".replaced-{}", format_time(now));
    let mut n = 1;
    loop {
        let suffix = if n == 1 {
            base.clone()
        } else {
            format!("{base}-{n}")
        };
        let path = with_suffix(db, &suffix);
        if !path.exists() {
            return path;
        }
        n += 1;
    }
}

/// Puts `backup` in place of the database at `db`: copies it next to the database,
/// checks the copy, moves the current database (with its journal files) aside under a
/// `replaced` name and renames the copy into place. On any failure the previous state
/// is restored and the error returned.
pub async fn put_in_place(backup: &Path, db: &Path, now: Timestamp) -> io::Result<Swap> {
    let tmp = with_suffix(db, ".restore-tmp");
    let copy = {
        let (backup, tmp) = (backup.to_path_buf(), tmp.clone());
        tokio::task::spawn_blocking(move || fs::copy(&backup, &tmp))
            .await
            .map_err(io::Error::other)
    };
    if let Err(e) = copy.and_then(|r| r) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    if !sofarr_db::inspect(&tmp).await.valid {
        let _ = fs::remove_file(&tmp);
        return Err(io::Error::other("Kopie der Sicherung ist nicht gültig"));
    }
    let mut moved = Vec::new();
    let replaced = db.exists().then(|| replaced_name(db, now));
    if let Some(replaced) = &replaced {
        let pairs = std::iter::once((db.to_path_buf(), replaced.clone())).chain(
            journals(db)
                .into_iter()
                .zip(journals(replaced))
                .filter(|(from, _)| from.exists()),
        );
        for (from, to) in pairs {
            if let Err(e) = fs::rename(&from, &to) {
                let _ = move_back(&moved);
                let _ = fs::remove_file(&tmp);
                return Err(e);
            }
            moved.push((from, to));
        }
    }
    if let Err(e) = fs::rename(&tmp, db) {
        let _ = move_back(&moved);
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(Swap {
        db: db.to_path_buf(),
        moved,
        replaced,
    })
}
