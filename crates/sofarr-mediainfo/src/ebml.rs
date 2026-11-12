//! EBML element headers (RFC 8794): variable-size integers for IDs and data sizes.

use std::io::{Read, Seek};

use crate::bits;
use crate::source::{Res, Source, Stop};

/// The ID of the EBML header element, the first four bytes of every EBML document.
pub(crate) const MAGIC: [u8; 4] = [0x1A, 0x45, 0xDF, 0xA3];

/// One element header read from the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Header {
    pub(crate) id: u32,
    /// `None` for an unknown size.
    pub(crate) size: Option<u64>,
    /// Position of the first ID byte.
    pub(crate) start: u64,
    /// Position of the first data byte.
    pub(crate) data: u64,
}

impl Header {
    /// Position after the element, `None` for an unknown size.
    pub(crate) fn end(&self) -> Res<Option<u64>> {
        match self.size {
            Some(size) => self.data.checked_add(size).map(Some).ok_or(Stop::Damaged),
            None => Ok(None),
        }
    }
}

/// Length in bytes of the variable-size integer starting with `first`.
fn vint_len(first: u8) -> Option<usize> {
    (first != 0).then(|| first.leading_zeros() as usize + 1)
}

/// An element ID with its marker bits; IDs of more than four bytes, and IDs whose value bits
/// are all zero or all one, are invalid.
fn decode_id(bytes: &[u8]) -> Option<u32> {
    let len = bytes.len();
    if !(1..=4).contains(&len) {
        return None;
    }
    let value = bits::be_uint(bytes)?;
    let mask = (1u64 << (7 * len)) - 1;
    let data = value & mask;
    if data == 0 || data == mask {
        return None;
    }
    u32::try_from(value).ok()
}

/// A data size; all value bits set means unknown.
fn decode_size(bytes: &[u8]) -> Option<Option<u64>> {
    let len = bytes.len();
    let (first, rest) = bytes.split_first()?;
    if len > 8 {
        return None;
    }
    let mut value = u64::from(first & 0xFFu8.checked_shr(len as u32).unwrap_or(0));
    for byte in rest {
        value = (value << 8) | u64::from(*byte);
    }
    let unknown = (1u64 << (7 * len)) - 1;
    Some((value != unknown).then_some(value))
}

/// A variable-size integer without its marker, with its length (block track numbers, lace sizes).
pub(crate) fn vint(data: &[u8]) -> Option<(u64, usize)> {
    let len = vint_len(*data.first()?)?;
    if len > 8 {
        return None;
    }
    let value = decode_size(data.get(..len)?)?.unwrap_or((1u64 << (7 * len)) - 1);
    Some((value, len))
}

/// Reads the element header at the current position.
pub(crate) fn read_header<R: Read + Seek>(src: &mut Source<'_, R>) -> Res<Header> {
    let start = src.pos();
    let first = src.read_byte()?;
    let id_len = vint_len(first)
        .filter(|len| *len <= 4)
        .ok_or(Stop::Damaged)?;
    let mut id_bytes = vec![first];
    id_bytes.extend(src.read_vec(id_len as u64 - 1)?);
    let id = decode_id(&id_bytes).ok_or(Stop::Damaged)?;
    let size_first = src.read_byte()?;
    let size_len = vint_len(size_first).ok_or(Stop::Damaged)?;
    let mut size_bytes = vec![size_first];
    size_bytes.extend(src.read_vec(size_len as u64 - 1)?);
    let size = decode_size(&size_bytes).ok_or(Stop::Damaged)?;
    Ok(Header {
        id,
        size,
        start,
        data: src.pos(),
    })
}

/// The child elements of a fully read element. A child that does not fit into its parent,
/// an unknown size or an invalid ID make the structure damaged.
pub(crate) struct Children<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Children<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn next_child(&mut self) -> Res<(u32, &'a [u8])> {
        let rest = self.data.get(self.pos..).ok_or(Stop::Damaged)?;
        let id_len = rest
            .first()
            .and_then(|first| vint_len(*first))
            .filter(|len| *len <= 4)
            .ok_or(Stop::Damaged)?;
        let id = rest
            .get(..id_len)
            .and_then(decode_id)
            .ok_or(Stop::Damaged)?;
        let size_len = rest
            .get(id_len)
            .and_then(|first| vint_len(*first))
            .ok_or(Stop::Damaged)?;
        let header_len = id_len + size_len;
        let size = rest
            .get(id_len..header_len)
            .and_then(decode_size)
            .flatten()
            .ok_or(Stop::Damaged)?;
        let size = usize::try_from(size).map_err(|_| Stop::Damaged)?;
        let end = header_len.checked_add(size).ok_or(Stop::Damaged)?;
        let body = rest.get(header_len..end).ok_or(Stop::Damaged)?;
        self.pos += end;
        Ok((id, body))
    }
}

impl<'a> Iterator for Children<'a> {
    type Item = Res<(u32, &'a [u8])>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.data.len() {
            return None;
        }
        let child = self.next_child();
        if child.is_err() {
            self.pos = self.data.len();
        }
        Some(child)
    }
}

/// An unsigned integer element.
pub(crate) fn uint(data: &[u8]) -> Res<u64> {
    bits::be_uint(data).ok_or(Stop::Damaged)
}

/// A string element: UTF-8 (or ASCII) up to the first NUL, trimmed.
pub(crate) fn string(data: &[u8]) -> String {
    let end = data.iter().position(|b| *b == 0).unwrap_or(data.len());
    String::from_utf8_lossy(data.get(..end).unwrap_or_default())
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{el, el_uint};

    #[test]
    fn ids_and_sizes_decode_with_their_rules() {
        assert_eq!(decode_id(&[0x1A, 0x45, 0xDF, 0xA3]), Some(0x1A45_DFA3));
        assert_eq!(decode_id(&[0xEC]), Some(0xEC));
        assert_eq!(decode_id(&[0xFF]), None);
        assert_eq!(decode_id(&[0x80]), None);
        assert_eq!(decode_size(&[0x81]), Some(Some(1)));
        assert_eq!(decode_size(&[0x40, 0x02]), Some(Some(2)));
        assert_eq!(decode_size(&[0xFF]), Some(None));
        assert_eq!(
            decode_size(&[0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
            Some(None)
        );
        assert_eq!(vint(&[0x81, 0x00]), Some((1, 1)));
        assert_eq!(vint(&[0x00]), None);
    }

    #[test]
    fn children_reject_overlong_children() {
        let mut data = el_uint(0xD7, 5);
        data.extend(el(0x86, b"V_VP9"));
        let children: Vec<_> = Children::new(&data).collect();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0], Ok((0xD7, &[5u8][..])));
        // A size larger than the parent.
        let broken = [0x86, 0x85, b'A'];
        let children: Vec<_> = Children::new(&broken).collect();
        assert_eq!(children, vec![Err(Stop::Damaged)]);
    }

    #[test]
    fn strings_stop_at_nul() {
        assert_eq!(string(b"ger\0\0"), "ger");
        assert_eq!(string(b" eng "), "eng");
    }
}
