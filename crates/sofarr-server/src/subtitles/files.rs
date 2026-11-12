//! Subtitle files on disk (blocking: call off the async threads): the subtitle files next
//! to a video, writing an own subtitle without ever overwriting a file (AC-06), and own
//! subtitles leaving with their video (AC-10).

use std::{
    fs,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};

use sofarr_db::OwnSubtitle;

use super::needs::{External, part_language};

/// Folder and file stem of a video.
pub fn video_parts(video: &Path) -> Option<(PathBuf, String)> {
    let folder = video.parent()?.to_path_buf();
    let stem = video.file_stem()?.to_str()?.to_owned();
    Some((folder, stem))
}

/// The subtitle files directly next to a video (subfolders count not), by name; `None`
/// when the video is not there or its folder cannot be read.
pub fn external_subtitles(video: &Path) -> Option<Vec<External>> {
    let (folder, stem) = video_parts(video)?;
    if !video.is_file() {
        return None;
    }
    let entries = fs::read_dir(&folder).ok()?;
    let mut out: Vec<External> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_owned();
            let sidecar = sofarr_subtitles::parse_sidecar(&stem, &name)?;
            Some(External {
                language: sidecar.parts.iter().find_map(|p| part_language(p)),
                file_name: name,
                forced: sidecar.forced,
                hearing_impaired: sidecar.hearing_impaired,
            })
        })
        .collect();
    out.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    Some(out)
}

/// What became of writing a subtitle.
#[derive(Debug, PartialEq, Eq)]
pub enum Written {
    Stored,
    /// Another file has the name already; it stays untouched.
    Taken,
}

/// Writes `text` as `name` next to `video`; never replaces an existing file, and leaves
/// no partial file behind when writing fails.
pub fn write_subtitle(video: &Path, name: &str, text: &str) -> std::io::Result<Written> {
    let folder = video
        .parent()
        .ok_or_else(|| std::io::Error::new(ErrorKind::NotFound, "video without folder"))?;
    let path = folder.join(name);
    let taken = fs::read_dir(folder)?
        .flatten()
        .any(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name));
    if taken {
        return Ok(Written::Taken);
    }
    let mut file = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(file) => file,
        Err(e) if e.kind() == ErrorKind::AlreadyExists => return Ok(Written::Taken),
        Err(e) => return Err(e),
    };
    if let Err(e) = file
        .write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
    {
        drop(file);
        let _ = fs::remove_file(&path);
        return Err(e);
    }
    Ok(Written::Stored)
}

/// Moves the own subtitles of a removed or replaced video out of `folder`: into the
/// recycle bin when one is set, deleted otherwise (like the video). Files already gone
/// count as done; returns the first failure.
pub fn remove_own(
    folder: &Path,
    own: &[OwnSubtitle],
    recycle_bin: Option<&Path>,
) -> std::io::Result<()> {
    for subtitle in own {
        let path = folder.join(&subtitle.file_name);
        if !path.is_file() {
            continue;
        }
        match recycle_bin {
            Some(bin) => {
                fs::create_dir_all(bin)?;
                let target = bin.join(&subtitle.file_name);
                if fs::rename(&path, &target).is_err() {
                    // Another file system: copy, then delete.
                    fs::copy(&path, &target)?;
                    fs::remove_file(&path)?;
                }
            }
            None => fs::remove_file(&path)?,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sofarr_db::{FileRef, SubtitleKind};

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("sofarr-subfiles-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("dir");
        dir
    }

    fn own(id: i64, name: &str) -> OwnSubtitle {
        OwnSubtitle {
            id,
            file: FileRef::Movie(1),
            language: "de".to_owned(),
            kind: SubtitleKind::Normal,
            hearing_impaired: false,
            file_name: name.to_owned(),
            provider_id: "1".to_owned(),
            release: String::new(),
            match_quality: "title".to_owned(),
            created_at: jiff::Timestamp::UNIX_EPOCH,
        }
    }

    #[test]
    fn lists_only_subtitles_of_the_video_next_to_it() {
        let dir = temp("list");
        let video = dir.join("Film (2019).mkv");
        for name in [
            "Film (2019).mkv",
            "Film (2019).de.srt",
            "Film (2019).Englisch.forced.ass",
            "Film (2019).srt",
            "Film (2019).nfo",
            "Anderer.de.srt",
        ] {
            fs::write(dir.join(name), b"x").expect("file");
        }
        fs::create_dir_all(dir.join("Subs")).expect("subs");
        fs::write(dir.join("Subs/Film (2019).fr.srt"), b"x").expect("sub");
        let found = external_subtitles(&video).expect("readable");
        assert_eq!(external_subtitles(&dir.join("Fehlt.mkv")), None);
        let text: Vec<String> = found
            .iter()
            .map(|e| format!("{}={:?}/{}", e.file_name, e.language, e.forced))
            .collect();
        assert_eq!(
            text,
            [
                "Film (2019).Englisch.forced.ass=Some(\"eng\")/true",
                "Film (2019).de.srt=Some(\"ger\")/false",
                "Film (2019).srt=None/false",
            ]
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn writing_never_replaces_a_file() {
        let dir = temp("write");
        let video = dir.join("Film.mkv");
        fs::write(dir.join("Film.DE.srt"), b"fremd").expect("foreign");
        assert_eq!(
            write_subtitle(&video, "Film.de.srt", "neu").expect("write"),
            Written::Taken
        );
        assert_eq!(fs::read(dir.join("Film.DE.srt")).expect("read"), b"fremd");
        assert_eq!(
            write_subtitle(&video, "Film.en.srt", "Grüße").expect("write"),
            Written::Stored
        );
        assert_eq!(
            fs::read_to_string(dir.join("Film.en.srt")).expect("read"),
            "Grüße"
        );
        assert!(write_subtitle(&dir.join("fehlt/Film.mkv"), "Film.en.srt", "x").is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn own_subtitles_leave_with_their_video() {
        let dir = temp("leave");
        for name in ["Neu.de.srt", "Neu.en.forced.srt", "Neu.fr.srt"] {
            fs::write(dir.join(name), name).expect("sub");
        }
        let bin = dir.join("bin");
        let own_subs = [
            own(1, "Neu.de.srt"),
            own(2, "Neu.en.forced.srt"),
            own(3, "Weg.de.srt"),
        ];
        remove_own(&dir, &own_subs[..1], Some(&bin)).expect("recycle");
        assert!(bin.join("Neu.de.srt").is_file());
        assert!(!dir.join("Neu.de.srt").exists());
        remove_own(&dir, &own_subs[1..], None).expect("delete");
        assert!(!dir.join("Neu.en.forced.srt").exists());
        assert!(dir.join("Neu.fr.srt").is_file(), "not own: stays");
        let _ = fs::remove_dir_all(&dir);
    }
}
