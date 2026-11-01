//! Case-insensitive regular expressions for custom format conditions.
//!
//! Profiles are written for a regex flavour with unrestricted lookbehind
//! (`(?<=\bGerman\b.*)\bDL\b`). fancy-regex compiles such lookbehinds but silently
//! never matches some of them (a `\b` before `.*` inside the lookbehind). A lookbehind
//! at the start of a top-level alternative is therefore evaluated here: the rest of the
//! alternative is searched normally, and at each match start the lookbehind body is
//! matched forwards against the text before it, anchored at its end. Lookbehinds
//! elsewhere are left to fancy-regex.

use fancy_regex::{Regex, RegexBuilder};

/// A case-insensitive regular expression, compared by its source text.
#[derive(Debug, Clone)]
pub struct Pattern {
    source: String,
    matcher: Matcher,
}

#[derive(Debug, Clone)]
enum Matcher {
    Plain(Regex),
    /// Top-level alternatives, at least one of them starting with a lookbehind.
    Branches(Vec<Branch>),
}

#[derive(Debug, Clone)]
struct Branch {
    lookbehind: Option<Lookbehind>,
    rest: Regex,
}

#[derive(Debug, Clone)]
struct Lookbehind {
    negative: bool,
    /// The body, anchored at the end of the text.
    body: Regex,
}

fn build(source: &str) -> Result<Regex, Box<fancy_regex::Error>> {
    RegexBuilder::new(source)
        .case_insensitive(true)
        .build()
        .map_err(Box::new)
}

