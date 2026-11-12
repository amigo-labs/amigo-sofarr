//! The input with a read budget: every byte handed out counts against [`READ_LIMIT`],
//! seeking is free.

use std::io::{ErrorKind, Read, Seek, SeekFrom};

use crate::READ_LIMIT;

/// Why reading stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stop {
    /// The input ended inside a structure.
    Truncated,
    /// The read budget is used up.
    Limit,
    /// The structure is invalid.
    Damaged,
    /// The reader failed.
    Io,
}

pub(crate) type Res<T> = Result<T, Stop>;

pub(crate) struct Source<'a, R> {
    inner: &'a mut R,
    len: u64,
    pos: u64,
    /// Where the inner reader stands, if known; avoids seeking between sequential reads.
    inner_pos: Option<u64>,
    budget: u64,
}

impl<'a, R: Read + Seek> Source<'a, R> {
    pub(crate) fn new(inner: &'a mut R) -> Res<Self> {
        let len = inner.seek(SeekFrom::End(0)).map_err(|_| Stop::Io)?;
        Ok(Self {
            inner,
            len,
            pos: 0,
            inner_pos: None,
            budget: READ_LIMIT,
        })
    }

    pub(crate) fn len(&self) -> u64 {
        self.len
    }

    pub(crate) fn pos(&self) -> u64 {
        self.pos
    }

    pub(crate) fn seek(&mut self, pos: u64) {
        self.pos = pos;
    }

    /// Reads exactly `n` bytes.
    pub(crate) fn read_vec(&mut self, n: u64) -> Res<Vec<u8>> {
        let end = self.pos.checked_add(n).ok_or(Stop::Damaged)?;
        if end > self.len {
            return Err(Stop::Truncated);
        }
        if n > self.budget {
            return Err(Stop::Limit);
        }
        self.fill(n)
    }

    /// Reads up to `n` bytes: fewer at the end of the input or of the budget.
    pub(crate) fn read_some(&mut self, n: u64) -> Res<Vec<u8>> {
        let available = self.len.saturating_sub(self.pos);
        self.fill(n.min(available).min(self.budget))
    }

    pub(crate) fn read_byte(&mut self) -> Res<u8> {
        let bytes = self.read_vec(1)?;
        bytes.first().copied().ok_or(Stop::Truncated)
    }

    fn fill(&mut self, n: u64) -> Res<Vec<u8>> {
        if n == 0 {
            return Ok(Vec::new());
        }
        let size = usize::try_from(n).map_err(|_| Stop::Limit)?;
        // Charged before reading, so a failing read can never exceed the limit either.
        self.budget = self.budget.saturating_sub(n);
        if self.inner_pos != Some(self.pos) {
            self.inner_pos = None;
            self.inner
                .seek(SeekFrom::Start(self.pos))
                .map_err(|_| Stop::Io)?;
        }
        let mut buf = vec![0; size];
        if let Err(err) = self.inner.read_exact(&mut buf) {
            self.inner_pos = None;
            return Err(if err.kind() == ErrorKind::UnexpectedEof {
                Stop::Truncated
            } else {
                Stop::Io
            });
        }
        self.pos = self.pos.saturating_add(n);
        self.inner_pos = Some(self.pos);
        Ok(buf)
    }
}
