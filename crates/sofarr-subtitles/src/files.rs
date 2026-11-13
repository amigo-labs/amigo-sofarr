//! Own subtitle files following their video on disk (AC-09; blocking).

use std::{fs, path::Path};

use crate::naming::renamed_sidecar;

/// Gives subtitle files of a renamed or moved video the new video name, in the new
/// video's folder. `file_names` are the files to move (sofarr's own; others stay
/// untouched). A file whose target exists, that is gone or cannot be moved keeps its
/// place and name. Returns `(old name, new name)` of every moved file.
pub fn follow_video(
    old_video: &Path,
    new_video: &Path,
    file_names: &[String],
) -> Vec<(String, String)> {
    let parts = |video: &Path| {
        Some((
            video.parent()?.to_path_buf(),
            video.file_stem()?.to_str()?.to_owned(),
        ))
    };
    let (Some((old_folder, old_stem)), Some((new_folder, new_stem))) =
        (parts(old_video), parts(new_video))
    else {
        return Vec::new();
    };
    let mut moved = Vec::new();
    for name in file_names {
        let Some(new_name) = renamed_sidecar(&old_stem, &new_stem, name) else {
            continue;
        };
        let from = old_folder.join(name);
        let to = new_folder.join(&new_name);
        if from == to || !from.is_file() || to.exists() {
            continue;
        }
        if fs::rename(&from, &to).is_ok() {
            moved.push((name.clone(), new_name));
        }
    }
    moved
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("sofarr-subtitles-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("dir");
        dir
    }

    #[test]
    fn own_subtitles_follow_a_renamed_and_moved_video() {
        let dir = temp("follow");
        fs::create_dir_all(dir.join("Neu")).expect("new folder");
        for name in ["Alt.de.srt", "Alt.en.forced.srt", "Alt.fr.srt"] {
            fs::write(dir.join(name), name).expect("sub");
        }
        fs::write(dir.join("Neu/Neu (2020).en.forced.srt"), b"fremd").expect("taken");
        let names: Vec<String> = ["Alt.de.srt", "Alt.en.forced.srt", "Weg.de.srt"]
            .map(str::to_owned)
            .to_vec();
        let moved = follow_video(
            &dir.join("Alt.mkv"),
            &dir.join("Neu/Neu (2020).mkv"),
            &names,
        );
        assert_eq!(
            moved,
            [("Alt.de.srt".to_owned(), "Neu (2020).de.srt".to_owned())]
        );
        assert_eq!(
            fs::read(dir.join("Neu/Neu (2020).de.srt")).expect("moved"),
            b"Alt.de.srt"
        );
        // A taken target is never overwritten; the subtitle stays where it was.
        assert_eq!(
            fs::read(dir.join("Neu/Neu (2020).en.forced.srt")).expect("kept"),
            b"fremd"
        );
        assert!(dir.join("Alt.en.forced.srt").is_file());
        assert!(dir.join("Alt.fr.srt").is_file(), "not listed: stays");
        let _ = fs::remove_dir_all(&dir);
    }
}