impl Pattern {
    pub fn new(source: &str) -> Result<Self, Box<fancy_regex::Error>> {
        let (flags, body) = leading_flags(source);
        let branches = split_alternatives(body);
        let matcher = if branches.iter().any(|b| lookbehind_prefix(b).is_some()) {
            Matcher::Branches(
                branches
                    .into_iter()
                    .map(|b| branch(flags, b))
                    .collect::<Result<_, _>>()?,
            )
        } else {
            Matcher::Plain(build(source)?)
        };
        Ok(Self {
            source: source.to_owned(),
            matcher,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.source
    }

    /// Whether the pattern matches; a pattern that fails at runtime does not match.
    pub fn is_match(&self, text: &str) -> bool {
        match &self.matcher {
            Matcher::Plain(regex) => regex.is_match(text).unwrap_or(false),
            Matcher::Branches(branches) => branches.iter().any(|b| b.is_match(text)),
        }
    }
}

impl PartialEq for Pattern {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
    }
}

impl Eq for Pattern {}

impl Branch {
    fn is_match(&self, text: &str) -> bool {
        let Some(lookbehind) = &self.lookbehind else {
            return self.rest.is_match(text).unwrap_or(false);
        };
        let mut pos = 0;
        while pos <= text.len() {
            let Ok(Some(found)) = self.rest.find_from_pos(text, pos) else {
                return false;
            };
            let start = found.start();
            let before = lookbehind.body.is_match(&text[..start]).unwrap_or(false);
            if before != lookbehind.negative {
                return true;
            }
            pos = start + text[start..].chars().next().map_or(1, char::len_utf8);
        }
        false
    }
}

fn branch(flags: &str, source: &str) -> Result<Branch, Box<fancy_regex::Error>> {
    let Some((negative, body, rest)) = lookbehind_prefix(source) else {
        return Ok(Branch {
            lookbehind: None,
            rest: build(&format!("{flags}{source}"))?,
        });
    };
    Ok(Branch {
        lookbehind: Some(Lookbehind {
            negative,
            body: build(&format!(r"{flags}(?:{body})\z"))?,
        }),
        rest: build(&format!("{flags}{rest}"))?,
    })
}

/// Inline flag groups at the very start (`(?i)`), which apply to every alternative.
fn leading_flags(source: &str) -> (&str, &str) {
    let mut end = 0;
    while let Some(group) = source[end..].strip_prefix("(?") {
        let flags = group.find(')').filter(|&i| {
            group[..i]
                .chars()
                .all(|c| c.is_ascii_alphabetic() || c == '-')
        });
        match flags {
            Some(i) if i > 0 => end += 2 + i + 1,
            _ => break,
        }
    }
    source.split_at(end)
}

/// `(?<=body)rest` or `(?<!body)rest` → (negative, body, rest).
fn lookbehind_prefix(branch: &str) -> Option<(bool, &str, &str)> {
    let negative = if branch.starts_with("(?<=") {
        false
    } else if branch.starts_with("(?<!") {
        true
    } else {
        return None;
    };
    let close = group_end(branch)?;
    Some((negative, &branch[4..close], &branch[close + 1..]))
}

/// Index of the `)` closing the group that opens at index 0.
fn group_end(source: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut scan = Scanner::default();
    for (i, c) in source.char_indices() {
        if !scan.structural(c) {
            continue;
        }
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Splits at `|` outside of groups and character classes.
fn split_alternatives(source: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    let mut scan = Scanner::default();
    for (i, c) in source.char_indices() {
        if !scan.structural(c) {
            continue;
        }
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            '|' if depth == 0 => {
                parts.push(&source[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&source[start..]);
    parts
}

/// Tracks escapes and character classes while walking a regex source.
#[derive(Default)]
struct Scanner {
    escaped: bool,
    class: Option<Class>,
}

/// Position inside `[...]`: a `]` right after `[` or `[^` is a literal.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Start,
    AfterCaret,
    Body,
}

impl Scanner {
    /// Whether `c` is a structural character (not escaped, not inside a class).
    fn structural(&mut self, c: char) -> bool {
        if self.escaped {
            self.escaped = false;
            return false;
        }
        if let Some(class) = self.class {
            self.class = match (class, c) {
                (Class::Body, ']') => None,
                (Class::Start, '^') => Some(Class::AfterCaret),
                _ => Some(Class::Body),
            };
            self.escaped = c == '\\';
            return false;
        }
        match c {
            '\\' => self.escaped = true,
            '[' => self.class = Some(Class::Start),
            _ => return true,
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(source: &str, text: &str) -> bool {
        Pattern::new(source).unwrap().is_match(text)
    }

    #[test]
    fn variable_lookbehind_with_word_boundaries() {
        let p = r"(?<=\bGerman\b.*)\bDL\b";
        assert!(matches(p, "Some.Film.2009.German.DTS.DL.1080p"));
        assert!(matches(p, "Some.Film.2009.GERMAN.DOKU.DL.720p-GRP.German"));
        assert!(!matches(p, "Some.Film.2009.DL.German.1080p"));
        assert!(!matches(p, "Some.Film.2009.Germany.DL"));
    }

    #[test]
    fn alternatives_flags_and_negative_lookbehind() {
        let p = r"(?i)german\s*\.?dl|(?<=\bGerman\b.*)(?<!\bWEB[-_. ])\bDL\b|\[DE\+[a-z]{2}\]";
        assert!(matches(p, "Some.Film.German.DL.1080p"));
        assert!(matches(p, "Some.Film.German.AC3.DL.1080p"));
        assert!(!matches(p, "Some.Film.German.1080p.WEB.DL"));
        assert!(matches(p, "Some Film [DE+en] 1080p"));
        assert!(matches(r"(?<!x)ab", "zab"));
        assert!(!matches(r"(?<!x)ab", "xab"));
        assert!(matches(r"(?<!x)ab", "xab ab"));
    }

    #[test]
    fn plain_patterns_stay_untouched() {
        assert!(matches(r"\bx265\b|\bhevc\b", "Some.Film.HEVC"));
        // `|` inside groups and classes does not split.
        assert!(matches(r"(?<=[|])a(b|c)", "|ac"));
        assert!(!matches(r"(?<=[|])a(b|c)", "-ac"));
        assert!(matches(r"(?<=\()x", "(x"));
    }

    #[test]
    fn scanning_helpers() {
        assert_eq!(leading_flags("(?i)ab"), ("(?i)", "ab"));
        assert_eq!(leading_flags("(?<=a)b"), ("", "(?<=a)b"));
        assert_eq!(
            split_alternatives(r"a|(b|c)|[|]|\|"),
            ["a", "(b|c)", "[|]", r"\|"]
        );
        assert_eq!(lookbehind_prefix(r"(?<=a(b))c"), Some((false, "a(b)", "c")));
        assert_eq!(lookbehind_prefix(r"(?<!\))c"), Some((true, r"\)", "c")));
    }
}
