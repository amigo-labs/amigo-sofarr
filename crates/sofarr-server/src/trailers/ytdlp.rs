//! yt-dlp as the trailer downloader (spec trailers AC-03, AC-04, AC-12, AC-13): started
//! without a shell, with fixed arguments and a checked YouTube video id, stopped after the
//! time limit or when its output grows beyond the size limit.

use std::{
    ffi::OsString,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

/// Longest a trailer download may take (AC-04).
pub const TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// Largest trailer sofarr keeps, in MiB (AC-04).
pub const MAX_MB: u64 = 500;
/// How long `--version` may take.
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
/// How often a running download is looked at.
const POLL: Duration = Duration::from_millis(200);
/// The program yt-dlp merges and remuxes with; found on `PATH` like yt-dlp does.
const FFMPEG: &str = "ffmpeg";

/// A YouTube video id: 11 characters of `A-Z a-z 0-9 _ -` (AC-13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoId(String);

impl VideoId {
    pub fn parse(raw: &str) -> Option<Self> {
        (raw.len() == 11
            && raw
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'))
        .then(|| Self(raw.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The video's page on YouTube.
    pub fn url(&self) -> String {
        format!("https://www.youtube.com/watch?v={}", self.0)
    }
}

/// When a running program is stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub timeout: Duration,
    /// Most bytes the watched folder may hold.
    pub max_bytes: u64,
}

impl Limits {
    /// The limits of AC-04.
    pub fn trailer() -> Self {
        Self {
            timeout: TIMEOUT,
            max_bytes: MAX_MB * 1024 * 1024,
        }
    }
}

/// Downloads a trailer; the seam between the trailer logic and yt-dlp. Both calls block.
pub trait Downloader: Send + Sync {
    /// Downloads the video into the empty folder `dir`; returns the video file it wrote,
    /// or a German reason.
    fn download(&self, id: &VideoId, dir: &Path) -> Result<PathBuf, String>;

    /// Whether the tools can be run; `Err` holds a German reason (AC-12).
    fn check(&self) -> Result<(), String>;
}

/// The real downloader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YtDlp {
    pub program: PathBuf,
    pub max_height: u32,
    pub limits: Limits,
}

impl YtDlp {
    /// The whole command line after the program: fixed options, the output folder and
    /// the video's URL after `--`, so nothing from outside can become an option.
    pub fn arguments(&self, id: &VideoId, out: &Path) -> Vec<OsString> {
        let height = self.max_height;
        let fixed = [
            "--ignore-config".to_owned(),
            "--no-playlist".to_owned(),
            "--no-progress".to_owned(),
            "--no-cache-dir".to_owned(),
            "--no-mtime".to_owned(),
            "--socket-timeout".to_owned(),
            "30".to_owned(),
            "--format".to_owned(),
            format!("bv*[height<={height}]+ba/b[height<={height}]"),
            "--merge-output-format".to_owned(),
            "mkv".to_owned(),
            "--remux-video".to_owned(),
            "mkv".to_owned(),
            "--max-filesize".to_owned(),
            format!("{}M", self.limits.max_bytes / (1024 * 1024)),
            "--output".to_owned(),
            "trailer.%(ext)s".to_owned(),
            "--paths".to_owned(),
        ];
        let mut args: Vec<OsString> = fixed.into_iter().map(OsString::from).collect();
        args.push(out.as_os_str().to_owned());
        args.push("--".into());
        args.push(id.url().into());
        args
    }
}

impl Downloader for YtDlp {
    fn download(&self, id: &VideoId, dir: &Path) -> Result<PathBuf, String> {
        let out = dir.join("out");
        fs::create_dir_all(&out).map_err(|e| format!("Arbeitsordner nicht anlegbar: {e}"))?;
        let log_path = dir.join("yt-dlp.log");
        let log = fs::File::create(&log_path)
            .map_err(|e| format!("Protokolldatei nicht anlegbar: {e}"))?;
        let mut command = Command::new(&self.program);
        command
            .args(self.arguments(id, &out))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log);
        let status =
            run_limited(command, Some(&out), self.limits).map_err(|e| format!("yt-dlp: {e}"))?;
        let stderr = fs::read_to_string(&log_path).unwrap_or_default();
        finished(status.success(), &stderr, &out, self.limits.max_bytes)
    }

    fn check(&self) -> Result<(), String> {
        runs(&self.program, "--version", "yt-dlp")?;
        runs(Path::new(FFMPEG), "-version", "ffmpeg")
    }
}

/// Whether `program arg` runs and succeeds.
fn runs(program: &Path, arg: &str, name: &str) -> Result<(), String> {
    let mut command = Command::new(program);
    command
        .arg(arg)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let limits = Limits {
        timeout: CHECK_TIMEOUT,
        max_bytes: u64::MAX,
    };
    match run_limited(command, None, limits) {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!(
            "{name} ({}) meldet einen Fehler ({status})",
            program.display()
        )),
        Err(e) => Err(format!("{name} ({}) nicht nutzbar: {e}", program.display())),
    }
}

/// Bytes of all files below `dir` (symbolic links are not followed).
fn folder_size(dir: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(t) if t.is_dir() => folder_size(&entry.path()),
            Ok(t) if t.is_file() => entry.metadata().map_or(0, |m| m.len()),
            _ => 0,
        })
        .sum()
}

