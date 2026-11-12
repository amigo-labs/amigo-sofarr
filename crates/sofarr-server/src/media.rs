//! Media info of library files (spec mediainfo): reading at import and disk scan, the
//! naming labels, the API answer.

use std::{
    fs,
    future::Future,
    io::{ErrorKind, Read, Seek},
    path::{Path, PathBuf},
};

use jiff::Timestamp;
use serde::Serialize;
use sofarr_db::{Db, FileRef, StoredMediaInfo};
use sofarr_mediainfo::{Labels, MediaInfo, ReadStatus};

/// Media info read from a file, with the file's size and modification time.
#[derive(Debug, Clone)]
pub struct FileMedia {
    pub info: MediaInfo,
    pub size: u64,
    pub modified: String,
}

fn modified(meta: &fs::Metadata) -> String {
    meta.modified()
        .ok()
        .and_then(|t| Timestamp::try_from(t).ok())
        .map(|t| t.to_string())
        .unwrap_or_default()
}

/// Reads the media info of a file (blocking: run it off the async threads). `None` if
/// the file is gone (the disk scan handles a missing file); a file that cannot be opened
/// or read is `unreadable`. Opens the file read-only, so nothing about it changes.
pub fn read_path(path: &Path) -> Option<FileMedia> {
    let meta = match fs::metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == ErrorKind::NotFound => return None,
        Err(_) => {
            return Some(FileMedia {
                info: sofarr_mediainfo::unreadable(),
                size: 0,
                modified: String::new(),
            });
        }
    };
    let info = match fs::File::open(path) {
        Ok(file) => read_file(file),
        Err(e) if e.kind() == ErrorKind::NotFound => return None,
        Err(_) => sofarr_mediainfo::unreadable(),
    };
    Some(FileMedia {
        info,
        size: meta.len(),
        modified: modified(&meta),
    })
}

/// Reads the media info of an opened file. Unbuffered on purpose: the reader asks for
/// whole elements, and a buffer would refill itself after every seek past a skipped
/// element, reading far more from disk than the read limit allows (AC-05).
pub fn read_file<R: Read + Seek>(mut file: R) -> MediaInfo {
    sofarr_mediainfo::read(&mut file)
}

/// [`read_path`] off the async threads.
pub async fn read(path: PathBuf) -> Option<FileMedia> {
    tokio::task::spawn_blocking(move || read_path(&path))
        .await
        .ok()
        .flatten()
}

/// The stored media info of a file, if any and readable.
pub fn parse(stored: &StoredMediaInfo) -> Option<MediaInfo> {
    serde_json::from_str(&stored.info).ok()
}

/// Stores media info for a file with the size and modification time it was read at:
/// taken before the read, so a file that changes while it is read differs from them at
/// the next scan and is read again.
pub async fn store(db: &Db, file: FileRef, media: &FileMedia) -> anyhow::Result<()> {
    db.put_media_info(
        file,
        &StoredMediaInfo {
            info: serde_json::to_string(&media.info)?,
            size: media.size,
            modified: media.modified.clone(),
        },
        Timestamp::now(),
    )
    .await?;
    Ok(())
}

/// Stores media info read from a download's file for the library file it was just moved
/// or copied to, with that file's size and modification time (they may differ from the
/// source's). If the library file cannot be looked at, size 0 makes the next scan read it.
pub async fn store_moved(
    db: &Db,
    file: FileRef,
    destination: &Path,
    info: &MediaInfo,
) -> anyhow::Result<()> {
    let (size, modified) = match fs::metadata(destination) {
        Ok(meta) => (meta.len(), self::modified(&meta)),
        Err(_) => (0, String::new()),
    };
    let media = FileMedia {
        info: info.clone(),
        size,
        modified,
    };
    store(db, file, &media).await
}

/// Reads and stores a library file's media info unless it is current: an unchanged
/// file (same size and modification time) is not read again, except one that was
/// `unreadable`. Returns whether it was read.
pub async fn refresh(db: &Db, file: FileRef, path: &Path) -> anyhow::Result<bool> {
    refresh_with(db, file, path, read).await
}

/// [`refresh`] with the function that reads the file.
pub async fn refresh_with<F, Fut>(
    db: &Db,
    file: FileRef,
    path: &Path,
    read: F,
) -> anyhow::Result<bool>
where
    F: FnOnce(PathBuf) -> Fut,
    Fut: Future<Output = Option<FileMedia>>,
{
    let meta = match fs::metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
        Err(_) => {
            // Cannot even be looked at: record it as unreadable unless it already is.
            let stored = db.media_info(file).await?;
            if stored.as_ref().and_then(parse).map(|m| m.status) == Some(ReadStatus::Unreadable) {
                return Ok(false);
            }
            let media = FileMedia {
                info: sofarr_mediainfo::unreadable(),
                size: 0,
                modified: String::new(),
            };
            store(db, file, &media).await?;
            return Ok(true);
        }
    };
    if let Some(stored) = db.media_info(file).await? {
        let unreadable = parse(&stored).is_some_and(|m| m.status == ReadStatus::Unreadable);
        if !unreadable && stored.size == meta.len() && stored.modified == modified(&meta) {
            return Ok(false);
        }
    }
    let Some(media) = read(path.to_path_buf()).await else {
        return Ok(false);
    };
    store(db, file, &media).await?;
    Ok(true)
}

/// The parts of the naming tokens a file knows: `{MediaInfo Simple}` video and audio,
/// and `{MediaInfo VideoDynamicRange}` (`Some(None)`: the file is SDR).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NamingParts {
    pub video: Option<String>,
    pub audio: Option<String>,
    pub dynamic_range: Option<Option<String>>,
}

impl NamingParts {
    pub fn of(info: Option<&MediaInfo>) -> Self {
        let Some(info) = info else {
            return Self::default();
        };
        let labels = info.labels();
        Self {
            video: labels.video.clone(),
            audio: labels.audio.clone(),
            dynamic_range: info.video.as_ref().map(|_| labels.dynamic_range.clone()),
        }
    }

    /// `{MediaInfo Simple}`: per part from the file, the rest from the release name.
    pub fn simple(&self, release: &str) -> Option<String> {
        sofarr_core::naming::media_info_simple(
            self.video.as_deref(),
            self.audio.as_deref(),
            release,
        )
    }

    /// `{MediaInfo VideoDynamicRange}`: from the file where its video is known.
    pub fn dynamic_range(&self, release: &str) -> Option<String> {
        sofarr_core::naming::video_dynamic_range(
            self.dynamic_range.as_ref().map(|d| d.as_deref()),
            release,
        )
    }
}

/// The `media_info` of the file endpoints.
#[derive(Debug, Serialize)]
pub struct MediaInfoAnswer {
    #[serde(flatten)]
    pub info: MediaInfo,
    pub labels: Labels,
}

impl MediaInfoAnswer {
    pub fn of(info: MediaInfo) -> Self {
        let labels = info.labels();
        Self { info, labels }
    }
}
