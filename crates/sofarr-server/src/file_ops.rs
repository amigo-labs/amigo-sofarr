//! File moves of the library pages (spec web-ui-library AC-12, AC-13, AC-15, AC-16):
//! renames and folder moves that never overwrite and move back on a failure, and files
//! that go to the recycle bin or are deleted. Only the file system, no database.

use std::{
    collections::HashSet,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

/// One file or folder to move, with the names the messages use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Move {
    pub from: PathBuf,
    pub to: PathBuf,
    /// How the user knows source and target, e.g. the path below the item's folder.
    pub from_label: String,
    pub to_label: String,
}

/// Why moves did not (all) happen; messages are German and name the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveError {
    /// Checked before anything moved: nothing changed.
    Refused(String),
    /// A move failed midway; the moves before it were moved back where possible.
    /// `at_target` lists the moves whose file is at its target now.
    Failed {
        message: String,
        at_target: Vec<usize>,
    },
}

fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

/// Whether two paths differ only in letter case (same folder, same name otherwise).
pub fn case_only(a: &Path, b: &Path) -> bool {
    a != b && a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
}

/// A free name next to `path` for a two-step case-only rename.
fn temporary(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    (0..)
        .map(|i| path.with_file_name(format!(".{name}.sofarr-{i}")))
        .find(|p| !exists(p))
        .unwrap_or_else(|| path.with_file_name(format!(".{name}.sofarr")))
}

/// One rename; a case-only change goes through a temporary name, so it also works on
/// case-insensitive file systems. The target must not exist (checked again right
/// before, as `rename` would replace a file on Unix).
fn rename_one(m: &Move) -> std::io::Result<()> {
    let only_case = case_only(&m.from, &m.to);
    if !only_case && exists(&m.to) {
        return Err(std::io::Error::new(
            ErrorKind::AlreadyExists,
            "target exists",
        ));
    }
    if let Some(parent) = m.to.parent() {
        fs::create_dir_all(parent)?;
    }
    if only_case {
        let tmp = temporary(&m.to);
        fs::rename(&m.from, &tmp)?;
        if let Err(e) = fs::rename(&tmp, &m.to) {
            let _ = fs::rename(&tmp, &m.from);
            return Err(e);
        }
        return Ok(());
    }
    fs::rename(&m.from, &m.to)
}

/// Checks every move, then carries them out in order. Refused when a source is
/// missing, a target exists (other than the source itself in another letter case) or
/// two moves share a target. When a move fails, the earlier ones are moved back.
pub fn move_all(moves: &[Move]) -> Result<(), MoveError> {
    let mut targets = HashSet::new();
    for m in moves {
        if !targets.insert(m.to.to_string_lossy().to_lowercase()) {
            return Err(MoveError::Refused(format!(
                "Zwei Dateien bekämen denselben Namen „{}“ – nichts wurde verschoben",
                m.to_label
            )));
        }
        if !exists(&m.from) {
            return Err(MoveError::Refused(format!(
                "„{}“ fehlt – nichts wurde verschoben",
                m.from_label
            )));
        }
        if exists(&m.to) && !case_only(&m.from, &m.to) {
            return Err(MoveError::Refused(format!(
                "„{}“ existiert bereits – nichts wurde verschoben oder überschrieben",
                m.to_label
            )));
        }
    }
    for (i, m) in moves.iter().enumerate() {
        if let Err(e) = rename_one(m) {
            tracing::warn!(from = %m.from.display(), to = %m.to.display(), error = %e, "move failed");
            let mut at_target = Vec::new();
            for (j, done) in moves[..i].iter().enumerate().rev() {
                let back = Move {
                    from: done.to.clone(),
                    to: done.from.clone(),
                    from_label: done.to_label.clone(),
                    to_label: done.from_label.clone(),
                };
                if let Err(e) = rename_one(&back) {
                    tracing::warn!(file = %done.to.display(), error = %e, "move back failed");
                    at_target.push(j);
                }
            }
            at_target.sort_unstable();
            return Err(MoveError::Failed {
                message: format!(
                    "„{}“ ließ sich nicht nach „{}“ verschieben; bereits verschobene Dateien \
                     wurden zurückverschoben",
                    m.from_label, m.to_label
                ),
                at_target,
            });
        }
    }
    Ok(())
}

