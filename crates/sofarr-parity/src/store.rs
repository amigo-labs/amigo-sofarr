//! Append-only JSONL fixture files, deduplicated by title.

use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::Context;
use serde::Serialize;
use serde_json::Value;

/// One line of a fixture file.
#[derive(Debug, Serialize)]
pub struct Record<'a> {
    pub title: &'a str,
    /// `rss` or `history` – never an indexer name or URL.
    pub origin: &'a str,
    pub collected_at: String,
    pub oracle: Oracle<'a>,
    pub response: Value,
}

#[derive(Debug, Serialize)]
pub struct Oracle<'a> {
    pub app: &'a str,
    pub version: &'a str,
}

pub struct Store {
    path: PathBuf,
    known: HashSet<String>,
    file: File,
}

impl Store {
    /// Opens (or creates) a fixture file. A trailing incomplete line – left behind by an
    /// interrupted write – is cut off.
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Verzeichnis {} nicht anlegbar", parent.display()))?;
        }
        let raw = match fs::read_to_string(path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e).with_context(|| format!("{} nicht lesbar", path.display())),
        };

        let complete = match raw.rfind('\n') {
            Some(i) => &raw[..=i],
            None => "",
        };
        let mut known = HashSet::new();
        for (n, line) in complete.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let value: Value = serde_json::from_str(line)
                .with_context(|| format!("{}:{}: ungültiges JSON", path.display(), n + 1))?;
            let title = value["title"]
                .as_str()
                .with_context(|| format!("{}:{}: `title` fehlt", path.display(), n + 1))?;
            known.insert(title.to_owned());
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("{} nicht schreibbar", path.display()))?;
        if complete.len() < raw.len() {
            file.set_len(complete.len() as u64).with_context(|| {
                format!("{}: unvollständige Zeile nicht entfernbar", path.display())
            })?;
            eprintln!("{}: unvollständige letzte Zeile entfernt", path.display());
        }
        Ok(Self {
            path: path.to_owned(),
            known,
            file,
        })
    }

    pub fn contains(&self, title: &str) -> bool {
        self.known.contains(title)
    }

    pub fn len(&self) -> usize {
        self.known.len()
    }

    pub fn is_empty(&self) -> bool {
        self.known.is_empty()
    }

    /// Appends one record as a single complete line. Returns `false` for a known title.
    pub fn append(&mut self, record: &Record<'_>) -> anyhow::Result<bool> {
        if self.contains(record.title) {
            return Ok(false);
        }
        let mut line = serde_json::to_string(record)?;
        line.push('\n');
        self.file
            .write_all(line.as_bytes())
            .and_then(|()| self.file.flush())
            .with_context(|| format!("{} nicht schreibbar", self.path.display()))?;
        self.known.insert(record.title.to_owned());
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sofarr-parity-store-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join(name);
        let _ = fs::remove_file(&path);
        path
    }

    fn record(title: &str) -> Record<'_> {
        Record {
            title,
            origin: "rss",
            collected_at: "2026-01-01T00:00:00Z".to_owned(),
            oracle: Oracle {
                app: "radarr",
                version: "0.0.0",
            },
            response: serde_json::json!({"ok": true}),
        }
    }

    #[test]
    fn dedupes_within_and_across_runs() {
        let path = temp_file("dedupe.jsonl");
        let mut store = Store::open(&path).expect("open");
        assert!(store.append(&record("A")).expect("append"));
        assert!(!store.append(&record("A")).expect("append"));
        assert!(store.append(&record("B")).expect("append"));
        drop(store);

        let mut store = Store::open(&path).expect("reopen");
        assert_eq!(store.len(), 2);
        assert!(!store.append(&record("B")).expect("append"));
        let raw = fs::read_to_string(&path).expect("read");
        assert_eq!(raw.lines().count(), 2);
        assert!(raw.ends_with('\n'));
    }

    #[test]
    fn line_has_the_documented_shape() {
        let path = temp_file("shape.jsonl");
        let mut store = Store::open(&path).expect("open");
        store.append(&record("Ein Titel")).expect("append");
        let raw = fs::read_to_string(&path).expect("read");
        assert_eq!(
            raw,
            "{\"title\":\"Ein Titel\",\"origin\":\"rss\",\"collected_at\":\"2026-01-01T00:00:00Z\",\
             \"oracle\":{\"app\":\"radarr\",\"version\":\"0.0.0\"},\"response\":{\"ok\":true}}\n"
        );
    }

    #[test]
    fn truncated_last_line_is_dropped() {
        let path = temp_file("truncated.jsonl");
        fs::write(&path, "{\"title\":\"A\"}\n{\"title\":\"B\",\"resp").expect("write");
        let mut store = Store::open(&path).expect("open");
        assert!(store.contains("A"));
        assert!(!store.contains("B"));
        store.append(&record("C")).expect("append");
        let raw = fs::read_to_string(&path).expect("read");
        assert_eq!(raw.lines().count(), 2);
        assert!(
            raw.lines()
                .all(|l| serde_json::from_str::<Value>(l).is_ok())
        );
    }

    #[test]
    fn invalid_json_aborts() {
        let path = temp_file("invalid.jsonl");
        fs::write(&path, "kein json\n").expect("write");
        assert!(Store::open(&path).is_err());
    }
}
