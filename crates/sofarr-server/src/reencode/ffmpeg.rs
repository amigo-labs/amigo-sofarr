//! ffmpeg with libx265 as the encoder (spec reencode-file AC-03, AC-04, AC-05): started
//! without a shell, with a fixed argument list, at the lowest CPU priority where `nice`
//! exists, and stopped after the time limit or when the result outgrows the size limit.

use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use crate::process::{self, Limits, Stopped};

/// How long a check of the tools may take.
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);

/// One encode.
#[derive(Debug, Clone)]
pub struct Job {
    /// The original (absolute path).
    pub input: PathBuf,
    /// The work file the result is written to (absolute path, must not exist).
    pub output: PathBuf,
    /// The original is MP4/QuickTime: data tracks go, text subtitles become SRT.
    pub mp4: bool,
    pub limits: Limits,
    /// Set when sofarr stops: the encoder is stopped (spec reencode-job AC-06).
    pub cancel: Option<Arc<AtomicBool>>,
}

/// Why an encode gave no result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    /// The result outgrew the size limit while being written.
    TooLarge,
    /// The encoder failed or was stopped (German reason).
    Failed(String),
    /// sofarr stops.
    Cancelled,
}

/// The seam between the re-encode logic and ffmpeg. Both calls block.
pub trait Encoder: Send + Sync {
    /// Whether ffmpeg with libx265 can be run; `Err` holds a German reason.
    fn check(&self) -> Result<(), String>;

    /// Writes the result of `job` to `job.output`.
    fn encode(&self, job: &Job) -> Result<(), EncodeError>;
}

/// The encoder settings of `[reencode]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub crf: u8,
    pub preset: String,
    pub threads: u32,
}

/// The real encoder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ffmpeg {
    pub program: PathBuf,
    pub settings: Settings,
    /// Start it through `nice -n 19`.
    pub nice: bool,
}

/// A path for ffmpeg: with the `file:` protocol, so no part of a file name is read as an
/// option or another protocol.
fn file_url(path: &Path) -> OsString {
    let mut url = OsString::from("file:");
    url.push(path.as_os_str());
    url
}

