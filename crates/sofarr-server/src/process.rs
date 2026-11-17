//! External programs (yt-dlp, ffmpeg): started without a shell and stopped when they run
//! too long or their output grows too large.

use std::{
    fmt, fs,
    io::ErrorKind,
    path::Path,
    process::{Command, ExitStatus},
    time::{Duration, Instant},
};

/// How often a running program is looked at.
const POLL: Duration = Duration::from_millis(200);

/// When a running program is stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub timeout: Duration,
    /// Most bytes the watched file or folder may hold.
    pub max_bytes: u64,
}

/// Why a program did not end by itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stopped {
    /// It could not be started (German reason).
    Start(String),
    /// Its state could not be read (German reason).
    Wait(String),
    /// It ran longer than the time limit.
    Timeout(Duration),
    /// Its output grew beyond the size limit (in bytes).
    TooLarge(u64),
}

fn duration_text(d: Duration) -> String {
    let secs = d.as_secs();
    if secs >= 3600 && secs.is_multiple_of(3600) {
        format!("{} Stunden", secs / 3600)
    } else if secs >= 60 && secs.is_multiple_of(60) {
        format!("{} Minuten", secs / 60)
    } else {
        format!("{secs} Sekunden")
    }
}

impl fmt::Display for Stopped {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start(reason) | Self::Wait(reason) => f.write_str(reason),
            Self::Timeout(limit) => {
                write!(f, "abgebrochen nach mehr als {}", duration_text(*limit))
            }
            Self::TooLarge(bytes) => {
                write!(f, "abgebrochen: größer als {} MB", bytes / (1024 * 1024))
            }
        }
    }
}

/// Bytes of a file, or of all files below a folder (symbolic links are not followed).
pub fn size_of(path: &Path) -> u64 {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() => meta.len(),
        Ok(meta) if meta.is_dir() => fs::read_dir(path).map_or(0, |entries| {
            entries.flatten().map(|entry| size_of(&entry.path())).sum()
        }),
        _ => 0,
    }
}

/// Runs a command (no shell) and waits for it; stops it when it runs longer than the
/// time limit or `watch` grows beyond the size limit.
pub fn run(
    mut command: Command,
    watch: Option<&Path>,
    limits: Limits,
) -> Result<ExitStatus, Stopped> {
    let mut child = command.spawn().map_err(|e| {
        Stopped::Start(match e.kind() {
            ErrorKind::NotFound => "Programm nicht gefunden".to_owned(),
            _ => format!("nicht startbar: {e}"),
        })
    })?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Stopped::Wait(format!("Zustand nicht lesbar: {e}")));
            }
        }
        let stop = if started.elapsed() >= limits.timeout {
            Some(Stopped::Timeout(limits.timeout))
        } else if watch.is_some_and(|path| size_of(path) > limits.max_bytes) {
            Some(Stopped::TooLarge(limits.max_bytes))
        } else {
            None
        };
        if let Some(stop) = stop {
            let _ = child.kill();
            let _ = child.wait();
            return Err(stop);
        }
        std::thread::sleep(POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_of_files_and_folders() {
        let dir = std::env::temp_dir().join(format!("sofarr-process-size-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("inner")).expect("dir");
        fs::write(dir.join("a"), [0u8; 10]).expect("a");
        fs::write(dir.join("inner/b"), [0u8; 5]).expect("b");
        assert_eq!(size_of(&dir.join("a")), 10);
        assert_eq!(size_of(&dir), 15);
        assert_eq!(size_of(&dir.join("missing")), 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reasons_read_in_german() {
        assert_eq!(
            Stopped::Timeout(Duration::from_secs(24 * 3600)).to_string(),
            "abgebrochen nach mehr als 24 Stunden"
        );
        assert_eq!(
            Stopped::Timeout(Duration::from_secs(600)).to_string(),
            "abgebrochen nach mehr als 10 Minuten"
        );
        assert_eq!(
            Stopped::TooLarge(500 * 1024 * 1024).to_string(),
            "abgebrochen: größer als 500 MB"
        );
    }
}
