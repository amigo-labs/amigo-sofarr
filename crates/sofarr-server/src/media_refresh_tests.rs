//! When the server reads a library file's media info again (spec mediainfo AC-01, edge
//! case "still being written"), and how much it reads from disk (AC-05).

use std::{
    io::{Cursor, Read, Seek, SeekFrom, Write},
    time::{Duration, SystemTime},
};

use sofarr_db::FileRef;
use sofarr_mediainfo::{READ_LIMIT, ReadStatus};

use crate::{
    media,
    media_tests::{dir, el, mkv, place, stored, text},
    testkit::{Library, app},
};

fn mtime(path: &std::path::Path) -> SystemTime {
    std::fs::metadata(path)
        .expect("meta")
        .modified()
        .expect("mtime")
}

fn set_mtime(path: &std::path::Path, time: SystemTime) {
    std::fs::File::options()
        .write(true)
        .open(path)
        .expect("open")
        .set_modified(time)
        .expect("set mtime");
}

#[tokio::test]
async fn an_unchanged_file_is_not_read_again_whatever_its_status() {
    let root = dir("unchanged");
    let app = app(Library::Movies, "").await;
    let full = mkv(Some(("A_EAC3", 6, "ger")));
    let damaged = [
        el(0x1A45DFA3, &text(0x4282, "matroska")),
        // A Segment whose first child has an invalid ID.
        el(0x18538067, &[0xFF, 0x81, 0x00]),
    ]
    .concat();
    let files: [(i64, &str, Vec<u8>, ReadStatus); 3] = [
        (
            101,
            "partial.mkv",
            full[..full.len() - 1].to_vec(),
            ReadStatus::Partial,
        ),
        (102, "damaged.mkv", damaged, ReadStatus::Damaged),
        (
            103,
            "unsupported.mkv",
            b"no container".to_vec(),
            ReadStatus::Unsupported,
        ),
    ];
    for (tmdb, name, bytes, status) in files {
        let folder = root.join(name);
        std::fs::create_dir_all(&folder).expect("folder");
        let path = folder.join(name);
        std::fs::write(&path, bytes).expect("file");
        let file = FileRef::Movie(place(&app, tmdb, &folder, name).await);
        assert!(media::refresh(&app.db, file, &path).await.expect("first"));
        assert_eq!(
            stored(&app, file).await.map(|m| m.status),
            Some(status.clone())
        );
        assert!(
            !media::refresh(&app.db, file, &path).await.expect("second"),
            "{status:?} read again"
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn a_change_of_only_size_or_only_time_reads_again() {
    let root = dir("changed");
    let app = app(Library::Movies, "").await;
    let path = root.join("film.mkv");
    std::fs::write(&path, mkv(Some(("A_EAC3", 6, "ger")))).expect("file");
    let file = FileRef::Movie(place(&app, 101, &root, "film.mkv").await);
    assert!(media::refresh(&app.db, file, &path).await.expect("first"));
    assert!(
        !media::refresh(&app.db, file, &path)
            .await
            .expect("unchanged")
    );
    let time = mtime(&path);

    // Other size, same modification time.
    std::fs::write(&path, mkv(Some(("A_AC3", 2, "eng")))).expect("resize");
    set_mtime(&path, time);
    assert_eq!(mtime(&path), time);
    assert!(media::refresh(&app.db, file, &path).await.expect("size"));
    let info = stored(&app, file).await.expect("stored");
    assert_eq!(info.audio[0].codec, "ac3");

    // Same size, other modification time.
    set_mtime(&path, time + Duration::from_secs(3600));
    assert!(media::refresh(&app.db, file, &path).await.expect("time"));
    assert!(!media::refresh(&app.db, file, &path).await.expect("settled"));
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn a_file_written_during_the_read_is_read_again_at_the_next_scan() {
    let root = dir("growing");
    let app = app(Library::Movies, "").await;
    let path = root.join("film.mkv");
    let bytes = mkv(Some(("A_EAC3", 6, "ger")));
    std::fs::write(&path, &bytes).expect("file");
    let file = FileRef::Movie(place(&app, 101, &root, "film.mkv").await);
    // The file grows while it is read: after the read, before the result is stored.
    let read = media::refresh_with(&app.db, file, &path, |path| async move {
        let media = media::read(path.clone()).await;
        let mut writer = std::fs::File::options()
            .append(true)
            .open(&path)
            .expect("append");
        writer.write_all(&[0; 64]).expect("write");
        media
    })
    .await
    .expect("refresh");
    assert!(read);
    let row = app.db.media_info(file).await.expect("db").expect("stored");
    assert_eq!(
        row.size,
        bytes.len() as u64,
        "the size seen before the read"
    );
    assert!(
        media::refresh(&app.db, file, &path)
            .await
            .expect("next scan")
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// Counts the bytes handed out by the underlying reader.
struct Counting<R> {
    inner: R,
    read: u64,
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.read += n as u64;
        Ok(n)
    }
}

impl<R: Seek> Seek for Counting<R> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(pos)
    }
}

#[test]
fn reading_a_file_reads_at_most_the_limit_from_the_underlying_reader() {
    // An unknown-size Segment with 24 MiB of 1 KiB Void elements: every element header
    // is read, every body skipped by seeking.
    let mut file = el(0x1A45DFA3, &text(0x4282, "matroska"));
    file.extend([
        0x18, 0x53, 0x80, 0x67, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    ]);
    let void = el(0xEC, &[0; 1024 - 9]);
    while file.len() < 24 << 20 {
        file.extend_from_slice(&void);
    }
    let mut reader = Counting {
        inner: Cursor::new(file),
        read: 0,
    };
    let info = media::read_file(&mut reader);
    assert_eq!(info.status, ReadStatus::Partial);
    assert!(
        reader.read <= READ_LIMIT + 64 * 1024,
        "read {} bytes from the reader",
        reader.read
    );
}
