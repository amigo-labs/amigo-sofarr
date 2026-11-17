//! External programs (yt-dlp, ffmpeg): started without a shell and stopped when they run
//! too long or their output grows too large.

use std::{
    fmt, fs,
    io::ErrorKind,
    path::Path,
    process::{Command, ExitStatus},
    sync::atomic::{AtomicBool, Ordering},
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
    /// It was stopped from outside (sofarr stops).
    Cancelled,
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
            Self::Cancelled => f.write_str("abgebrochen: sofarr wurde beendet"),
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
/// time limit, `watch` grows beyond the size limit, or `cancel` is set.
pub fn run(
    mut command: Command,
    watch: Option<&Path>,
    limits: Limits,
    cancel: Option<&AtomicBool>,
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
        let stop = if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            Some(Stopped::Cancelled)
        } else if started.elapsed() >= limits.timeout {
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
    fn a_set_stop_flag_stops_a_running_program() {
        // Spec reencode-job AC-06: the trailers' fake program in its "hang" mode sleeps two
        // minutes; the flag stops it at the next poll.
        let mut command = Command::new(std::env::current_exe().expect("test binary"));
        command
            .args([
                "--exact",
                "trailers::ytdlp::tests::fake_program",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("SOFARR_FAKE_PROGRAM", "hang")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        let flag = std::sync::Arc::clone(&cancel);
        let setter = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            flag.store(true, Ordering::SeqCst);
        });
        let started = Instant::now();
        let limits = Limits {
            timeout: Duration::from_secs(120),
            max_bytes: u64::MAX,
        };
        assert_eq!(
            run(command, None, limits, Some(&cancel)),
            Err(Stopped::Cancelled)
        );
        assert!(started.elapsed() < Duration::from_secs(10));
        setter.join().expect("setter");
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
        assert_eq!(
            Stopped::Cancelled.to_string(),
            "abgebrochen: sofarr wurde beendet"
        );
    }
}