/// Whether `nice -n 19` can start a program here.
pub fn nice_works() -> bool {
    let mut command = Command::new("nice");
    command
        .args(["-n", "19", "nice"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let limits = Limits {
        timeout: CHECK_TIMEOUT,
        max_bytes: u64::MAX,
    };
    process::run(command, None, limits, None).is_ok_and(|status| status.success())
}

impl Ffmpeg {
    /// ffmpeg on `PATH`, at the lowest priority where `nice` works.
    pub fn new(settings: Settings) -> Self {
        Self {
            program: PathBuf::from("ffmpeg"),
            settings,
            nice: nice_works(),
        }
    }

    /// Everything after the program name.
    pub fn arguments(&self, job: &Job) -> Vec<OsString> {
        let mut args: Vec<OsString> = ["-nostdin", "-hide_banner", "-loglevel", "error", "-n"]
            .into_iter()
            .map(OsString::from)
            .collect();
        args.push("-i".into());
        args.push(file_url(&job.input));
        args.extend(["-map", "0"].map(OsString::from));
        if job.mp4 {
            args.extend(["-map", "-0:d?"].map(OsString::from));
        }
        args.extend(["-c", "copy"].map(OsString::from));
        if job.mp4 {
            args.extend(["-c:s", "srt"].map(OsString::from));
        }
        args.extend(["-c:v:0", "libx265", "-preset"].map(OsString::from));
        args.push(self.settings.preset.clone().into());
        args.push("-crf".into());
        args.push(self.settings.crf.to_string().into());
        args.extend(["-pix_fmt", "yuv420p10le"].map(OsString::from));
        // x265 logs to stderr by itself; only its errors may end up as the reason.
        let mut x265 = "log-level=error".to_owned();
        if self.settings.threads > 0 {
            x265.push_str(&format!(":pools={}", self.settings.threads));
        }
        args.push("-x265-params".into());
        args.push(x265.into());
        args.extend(["-f", "matroska"].map(OsString::from));
        args.push(file_url(&job.output));
        args
    }

    /// The command for `job`, through `nice` where it is used.
    pub fn command(&self, job: &Job) -> Command {
        let mut command = if self.nice {
            let mut nice = Command::new("nice");
            nice.args(["-n", "19"]).arg(&self.program);
            nice
        } else {
            Command::new(&self.program)
        };
        command.args(self.arguments(job));
        command
    }
}

/// ffmpeg's reason for a failure: its last line.
fn reason(stderr: &str) -> String {
    let line = stderr
        .lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .unwrap_or("ohne Meldung fehlgeschlagen");
    let short: String = line.chars().take(300).collect();
    format!("ffmpeg: {short}")
}

impl Encoder for Ffmpeg {
    fn check(&self) -> Result<(), String> {
        let missing = || "ffmpeg mit libx265 nicht gefunden".to_owned();
        let output = Command::new(&self.program)
            .args(["-hide_banner", "-encoders"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .map_err(|_| missing())?;
        let listed = String::from_utf8_lossy(&output.stdout)
            .lines()
            .any(|line| line.split_whitespace().nth(1) == Some("libx265"));
        if output.status.success() && listed {
            Ok(())
        } else {
            Err(missing())
        }
    }

    fn encode(&self, job: &Job) -> Result<(), EncodeError> {
        let log_path = std::env::temp_dir().join(format!(
            "sofarr-reencode-{}-{}.log",
            std::process::id(),
            jiff::Timestamp::now().as_millisecond()
        ));
        let log = fs::File::create(&log_path)
            .map_err(|e| EncodeError::Failed(format!("Protokolldatei nicht anlegbar: {e}")))?;
        let mut command = self.command(job);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log);
        let result = process::run(
            command,
            Some(&job.output),
            job.limits,
            job.cancel.as_deref(),
        );
        let stderr = fs::read_to_string(&log_path).unwrap_or_default();
        let _ = fs::remove_file(&log_path);
        match result {
            Ok(status) if status.success() => Ok(()),
            Ok(_) => Err(EncodeError::Failed(reason(&stderr))),
            Err(Stopped::TooLarge(_)) => Err(EncodeError::TooLarge),
            Err(Stopped::Cancelled) => Err(EncodeError::Cancelled),
            Err(stop) => Err(EncodeError::Failed(format!("ffmpeg: {stop}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ffmpeg(threads: u32, nice: bool) -> Ffmpeg {
        Ffmpeg {
            program: PathBuf::from("ffmpeg"),
            settings: Settings {
                crf: 22,
                preset: "medium".to_owned(),
                threads,
            },
            nice,
        }
    }

    fn job(input: &str, output: &str, mp4: bool) -> Job {
        Job {
            input: PathBuf::from(input),
            output: PathBuf::from(output),
            mp4,
            limits: Limits {
                timeout: Duration::from_secs(3600),
                max_bytes: 1000,
            },
            cancel: None,
        }
    }

    fn strings(args: Vec<OsString>) -> Vec<String> {
        args.into_iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn the_argument_list_is_fixed() {
        // AC-03, AC-04.
        let args = strings(ffmpeg(0, false).arguments(&job(
            "/filme/Film/Film.mkv",
            "/filme/Film/.Film.mkv.reencode-partial",
            false,
        )));
        assert_eq!(
            args,
            [
                "-nostdin",
                "-hide_banner",
                "-loglevel",
                "error",
                "-n",
                "-i",
                "file:/filme/Film/Film.mkv",
                "-map",
                "0",
                "-c",
                "copy",
                "-c:v:0",
                "libx265",
                "-preset",
                "medium",
                "-crf",
                "22",
                "-pix_fmt",
                "yuv420p10le",
                "-x265-params",
                "log-level=error",
                "-f",
                "matroska",
                "file:/filme/Film/.Film.mkv.reencode-partial",
            ]
        );
        // MP4 originals drop data tracks and turn text subtitles into SRT; threads above 0
        // limit the encoder's pools.
        let args = strings(ffmpeg(4, false).arguments(&job("/a/F.mp4", "/a/.F.mp4.p", true)));
        let at = |flag: &str| args.iter().position(|a| a == flag).expect(flag);
        assert_eq!(args[at("-i") + 1], "file:/a/F.mp4");
        assert_eq!(&args[9..13], ["-map", "-0:d?", "-c", "copy"]);
        assert_eq!(args[at("-c:s") + 1], "srt");
        assert_eq!(args[at("-x265-params") + 1], "log-level=error:pools=4");
        assert_eq!(args.last().map(String::as_str), Some("file:/a/.F.mp4.p"));
    }

    #[test]
    fn file_names_stay_one_argument_each() {
        // AC-04: spaces, quotes, `;`, `$(…)`, a leading `-` and a colon stay inside the
        // one argument after `-i` and the last one, behind the `file:` protocol.
        let odd = "/filme/-x; rm $(echo \"a b\") 'c': Teil 2/-x; rm.mp4";
        let out = "/filme/-x; rm $(echo \"a b\") 'c': Teil 2/.-x; rm.mp4.reencode-partial";
        let args = strings(ffmpeg(0, false).arguments(&job(odd, out, true)));
        let input = args.iter().position(|a| a == "-i").expect("-i") + 1;
        assert_eq!(args[input], format!("file:{odd}"));
        assert_eq!(args.last(), Some(&format!("file:{out}")));
        let from_library = args.iter().filter(|a| a.contains("rm")).count();
        assert_eq!(from_library, 2, "the names appear nowhere else");
    }

    #[test]
    fn the_encoder_runs_at_the_lowest_priority_where_nice_exists() {
        // AC-05.
        let with = ffmpeg(0, true).command(&job("/a/F.mkv", "/a/.F.mkv.p", false));
        assert_eq!(with.get_program(), "nice");
        let args: Vec<_> = with
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(&args[..4], ["-n", "19", "ffmpeg", "-nostdin"]);
        let without = ffmpeg(0, false).command(&job("/a/F.mkv", "/a/.F.mkv.p", false));
        assert_eq!(without.get_program(), "ffmpeg");
        // Where `nice` exists, a program started through the prefix really runs at 19:
        // `nice` without a command prints its own niceness.
        if nice_works() {
            let command = Ffmpeg {
                program: PathBuf::from("nice"),
                ..ffmpeg(0, true)
            }
            .command(&job("/a/F.mkv", "/a/.F.mkv.p", false));
            let mut probe = Command::new(command.get_program());
            probe.args(command.get_args().take(3));
            let output = probe.output().expect("nice runs");
            assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "19");
        } else {
            assert!(!Ffmpeg::new(ffmpeg(0, false).settings).nice);
        }
    }

    #[test]
    fn a_missing_encoder_is_reported_in_german() {
        // AC-10 (part): no ffmpeg, or one without libx265.
        let missing = Ffmpeg {
            program: PathBuf::from("/nonexistent/ffmpeg"),
            ..ffmpeg(0, false)
        };
        assert_eq!(
            missing.check(),
            Err("ffmpeg mit libx265 nicht gefunden".to_owned())
        );
        let failed = missing.encode(&job("/nonexistent/a.mkv", "/nonexistent/.a.p", false));
        assert!(
            matches!(failed, Err(EncodeError::Failed(reason)) if reason.starts_with("ffmpeg: "))
        );
        assert_eq!(
            reason("frame=1\n\nConversion failed!\n"),
            "ffmpeg: Conversion failed!"
        );
        assert_eq!(reason(""), "ffmpeg: ohne Meldung fehlgeschlagen");
    }
}
