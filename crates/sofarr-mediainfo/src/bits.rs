//! Bounds-checked big-endian integers and a most-significant-bit-first bit reader.

pub(crate) fn be16(data: &[u8], at: usize) -> Option<u16> {
    let bytes = data.get(at..at.checked_add(2)?)?;
    Some(u16::from_be_bytes(bytes.try_into().ok()?))
}

pub(crate) fn be32(data: &[u8], at: usize) -> Option<u32> {
    let bytes = data.get(at..at.checked_add(4)?)?;
    Some(u32::from_be_bytes(bytes.try_into().ok()?))
}

pub(crate) fn be64(data: &[u8], at: usize) -> Option<u64> {
    let bytes = data.get(at..at.checked_add(8)?)?;
    Some(u64::from_be_bytes(bytes.try_into().ok()?))
}

/// Unsigned big-endian integer of 0 to 8 bytes.
pub(crate) fn be_uint(data: &[u8]) -> Option<u64> {
    if data.len() > 8 {
        return None;
    }
    Some(
        data.iter()
            .fold(0u64, |acc, byte| (acc << 8) | u64::from(*byte)),
    )
}

/// Whether `needle` occurs in `haystack`.
pub(crate) fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

pub(crate) struct BitReader<'a> {
    data: &'a [u8],
    /// Position in bits.
    pos: usize,
}

impl<'a> BitReader<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Reads `n` bits (at most 32), most significant first.
    pub(crate) fn read(&mut self, n: u32) -> Option<u32> {
        if n > 32 {
            return None;
        }
        let mut value = 0u32;
        for _ in 0..n {
            let byte = *self.data.get(self.pos / 8)?;
            let bit = (byte >> (7 - (self.pos % 8))) & 1;
            value = (value << 1) | u32::from(bit);
            self.pos += 1;
        }
        Some(value)
    }

    pub(crate) fn flag(&mut self) -> Option<bool> {
        self.read(1).map(|bit| bit == 1)
    }

    pub(crate) fn skip(&mut self, n: usize) -> Option<()> {
        let pos = self.pos.checked_add(n)?;
        if pos > self.data.len().checked_mul(8)? {
            return None;
        }
        self.pos = pos;
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integers_are_bounds_checked() {
        let data = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
        assert_eq!(be16(&data, 0), Some(0x1234));
        assert_eq!(be32(&data, 4), Some(0x9ABC_DEF0));
        assert_eq!(be64(&data, 0), Some(0x1234_5678_9ABC_DEF0));
        assert_eq!(be16(&data, 7), None);
        assert_eq!(be32(&data, usize::MAX), None);
        assert_eq!(be_uint(&[1, 0]), Some(256));
        assert_eq!(be_uint(&[0; 9]), None);
    }

    #[test]
    fn bit_reader_reads_msb_first_and_stops_at_the_end() {
        let mut bits = BitReader::new(&[0b1010_1100, 0xFF]);
        assert_eq!(bits.read(1), Some(1));
        assert_eq!(bits.read(3), Some(0b010));
        assert_eq!(bits.read(8), Some(0b1100_1111));
        assert_eq!(bits.skip(4), Some(()));
        assert_eq!(bits.read(1), None);
        assert!(BitReader::new(&[0]).skip(9).is_none());
    }
}
