//! Finds the boundary between the title and the metadata part, the year and the
//! release group.

use std::ops::Range;

use crate::classify::{Tag, is_unambiguous_strong};
use crate::token::{Sep, Token};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Layout {
    /// Token ranges of the title candidates.
    pub titles: Vec<Range<usize>>,
    /// First token after the title; everything from here on is metadata.
    pub metadata_start: usize,
    pub year: Option<(usize, u16)>,
    /// Byte range of the release group in the input.
    pub group: Option<Range<usize>>,
    /// Token range of the release group, excluded from metadata scanning.
    pub group_tokens: Option<Range<usize>>,
}

pub fn layout(input: &str, tokens: &[Token<'_>], tags: &[Tag]) -> Layout {
    let n = tokens.len();
    if n == 0 {
        return Layout::default();
    }

    let leading = leading_group(input, tokens, tags);
    let start = leading.as_ref().map_or(0, |r| r.end);
    if start >= n {
        return Layout::default();
    }

    let first_strong = (start + 1..n)
        .find(|&i| is_unambiguous_strong(tokens, tags, i))
        .unwrap_or(n);
    let year = find_year(tokens, tags, start, first_strong);

    let mut title_end = match year {
        Some((i, _)) if i < first_strong => i,
        _ => (start + 1..first_strong)
            .find(|&i| tags[i].is_strong())
            .unwrap_or(first_strong),
    };
    let year_bounded = matches!(year, Some((i, _)) if i == title_end);
    // Drop trailing bracketed tokens and, without a year as anchor, trailing keywords
    // such as editions or languages (`Title.German.1080p`).
    while title_end > start + 1 {
        let last = title_end - 1;
        let keyword = !matches!(tags[last], Tag::Plain | Tag::Aka | Tag::Year(_));
        if !(tokens[last].bracketed || (!year_bounded && keyword)) {
            break;
        }
        title_end -= 1;
    }

    let boundary = year.map_or(title_end, |(i, _)| i.max(title_end));
    let (group, group_tokens) = match trailing_group(tokens, tags, boundary) {
        Some(range) => (
            Some(tokens[range.start].start..tokens[range.end - 1].end),
            Some(range),
        ),
        None => (
            leading
                .clone()
                .map(|r| tokens[r.start].start..tokens[r.end - 1].end),
            None,
        ),
    };

    Layout {
        titles: split_aka(tags, start..title_end),
        metadata_start: title_end,
        year,
        group,
        group_tokens,
    }
}

/// `[Group] Title …`: a square-bracketed group of unrecognised tokens at the very start.
fn leading_group(input: &str, tokens: &[Token<'_>], tags: &[Tag]) -> Option<Range<usize>> {
    if !tokens[0].bracketed || !input[..tokens[0].start].trim_end().ends_with('[') {
        return None;
    }
    let end = 1 + tokens[1..]
        .iter()
        .position(|t| !t.bracketed || t.sep == Sep::Bracket)?;
    tags[..end]
        .iter()
        .all(|t| *t == Tag::Plain)
        .then_some(0..end)
}

/// Picks the release year: a bracketed year wins, otherwise the right-most year before
/// the metadata. A year as first title token is never taken, so `1984.1080p` keeps its title.
fn find_year(
    tokens: &[Token<'_>],
    tags: &[Tag],
    start: usize,
    first_strong: usize,
) -> Option<(usize, u16)> {
    let year_at = |i: usize| match tags[i] {
        Tag::Year(y) => Some((i, y)),
        _ => None,
    };
    let before: Vec<(usize, u16)> = (start + 1..first_strong).filter_map(year_at).collect();
    before
        .iter()
        .rev()
        .find(|(i, _)| tokens[*i].bracketed)
        .or_else(|| before.last())
        .copied()
        .or_else(|| (first_strong.max(start + 1)..tokens.len()).find_map(year_at))
}

/// `…x264-GROUP`: unrecognised, dash-joined tokens at the end of the name.
fn trailing_group(tokens: &[Token<'_>], tags: &[Tag], boundary: usize) -> Option<Range<usize>> {
    let last = tokens.iter().rposition(|t| !t.bracketed)?;
    let candidate = |i: usize| i > boundary && tags[i] == Tag::Plain && tokens[i].sep == Sep::Dash;
    if !candidate(last) {
        return None;
    }
    let mut first = last;
    while first > 0 && candidate(first - 1) {
        first -= 1;
    }
    Some(first..last + 1)
}

fn split_aka(tags: &[Tag], range: Range<usize>) -> Vec<Range<usize>> {
    let mut parts = Vec::new();
    let mut part_start = range.start;
    for i in range.clone() {
        if tags[i] == Tag::Aka {
            parts.push(part_start..i);
            part_start = i + 1;
        }
    }
    parts.push(part_start..range.end);
    parts.retain(|r| !r.is_empty());
    parts
}

/// Turns a token range back into a readable title.
pub fn title_text(input: &str, tokens: &[Token<'_>], range: Range<usize>) -> Option<String> {
    let mut start = tokens[range.start].start;
    let end = tokens[range.end - 1].end;
    // Keep an opening bracket whose closing counterpart is part of the title: `(300) Letters`.
    if let Some(open) = input[..start].chars().next_back()
        && let Some(close) = closing(open)
        && input[start..end].contains(close)
    {
        start -= open.len_utf8();
    }
    let raw = &input[start..end];
    // Dotted names use `.` as a word separator; in spaced names a dot is punctuation.
    let spaced = raw.contains(' ');
    let text: String = raw
        .chars()
        .map(|c| match c {
            '_' => ' ',
            '.' if !spaced => ' ',
            c => c,
        })
        .collect();
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!text.is_empty()).then_some(text)
}

fn closing(open: char) -> Option<char> {
    match open {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        _ => None,
    }
}