/// Every file below a folder (symbolic links count as files and are not followed),
/// with its path below the folder, `/`-separated; sorted.
pub fn files_below(folder: &Path) -> std::io::Result<Vec<(PathBuf, String)>> {
    let mut out = Vec::new();
    if !folder.is_dir() {
        return Ok(out);
    }
    let mut stack = vec![folder.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                stack.push(path);
            } else {
                let relative = crate::importer::relative_path(folder, &path);
                out.push((path, relative));
            }
        }
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(out)
}

/// A name in the recycle bin that is not taken yet: `name`, `name (1)`, …
fn free_in_bin(target: &Path) -> PathBuf {
    if !exists(target) {
        return target.to_path_buf();
    }
    let stem = target
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    let extension = target
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    (1..)
        .map(|i| target.with_file_name(format!("{stem} ({i}){extension}")))
        .find(|p| !exists(p))
        .unwrap_or_else(|| target.to_path_buf())
}

/// Moves a file; across file systems it is copied and the source removed.
fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    match fs::rename(from, to) {
        Err(e) if e.kind() == ErrorKind::CrossesDevices => {
            fs::copy(from, to)?;
            fs::remove_file(from)
        }
        other => other,
    }
}

/// Removes the files of an item: into `bin/<folder name>/<path below the folder>` or,
/// without a recycle bin, deleted. Stops at the first failure; returns the indices of
/// the files that went and, on a failure, the German message naming the file.
pub fn remove_files(
    files: &[(PathBuf, String)],
    bin: Option<&Path>,
    folder_name: &str,
) -> (Vec<usize>, Option<String>) {
    let mut gone = Vec::new();
    for (i, (path, label)) in files.iter().enumerate() {
        let result = match bin {
            Some(bin) => {
                let target = free_in_bin(&bin.join(folder_name).join(label));
                target
                    .parent()
                    .map_or(Ok(()), fs::create_dir_all)
                    .and_then(|()| move_file(path, &target))
            }
            None => fs::remove_file(path),
        };
        match result {
            Ok(()) => gone.push(i),
            Err(e) => {
                tracing::warn!(file = %path.display(), error = %e, "file not removed");
                let what = if bin.is_some() {
                    "ließ sich nicht in den Papierkorb verschieben"
                } else {
                    "ließ sich nicht löschen"
                };
                return (gone, Some(format!("„{label}“ {what}")));
            }
        }
    }
    (gone, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sofarr-ops-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("dir");
        dir
    }

    fn mv(dir: &Path, from: &str, to: &str) -> Move {
        Move {
            from: dir.join(from),
            to: dir.join(to),
            from_label: from.to_owned(),
            to_label: to.to_owned(),
        }
    }

    #[test]
    fn renames_and_refuses_before_moving() {
        let dir = temp("rename");
        fs::write(dir.join("a.mkv"), b"a").expect("a");
        fs::write(dir.join("b.mkv"), b"b").expect("b");
        fs::write(dir.join("taken.mkv"), b"t").expect("taken");
        assert_eq!(
            move_all(&[mv(&dir, "a.mkv", "x.mkv"), mv(&dir, "b.mkv", "taken.mkv")]),
            Err(MoveError::Refused(
                "„taken.mkv“ existiert bereits – nichts wurde verschoben oder überschrieben"
                    .to_owned()
            ))
        );
        assert!(dir.join("a.mkv").exists(), "nothing moved");
        assert_eq!(
            move_all(&[mv(&dir, "a.mkv", "x.mkv"), mv(&dir, "b.mkv", "X.mkv")]),
            Err(MoveError::Refused(
                "Zwei Dateien bekämen denselben Namen „X.mkv“ – nichts wurde verschoben".to_owned()
            ))
        );
        assert!(matches!(
            move_all(&[mv(&dir, "fehlt.mkv", "y.mkv")]),
            Err(MoveError::Refused(m)) if m.contains("fehlt.mkv")
        ));
        move_all(&[
            mv(&dir, "a.mkv", "Staffel 01/x.mkv"),
            mv(&dir, "b.mkv", "B.mkv"),
        ])
        .expect("moves");
        assert_eq!(fs::read(dir.join("Staffel 01/x.mkv")).expect("x"), b"a");
        let names: Vec<String> = fs::read_dir(&dir)
            .expect("list")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"B.mkv".to_owned()), "{names:?}");
        assert!(!names.contains(&"b.mkv".to_owned()), "{names:?}");
        assert_eq!(fs::read(dir.join("taken.mkv")).expect("t"), b"t");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failure_midway_moves_back() {
        let dir = temp("midway");
        fs::write(dir.join("a.mkv"), b"a").expect("a");
        fs::write(dir.join("b.mkv"), b"b").expect("b");
        // A file where the second target's folder would be: creating it fails.
        fs::write(dir.join("Staffel 02"), b"x").expect("blocker");
        let result = move_all(&[
            mv(&dir, "a.mkv", "Staffel 01/a.mkv"),
            mv(&dir, "b.mkv", "Staffel 02/b.mkv"),
        ]);
        assert_eq!(
            result,
            Err(MoveError::Failed {
                message: "„b.mkv“ ließ sich nicht nach „Staffel 02/b.mkv“ verschieben; bereits \
                          verschobene Dateien wurden zurückverschoben"
                    .to_owned(),
                at_target: vec![],
            })
        );
        assert!(dir.join("a.mkv").exists(), "moved back");
        assert!(dir.join("b.mkv").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn removes_into_the_recycle_bin_or_deletes() {
        let dir = temp("remove");
        let item = dir.join("Insel");
        fs::create_dir_all(item.join("Staffel 01")).expect("s1");
        fs::create_dir_all(item.join("Staffel 02")).expect("s2");
        fs::write(item.join("Staffel 01/e1.mkv"), b"1").expect("e1");
        fs::write(item.join("Staffel 02/e2.mkv"), b"2").expect("e2");
        fs::write(item.join("tvshow.nfo"), b"n").expect("nfo");
        let files = files_below(&item).expect("files");
        let labels: Vec<&str> = files.iter().map(|(_, l)| l.as_str()).collect();
        assert_eq!(
            labels,
            ["Staffel 01/e1.mkv", "Staffel 02/e2.mkv", "tvshow.nfo"]
        );
        let bin = dir.join("bin");
        // Something in the bin blocks the second season folder.
        fs::create_dir_all(bin.join("Insel")).expect("bin");
        fs::write(bin.join("Insel/Staffel 02"), b"x").expect("blocker");
        let (gone, error) = remove_files(&files, Some(&bin), "Insel");
        assert_eq!(gone, [0]);
        assert_eq!(
            error.as_deref(),
            Some("„Staffel 02/e2.mkv“ ließ sich nicht in den Papierkorb verschieben")
        );
        assert!(bin.join("Insel/Staffel 01/e1.mkv").exists());
        assert!(item.join("Staffel 02/e2.mkv").exists());
        fs::remove_file(bin.join("Insel/Staffel 02")).expect("unblock");
        fs::write(item.join("Staffel 01/e1.mkv"), b"again").expect("again");
        let files = files_below(&item).expect("files");
        let (gone, error) = remove_files(&files, Some(&bin), "Insel");
        assert_eq!((gone.len(), error), (3, None));
        assert!(
            bin.join("Insel/Staffel 01/e1 (1).mkv").exists(),
            "nothing overwritten"
        );
        fs::write(item.join("x.mkv"), b"x").expect("x");
        let (gone, error) = remove_files(&files_below(&item).expect("files"), None, "Insel");
        assert_eq!((gone.len(), error), (1, None));
        assert!(!item.join("x.mkv").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
