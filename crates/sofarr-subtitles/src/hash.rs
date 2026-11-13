//! The movie hash of OpenSubtitles' published algorithm: the file size plus the sum of
//! all 64-bit little-endian words of the first and of the last 64 KiB, modulo 2^64,
//! written as 16 lower-case hex digits.

use std::io::{Read, Seek, SeekFrom};

/// Bytes read from each end of the file.
pub const HASH_CHUNK: u64 = 64 * 1024;

fn sum_chunk<R: Read + Seek>(reader: &mut R, start: u64, sum: u64) -> std::io::Result<u64> {
    reader.seek(SeekFrom::Start(start))?;
    let mut buffer = vec![0u8; usize::try_from(HASH_CHUNK).unwrap_or(65_536)];
    reader.read_exact(&mut buffer)?;
    let (words, _) = buffer.as_chunks::<8>();
    Ok(words
        .iter()
        .fold(sum, |sum, word| sum.wrapping_add(u64::from_le_bytes(*word))))
}

/// The hash of a video; `None` for a file smaller than one chunk (the algorithm needs
/// 64 KiB at each end; the chunks of a file below 128 KiB overlap).
pub fn movie_hash<R: Read + Seek>(reader: &mut R) -> std::io::Result<Option<String>> {
    let size = reader.seek(SeekFrom::End(0))?;
    if size < HASH_CHUNK {
        return Ok(None);
    }
    let sum = sum_chunk(reader, 0, size)?;
    let sum = sum_chunk(reader, size - HASH_CHUNK, sum)?;
    Ok(Some(format!("{sum:016x}")))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn hash(bytes: Vec<u8>) -> Option<String> {
        movie_hash(&mut Cursor::new(bytes)).expect("hash")
    }

    #[test]
    fn hash_of_zeros_is_the_size() {
        // Every word is zero: only the size counts (128 KiB = 0x20000).
        assert_eq!(hash(vec![0; 131_072]).as_deref(), Some("0000000000020000"));
    }

    #[test]
    fn hash_sums_the_words_of_both_ends_only() {
        // 200,000 bytes: the first word is 1, the last word 0x0102030405060708, a word in
        // the middle (outside both chunks) must not count.
        let mut bytes = vec![0u8; 200_000];
        bytes[..8].copy_from_slice(&1u64.to_le_bytes());
        bytes[100_000..100_008].copy_from_slice(&0xffu64.to_le_bytes());
        bytes[199_992..].copy_from_slice(&0x0102_0304_0506_0708u64.to_le_bytes());
        // 200,000 + 1 + 0x0102030405060708 = 0x0102030405091449
        assert_eq!(hash(bytes).as_deref(), Some("0102030405091449"));
    }

    #[test]
    fn hash_wraps_and_counts_overlapping_chunks_twice() {
        // 64 KiB exactly: both chunks are the whole file, so the first word counts twice;
        // u64::MAX twice plus the size wraps to 0x10000 - 2.
        let mut bytes = vec![0u8; 65_536];
        bytes[..8].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(hash(bytes).as_deref(), Some("000000000000fffe"));
    }

    #[test]
    fn small_files_have_no_hash() {
        assert_eq!(hash(vec![1; 65_535]), None);
        assert_eq!(hash(Vec::new()), None);
    }
}
