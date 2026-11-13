//! A downloaded subtitle as UTF-8 text (AC-04, AC-14): UTF-8 with or without byte-order
//! mark, UTF-16 with byte-order mark, otherwise Windows-1252 (the usual legacy encoding of
//! German and English subtitles). Anything empty, binary or without SRT timings is refused.

/// Why a download is no usable subtitle; the messages are German (they reach the log
/// and the overview).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TextError {
    #[error("Untertitel ist leer")]
    Empty,
    #[error("Untertitel ist kein Text")]
    NotText,
    #[error("Untertitel lässt sich nicht nach UTF-8 umwandeln")]
    NotConvertible,
    #[error("Untertitel ist kein SRT")]
    NotSrt,
}

/// Windows-1252 characters of the bytes 0x80..=0x9F; `None` where the code page has none.
const CP1252_HIGH: [Option<char>; 32] = [
    Some('\u{20AC}'),
    None,
    Some('\u{201A}'),
    Some('\u{0192}'),
    Some('\u{201E}'),
    Some('\u{2026}'),
    Some('\u{2020}'),
    Some('\u{2021}'),
    Some('\u{02C6}'),
    Some('\u{2030}'),
    Some('\u{0160}'),
    Some('\u{2039}'),
    Some('\u{0152}'),
    None,
    Some('\u{017D}'),
    None,
    None,
    Some('\u{2018}'),
    Some('\u{2019}'),
    Some('\u{201C}'),
    Some('\u{201D}'),
    Some('\u{2022}'),
    Some('\u{2013}'),
    Some('\u{2014}'),
    Some('\u{02DC}'),
    Some('\u{2122}'),
    Some('\u{0161}'),
    Some('\u{203A}'),
    Some('\u{0153}'),
    None,
    Some('\u{017E}'),
    Some('\u{0178}'),
];

fn cp1252(bytes: &[u8]) -> Result<String, TextError> {
    bytes
        .iter()
        .map(|&b| match b {
            0x80..=0x9f => CP1252_HIGH[usize::from(b - 0x80)].ok_or(TextError::NotConvertible),
            _ => Ok(char::from(b)),
        })
        .collect()
}

fn utf16(bytes: &[u8], little_endian: bool) -> Result<String, TextError> {
    if !bytes.len().is_multiple_of(2) {
        return Err(TextError::NotConvertible);
    }
    let (pairs, _) = bytes.as_chunks::<2>();
    let units: Vec<u16> = pairs
        .iter()
        .map(|pair| {
            if little_endian {
                u16::from_le_bytes(*pair)
            } else {
                u16::from_be_bytes(*pair)
            }
        })
        .collect();
    String::from_utf16(&units).map_err(|_| TextError::NotConvertible)
}

fn decode(bytes: &[u8]) -> Result<String, TextError> {
    if let Some(rest) = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
        return String::from_utf8(rest.to_vec()).map_err(|_| TextError::NotConvertible);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xff, 0xfe]) {
        return utf16(rest, true);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xfe, 0xff]) {
        return utf16(rest, false);
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => Ok(text.to_owned()),
        Err(_) => cp1252(bytes),
    }
}

/// The subtitle as UTF-8 text without byte-order mark.
pub fn to_utf8(bytes: &[u8]) -> Result<String, TextError> {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Err(TextError::Empty);
    }
    // Binary content (images, archives) has NUL bytes or other control characters.
    let text = decode(bytes)?;
    if text
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t' | '\u{c}'))
    {
        return Err(TextError::NotText);
    }
    if text.trim().is_empty() {
        return Err(TextError::Empty);
    }
    if !text.lines().any(|line| line.contains("-->")) {
        return Err(TextError::NotSrt);
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRT: &str = "1\n00:00:01,000 --> 00:00:02,000\nGrüße\n";

    #[test]
    fn utf8_stays_and_loses_its_mark() {
        assert_eq!(to_utf8(SRT.as_bytes()).as_deref(), Ok(SRT));
        let mut marked = vec![0xef, 0xbb, 0xbf];
        marked.extend_from_slice(SRT.as_bytes());
        assert_eq!(to_utf8(&marked).as_deref(), Ok(SRT));
    }

    #[test]
    fn legacy_and_utf16_are_converted() {
        // "Grüße" in Windows-1252, with a typographic quote from the 0x80 range.
        let mut latin = b"1\n00:00:01,000 --> 00:00:02,000\nGr".to_vec();
        latin.extend_from_slice(&[0xfc, 0xdf, 0x65, 0x84, 0x93]);
        assert_eq!(
            to_utf8(&latin).as_deref(),
            Ok("1\n00:00:01,000 --> 00:00:02,000\nGrüße\u{201E}\u{201C}")
        );
        let mut le = vec![0xff, 0xfe];
        le.extend(SRT.encode_utf16().flat_map(u16::to_le_bytes));
        assert_eq!(to_utf8(&le).as_deref(), Ok(SRT));
        let mut be = vec![0xfe, 0xff];
        be.extend(SRT.encode_utf16().flat_map(u16::to_be_bytes));
        assert_eq!(to_utf8(&be).as_deref(), Ok(SRT));
    }

    #[test]
    fn unusable_downloads_are_refused() {
        assert_eq!(to_utf8(b""), Err(TextError::Empty));
        assert_eq!(to_utf8(b" \r\n "), Err(TextError::Empty));
        assert_eq!(to_utf8(&[0xef, 0xbb, 0xbf]), Err(TextError::Empty));
        assert_eq!(
            to_utf8(b"PK\x03\x04\x00\x00 --> binary"),
            Err(TextError::NotText)
        );
        // 0x81 has no character in Windows-1252.
        assert_eq!(
            to_utf8(&[b'-', b'-', b'>', 0x81]),
            Err(TextError::NotConvertible)
        );
        assert_eq!(to_utf8(&[0xff, 0xfe, 0x41]), Err(TextError::NotConvertible));
        assert_eq!(to_utf8(b"<html>Not found</html>"), Err(TextError::NotSrt));
    }
}
