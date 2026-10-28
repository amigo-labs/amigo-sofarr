//! Finds the boundary between the title and the metadata part, the year and the
//! release group.

use std::ops::Range;

use crate::classify::{Tag, is_unambiguous_strong};
use crate::model::Language;
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
    let year = find_year(tags, start, first_strong);

    let mut title_end = match year {
        Some((i, _)) if i < first_strong => i,
        _ => (start + 1..first_strong)
            .find(|&i| tags[i].is_strong())
            .unwrap_or(first_strong),
    };
    let year_bounded = matches!(year, Some((i, _)) if i == title_end);
    // Drop trailing keywords from the title. Editions and languages always go
    // (`Title.GERMAN.2026`, `Title.Directors.Cut.2026`); other keywords only without a
    // year as anchor, since words like `Web` may belong to a title (`Spinners.Web.2006`).
    // Bracketed plain words stay: `Title (Other Title) 1999`.
    while title_end > start + 1 {
        let last = title_end - 1;
        let head = phrase_head(tags, last);
        // Of the languages only German is cut off: `Johnny.English.2003` keeps its title.
        let edition_or_language = matches!(
            tags[head],
            Tag::Edition | Tag::Language(Language::German) | Tag::DualLanguage
        );
        let keyword = !matches!(tags[last], Tag::Plain | Tag::Aka | Tag::Year(_));
        let removable =
            edition_or_language || (keyword && (tokens[last].bracketed || !year_bounded));
        if !removable || head <= start {
            break;
        }
        title_end = if edition_or_language { head } else { last };
    }

    let boundary = year.map_or(title_end, |(i, _)| i.max(title_end));
    let trailing = bracketed_group(input, tokens, tags, boundary)
        .or_else(|| trailing_group(tokens, tags, boundary));
    let (group, group_tokens) = match trailing {
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

/// First token of the keyword phrase that ends at `i` (`Directors.Cut` → `Directors`).
fn phrase_head(tags: &[Tag], i: usize) -> usize {
    let mut head = i;
    while head > 0 && tags[head] == Tag::Continued {
        head -= 1;
    }
    head
}

/// Picks the release year: the first year before the metadata, unless another year
/// follows right after it (`Station.2088.2019`). A year as first title token is never
/// taken, so `1984.1080p` keeps its title. Double features keep the first year
/// (`First.1968.-.Second.1970`).
fn find_year(tags: &[Tag], start: usize, first_strong: usize) -> Option<(usize, u16)> {
    let year_at = |i: usize| match tags.get(i) {
        Some(Tag::Year(y)) => Some((i, *y)),
        _ => None,
    };
    (start + 1..first_strong)
        .filter_map(year_at)
        .find(|(i, _)| year_at(i + 1).is_none())
        .or_else(|| (first_strong.max(start + 1)..tags.len()).find_map(year_at))
}

/// `…x264-GROUP`: the last dash-separated token after the metadata. Only the last
/// segment counts (`-DNU-TAoE` → `TAoE`), scene words may serve as group names
/// (`-ATMOS`), a numeric suffix is skipped (`-GRP-1`), and words after the group are
/// ignored (`-Ben.The.Men`). It may sit inside brackets: `(… English.-.GRP)`.
fn trailing_group(tokens: &[Token<'_>], tags: &[Tag], boundary: usize) -> Option<Range<usize>> {
    let numeric = |i: usize| tokens[i].text.bytes().all(|b| b.is_ascii_digit());
    let last = (boundary + 1..tokens.len()).rev().find(|&i| {
        tokens[i].sep == Sep::Dash && matches!(tags[i], Tag::Plain | Tag::Technical) && !numeric(i)
    })?;
    Some(last..last + 1)
}

/// `….(2001).[GROUP]`: a single word in square brackets at the very end, attached to
/// the name with a dot or a closing bracket. Years are not groups.
fn bracketed_group(
    input: &str,
    tokens: &[Token<'_>],
    tags: &[Tag],
    boundary: usize,
) -> Option<Range<usize>> {
    let last = tokens.len() - 1;
    let token = &tokens[last];
    let before = input[..token.start].strip_suffix('[')?;
    let attached = before.ends_with('.') || before.ends_with(')');
    let closed = input[token.end..].trim() == "]";
    (last > boundary && attached && closed && !matches!(tags[last], Tag::Year(_)))
        .then_some(last..last + 1)
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
    let mut end = tokens[range.end - 1].end;
    // Keep a closing bracket whose opening counterpart is part of the title:
    // `Title (Other Title)`.
    if let Some(close) = input[end..].chars().next()
        && let Some(open) = opening(close)
        && input[start..end].contains(open)
    {
        end += close.len_utf8();
    }
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

fn opening(close: char) -> Option<char> {
    match close {
        ')' => Some('('),
        ']' => Some('['),
        '}' => Some('{'),
        _ => None,
    }
}