fn duration_text(d: Duration) -> String {
    let secs = d.as_secs();
    if secs >= 60 && secs.is_multiple_of(60) {
        format!("{} Minuten", secs / 60)
    } else {
        format!("{secs} Sekunden")
    }
}

/// Runs a command (no shell) and waits for it; stops it when it runs longer than the
/// time limit or `watch` grows beyond the size limit. Errors are German.
pub fn run_limited(
    mut command: Command,
    watch: Option<&Path>,
    limits: Limits,
) -> Result<ExitStatus, String> {
    let mut child = command.spawn().map_err(|e| match e.kind() {
        ErrorKind::NotFound => "Programm nicht gefunden".to_owned(),
        _ => format!("nicht startbar: {e}"),
    })?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("Zustand nicht lesbar: {e}"));
            }
        }
        let stop = if started.elapsed() >= limits.timeout {
            Some(format!(
                "abgebrochen nach mehr als {}",
                duration_text(limits.timeout)
            ))
        } else if watch.is_some_and(|dir| folder_size(dir) > limits.max_bytes) {
            Some(format!(
                "abgebrochen: größer als {} MB",
                limits.max_bytes / (1024 * 1024)
            ))
        } else {
            None
        };
        if let Some(reason) = stop {
            let _ = child.kill();
            let _ = child.wait();
            return Err(reason);
        }
        std::thread::sleep(POLL);
    }
}

/// yt-dlp's reason for a failure: its last `ERROR:` line, else its last line.
pub fn reason(stderr: &str) -> String {
    let lines: Vec<&str> = stderr
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let line = lines
        .iter()
        .rev()
        .find_map(|l| l.strip_prefix("ERROR:"))
        .or_else(|| lines.last().copied())
        .map_or("ohne Meldung fehlgeschlagen", str::trim);
    let short: String = line.chars().take(300).collect();
    format!("yt-dlp: {short}")
}

/// The result of a yt-dlp run that ended by itself: the one MKV file it wrote within the
/// size limit, or why not.
pub fn finished(
    success: bool,
    stderr: &str,
    out: &Path,
    max_bytes: u64,
) -> Result<PathBuf, String> {
    if !success {
        return Err(reason(stderr));
    }
    let files: Vec<PathBuf> = fs::read_dir(out)
        .map_err(|e| format!("Arbeitsordner nicht lesbar: {e}"))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    let mkv: Vec<&PathBuf> = files
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("mkv")))
        .collect();
    let [file] = mkv.as_slice() else {
        // `--max-filesize` makes yt-dlp skip a too large video without an error.
        return Err(if files.is_empty() {
            "yt-dlp hat keine Datei geschrieben (zu groß oder nicht verfügbar)".to_owned()
        } else {
            "yt-dlp hat keine einzelne MKV-Datei geschrieben".to_owned()
        });
    };
    let size = fs::metadata(file).map_or(0, |m| m.len());
    if size > max_bytes {
        return Err(format!("größer als {} MB", max_bytes / (1024 * 1024)));
    }
    Ok((*file).clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Set in the environment of the fake program (see [`fake_program`]).
    const FAKE: &str = "SOFARR_FAKE_PROGRAM";
    const FAKE_DIR: &str = "SOFARR_FAKE_DIR";

    /// Not a test of its own: the fake program the limit tests start. They run this test
    /// binary again with only this function selected and `FAKE` set; without it, it
    /// does nothing.
    #[test]
    fn fake_program() {
        let Ok(mode) = std::env::var(FAKE) else {
            return;
        };
        let dir = std::env::var_os(FAKE_DIR).map(PathBuf::from);
        let write = |name: &str, bytes: usize| {
            if let Some(dir) = &dir {
                let _ = fs::write(dir.join(name), vec![0u8; bytes]);
            }
        };
        match mode.as_str() {
            "hang" => std::thread::sleep(Duration::from_secs(120)),
            "grow" => {
                write("trailer.f137.mp4.part", 3 * 1024 * 1024);
                std::thread::sleep(Duration::from_secs(120));
            }
            "ok" => write("trailer.mkv", 1024),
            "fail" => {
                eprintln!("ERROR: [youtube] abcdefghijk: Video unavailable");
                std::process::exit(1);
            }
            _ => {}
        }
        std::process::exit(0);
    }

    fn fake(mode: &str, dir: &Path) -> Command {
        let mut command = Command::new(std::env::current_exe().expect("test binary"));
        command
            .args([
                "--exact",
                "trailers::ytdlp::tests::fake_program",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(FAKE, mode)
            .env(FAKE_DIR, dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sofarr-ytdlp-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn ytdlp() -> YtDlp {
        YtDlp {
            program: PathBuf::from("yt-dlp"),
            max_height: 1080,
            limits: Limits::trailer(),
        }
    }

    #[test]
    fn video_ids_are_checked() {
        for good in ["dQw4w9WgXcQ", "-abc_DEF123", "___________"] {
            assert_eq!(
                VideoId::parse(good).map(|id| id.as_str().to_owned()),
                Some(good.to_owned())
            );
        }
        for bad in [
            "",
            "dQw4w9WgXc",
            "dQw4w9WgXcQQ",
            "dQw4w9WgX;Q",
            "dQw4w9 WgXQ",
            "dQw4w9WgX&Q",
            "--exec=rmQQ",
            "dQw4w9WgXcä",
            "../../../aa",
        ] {
            assert_eq!(VideoId::parse(bad), None, "{bad}");
        }
        let id = VideoId::parse("dQw4w9WgXcQ").expect("id");
        assert_eq!(id.url(), "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    }

    #[test]
    fn the_command_line_is_fixed() {
        let id = VideoId::parse("-abc_DEF123").expect("id");
        let out = PathBuf::from("/config/cache/trailers/movie-1/out");
        let args: Vec<String> = ytdlp()
            .arguments(&id, &out)
            .into_iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "--ignore-config",
                "--no-playlist",
                "--no-progress",
                "--no-cache-dir",
                "--no-mtime",
                "--socket-timeout",
                "30",
                "--format",
                "bv*[height<=1080]+ba/b[height<=1080]",
                "--merge-output-format",
                "mkv",
                "--remux-video",
                "mkv",
                "--max-filesize",
                "500M",
                "--output",
                "trailer.%(ext)s",
                "--paths",
                &out.to_string_lossy(),
                "--",
                "https://www.youtube.com/watch?v=-abc_DEF123",
            ]
        );
        let low = YtDlp {
            max_height: 720,
            ..ytdlp()
        };
        let args = low.arguments(&id, &out);
        assert!(args.contains(&OsString::from("bv*[height<=720]+ba/b[height<=720]")));
    }

    #[test]
    fn a_download_that_takes_too_long_is_stopped() {
        let dir = temp("hang");
        let limits = Limits {
            timeout: Duration::from_secs(2),
            max_bytes: u64::MAX,
        };
        let started = Instant::now();
        let result = run_limited(fake("hang", &dir), Some(&dir), limits);
        assert_eq!(
            result,
            Err("abgebrochen nach mehr als 2 Sekunden".to_owned())
        );
        assert!(started.elapsed() < Duration::from_secs(60));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_download_that_grows_too_large_is_stopped() {
        let dir = temp("grow");
        let limits = Limits {
            timeout: Duration::from_secs(60),
            max_bytes: 2 * 1024 * 1024,
        };
        let started = Instant::now();
        let result = run_limited(fake("grow", &dir), Some(&dir), limits);
        assert_eq!(result, Err("abgebrochen: größer als 2 MB".to_owned()));
        assert!(started.elapsed() < Duration::from_secs(60));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_finished_program_reports_its_exit() {
        let dir = temp("exit");
        let limits = Limits {
            timeout: Duration::from_secs(60),
            max_bytes: 2 * 1024 * 1024,
        };
        let ok = run_limited(fake("ok", &dir), Some(&dir), limits).expect("ran");
        assert!(ok.success());
        assert_eq!(
            finished(true, "", &dir, limits.max_bytes),
            Ok(dir.join("trailer.mkv"))
        );
        let failed = run_limited(fake("fail", &dir), Some(&dir), limits).expect("ran");
        assert!(!failed.success());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn results_are_one_mkv_within_the_limit() {
        let dir = temp("finished");
        assert_eq!(
            finished(
                false,
                "[youtube] x\nWARNING: slow\nERROR: [youtube] abcdefghijk: Private video\n",
                &dir,
                100
            ),
            Err("yt-dlp: [youtube] abcdefghijk: Private video".to_owned())
        );
        assert_eq!(
            finished(false, "something odd\n", &dir, 100),
            Err("yt-dlp: something odd".to_owned())
        );
        assert_eq!(
            finished(false, "", &dir, 100),
            Err("yt-dlp: ohne Meldung fehlgeschlagen".to_owned())
        );
        assert!(finished(true, "", &dir, 100).is_err_and(|e| e.contains("keine Datei")));
        fs::write(dir.join("trailer.webm"), [0u8; 10]).expect("write");
        assert!(finished(true, "", &dir, 100).is_err_and(|e| e.contains("keine einzelne MKV")));
        fs::write(dir.join("trailer.mkv"), [0u8; 10]).expect("write");
        assert_eq!(finished(true, "", &dir, 100), Ok(dir.join("trailer.mkv")));
        assert_eq!(
            finished(true, "", &dir, 9),
            Err("größer als 0 MB".to_owned())
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_tools_are_reported() {
        let missing = YtDlp {
            program: std::env::temp_dir().join("sofarr-no-such-yt-dlp"),
            ..ytdlp()
        };
        let error = missing.check().expect_err("missing yt-dlp");
        assert!(error.starts_with("yt-dlp ("), "{error}");
        assert!(error.contains("nicht gefunden"), "{error}");
        let dir = temp("missing");
        assert!(
            missing
                .download(&VideoId::parse("dQw4w9WgXcQ").expect("id"), &dir)
                .is_err_and(|e| e.contains("nicht gefunden"))
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
